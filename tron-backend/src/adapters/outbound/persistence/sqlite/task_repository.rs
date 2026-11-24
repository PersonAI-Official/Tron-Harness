mod paging;

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::{
    application::task::{TaskReadRepository, TaskRelations, TaskRepository},
    domain::task::{
        PagedTaskRecords, Task, TaskCodeLink, TaskError, TaskExecutionRecord,
        TaskExecutionStatus, TaskFlow, TaskFlowStatus, TaskUserAssignment,
        task_priority_from_storage, task_priority_to_storage, task_source_type_from_storage,
        task_source_type_to_storage, task_type_from_storage, task_type_to_storage,
    },
    shared::{
        id::SnowflakeId,
        time::{
            option_timestamp_from_millis, option_timestamp_to_millis, timestamp_from_millis,
            timestamp_to_millis,
        },
    },
};

struct SqliteTaskRepositorySql<'a> {
    connection: &'a mut Connection,
}
pub struct SqliteTaskRepository {
    connection: crate::adapters::outbound::persistence::sqlite::framework_storage::SharedDatabase,
}

struct SqliteTaskReader {
    connection: crate::adapters::outbound::persistence::sqlite::framework_storage::SharedDatabase,
}
impl TaskReadRepository for SqliteTaskReader {
    fn get_task(&self, code: &str) -> Result<Task, TaskError> {
        let code = code.to_owned();
        self.connection.call_read(move|c|c.query_row("SELECT id,task_code,workspace_code,parent_task_code,title,description,execution_result,flow_code,status,previous_status,priority,task_type,source_type,source_url,agent_code,model_code,progress_value,created_at,updated_at,archived_at,owner_assistant_code,due_at,planned_start_at,next_followup_at,work_session_code,schedule_version FROM tasks WHERE task_code=?1",[code],row_to_task).optional().map_err(storage_error)?.ok_or(TaskError::NotFound),||TaskError::StorageFailure)
    }
    fn page(
        &self,
        query: &crate::domain::task::TaskListQuery,
        window: Option<(usize, usize)>,
    ) -> Result<crate::application::task::TaskReadPage, TaskError> {
        let query = query.clone();
        self.connection.call_read(
            move |connection| paging::read_page(connection, &query, window),
            || TaskError::StorageFailure,
        )
    }
    fn list_tasks(&self) -> Result<Vec<Task>, TaskError> {
        self.connection.call_read(
            |connection| SqliteTaskRepositorySql { connection }.list_tasks(),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn page_task_executions(
        &self,
        task_code: &str,
        current: usize,
        size: usize,
    ) -> Result<PagedTaskRecords<TaskExecutionRecord>, TaskError> {
        let task_code = task_code.to_owned();
        self.connection.call_read(
            move |connection| page_task_executions(connection, &task_code, current, size),
            || TaskError::StorageFailure,
        )
    }
    fn relations(
        &self,
        codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, TaskRelations>, TaskError> {
        let codes = codes.to_vec();
        self.connection.call_read(
            move |connection| SqliteTaskRepositorySql { connection }.relations(&codes),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
}

impl<'a> SqliteTaskRepositorySql<'a> {
    pub fn initialize(
        _database_url: &str,
        connection: &'a mut Connection,
    ) -> Result<Self, TaskError> {
        crate::adapters::outbound::persistence::sqlite::schema::initialize_business(connection)
            .map_err(storage_error)?;
        Ok(Self { connection })
    }

    fn list_code_links(
        &mut self,
        table: &str,
        code_column: &str,
        task_code: &str,
    ) -> Result<Vec<TaskCodeLink>, TaskError> {
        let sql = format!(
            "SELECT {code_column}, linked_at FROM {table} WHERE task_code = ?1 ORDER BY {code_column} ASC"
        );
        let mut statement = self.connection.prepare(&sql).map_err(storage_error)?;
        statement
            .query_map([task_code], |row| {
                Ok(TaskCodeLink {
                    code: row.get(0)?,
                    linked_at: timestamp_from_millis(row.get(1)?),
                })
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    fn replace_code_links(
        &mut self,
        table: &str,
        code_column: &str,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<(), TaskError> {
        let mut next_id = self.next_id()?;
        let delete_sql = format!("DELETE FROM {table} WHERE task_code = ?1");
        let insert_sql = format!(
            "INSERT INTO {table} (id, task_code, {code_column}, linked_at) VALUES (?1, ?2, ?3, ?4)"
        );
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self.connection, rusqlite::TransactionBehavior::Immediate).map_err(storage_error)?;
        tx.execute(&delete_sql, [task_code])
            .map_err(storage_error)?;
        for entry in entries {
            tx.execute(
                &insert_sql,
                params![
                    next_id,
                    task_code,
                    entry.code,
                    timestamp_to_millis(&entry.linked_at)
                ],
            )
            .map_err(storage_error)?;
            next_id += 1;
        }
        tx.commit().map_err(storage_error)
    }
    fn read_task_flow(&self, flow_code: &str) -> Result<Option<TaskFlow>, TaskError> {
        let Some((flow_code, name)) = self
            .connection
            .query_row(
                "SELECT flow_code, name FROM task_flows WHERE flow_code = ?1",
                [flow_code],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(storage_error)?
        else {
            return Ok(None);
        };
        let mut statement = self
            .connection
            .prepare(
                "SELECT status_code, name, position, required FROM task_flow_statuses WHERE flow_code = ?1 ORDER BY position ASC",
            )
            .map_err(storage_error)?;
        let statuses = statement
            .query_map([&flow_code], |row| {
                Ok(TaskFlowStatus {
                    status_code: row.get(0)?,
                    name: row.get(1)?,
                    position: row.get::<_, i64>(2)? as usize,
                    required: row.get::<_, i64>(3)? != 0,
                })
            })
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)?;
        Ok(Some(TaskFlow {
            flow_code,
            name,
            statuses,
        }))
    }

    fn replace_task_flow_statuses(&mut self, flow: &TaskFlow) -> Result<(), TaskError> {
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self.connection, rusqlite::TransactionBehavior::Immediate).map_err(storage_error)?;
        tx.execute(
            "DELETE FROM task_flow_statuses WHERE flow_code = ?1",
            [&flow.flow_code],
        )
        .map_err(storage_error)?;
        for status in &flow.statuses {
            tx.execute(
                "INSERT INTO task_flow_statuses (flow_code, status_code, name, position, required) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    flow.flow_code,
                    status.status_code,
                    status.name,
                    status.position as i64,
                    if status.required { 1_i64 } else { 0_i64 },
                ],
            )
            .map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)
    }
}

impl SqliteTaskRepositorySql<'_> {
    fn claim_assistant_task(
        &mut self,
        task_code: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<bool, TaskError> {
        self.connection.execute(
            "UPDATE tasks SET schedule_version = schedule_version + 1, next_followup_at = ?4, updated_at = ?3 WHERE task_code = ?1 AND schedule_version = ?2 AND owner_assistant_code IS NOT NULL AND archived_at IS NULL AND status <> 'CLOSED' AND next_followup_at <= ?3",
            params![task_code, expected_version, now_ms, now_ms.saturating_add(15 * 60 * 1000)],
        ).map(|count| count == 1).map_err(storage_error)
    }

    fn bind_assistant_work_session(
        &mut self,
        task_code: &str,
        session_code: &str,
    ) -> Result<bool, TaskError> {
        self.connection.execute(
            "UPDATE tasks SET work_session_code = ?2 WHERE task_code = ?1 AND owner_assistant_code IS NOT NULL AND archived_at IS NULL AND status <> 'CLOSED' AND (work_session_code IS NULL OR work_session_code = ?2)",
            params![task_code, session_code],
        ).map(|count| count == 1).map_err(storage_error)
    }

    fn begin_task_execution(
        &mut self,
        task: &Task,
        execution: &TaskExecutionRecord,
    ) -> Result<(), TaskError> {
        let transaction = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
            self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(storage_error)?;
        update_task_row(&transaction, task)?;
        transaction
            .execute(
                "INSERT INTO task_execution_records (execution_code, task_code, session_code, status, summary, started_at, finished_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    execution.execution_code,
                    execution.task_code,
                    execution.session_code,
                    task_execution_status_to_storage(execution.status),
                    execution.summary,
                    timestamp_to_millis(&execution.started_at),
                    option_timestamp_to_millis(execution.finished_at.as_deref()),
                    timestamp_to_millis(&execution.created_at),
                ],
            )
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }

    fn finish_task_execution(
        &mut self,
        task: &Task,
        execution_code: &str,
        status: TaskExecutionStatus,
        summary: Option<&str>,
        finished_at: &str,
    ) -> Result<(), TaskError> {
        let transaction = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
            self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(storage_error)?;
        match update_task_row(&transaction, task) {
            Ok(()) | Err(TaskError::InvalidRequest) => {}
            Err(error) => return Err(error),
        }
        let changed = transaction
            .execute(
                "UPDATE task_execution_records SET status = ?2, summary = COALESCE(?3, summary), finished_at = ?4 WHERE execution_code = ?1 AND task_code = ?5",
                params![
                    execution_code,
                    task_execution_status_to_storage(status),
                    summary,
                    timestamp_to_millis(finished_at),
                    task.task_code,
                ],
            )
            .map_err(storage_error)?;
        if changed != 1 {
            return Err(TaskError::InvalidRequest);
        }
        transaction.commit().map_err(storage_error)
    }

    fn list_task_flows(&mut self) -> Result<Vec<TaskFlow>, TaskError> {
        let mut flows = std::collections::BTreeMap::<String, TaskFlow>::new();
        let mut q = self.connection.prepare("SELECT f.flow_code,f.name,s.status_code,s.name,s.position,s.required FROM task_flows f LEFT JOIN task_flow_statuses s ON s.flow_code=f.flow_code ORDER BY f.flow_code,s.position").map_err(storage_error)?;
        let rows = q
            .query_map([], |r| {
                let status = r
                    .get::<_, Option<String>>(2)?
                    .map(|status_code| {
                        Ok::<_, rusqlite::Error>(TaskFlowStatus {
                            status_code,
                            name: r.get(3)?,
                            position: r.get::<_, usize>(4)?,
                            required: r.get::<_, bool>(5)?,
                        })
                    })
                    .transpose()?;
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, status))
            })
            .map_err(storage_error)?;
        for row in rows {
            let (code, name, status) = row.map_err(storage_error)?;
            let flow = flows.entry(code.clone()).or_insert_with(|| TaskFlow {
                flow_code: code,
                name,
                statuses: Vec::new(),
            });
            if let Some(status) = status {
                flow.statuses.push(status);
            }
        }
        Ok(flows.into_values().collect())
    }

    fn get_task_flow(&mut self, flow_code: &str) -> Result<Option<TaskFlow>, TaskError> {
        self.read_task_flow(flow_code)
    }

    fn create_task_flow(&mut self, flow: &TaskFlow) -> Result<(), TaskError> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        self.connection
            .execute(
                "INSERT INTO task_flows (flow_code, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
                params![flow.flow_code, flow.name, timestamp],
            )
            .map_err(storage_error)?;
        self.replace_task_flow_statuses(flow)
    }

    fn update_task_flow(&mut self, flow: &TaskFlow) -> Result<(), TaskError> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        self.connection
            .execute(
                "UPDATE task_flows SET name = ?2, updated_at = ?3 WHERE flow_code = ?1",
                params![flow.flow_code, flow.name, timestamp],
            )
            .map_err(storage_error)?;
        self.replace_task_flow_statuses(flow)
    }

    fn delete_task_flow(&mut self, flow_code: &str) -> Result<(), TaskError> {
        let assigned: bool = self
            .connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspace_task_flows WHERE flow_code = ?1)",
                [flow_code],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        if assigned {
            return Err(TaskError::FlowInUse);
        }
        self.connection
            .execute("DELETE FROM task_flows WHERE flow_code = ?1", [flow_code])
            .map_err(storage_error)?;
        Ok(())
    }

    fn workspace_task_flow_code(
        &mut self,
        workspace_code: &str,
    ) -> Result<Option<String>, TaskError> {
        self.connection
            .query_row(
                "SELECT flow_code FROM workspace_task_flows WHERE workspace_code = ?1",
                [workspace_code],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage_error)
    }

    fn assign_workspace_task_flow(
        &mut self,
        workspace_code: &str,
        flow_code: &str,
    ) -> Result<(), TaskError> {
        self.connection
            .execute(
                "INSERT INTO workspace_task_flows (workspace_code, flow_code, updated_at) VALUES (?1, ?2, ?3) ON CONFLICT(workspace_code) DO UPDATE SET flow_code = excluded.flow_code, updated_at = excluded.updated_at",
                params![workspace_code, flow_code, chrono::Utc::now().timestamp_millis()],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    fn update_workspace_task_flow(
        &mut self,
        workspace_code: &str,
        flow_code: &str,
    ) -> Result<(), TaskError> {
        self.connection
            .execute(
                "UPDATE tasks SET flow_code = ?2, updated_at = ?3, schedule_version = schedule_version + 1 WHERE workspace_code = ?1",
                params![
                    workspace_code,
                    flow_code,
                    chrono::Utc::now().timestamp_millis()
                ],
            )
            .map_err(storage_error)?;
        Ok(())
    }

    fn list_tasks(&mut self) -> Result<Vec<Task>, TaskError> {
        let mut statement = self.connection.prepare(
            "SELECT id, task_code, workspace_code, parent_task_code, title, description, execution_result, flow_code, status, previous_status, priority, task_type, source_type, source_url, agent_code, model_code, progress_value, created_at, updated_at, archived_at, owner_assistant_code, due_at, planned_start_at, next_followup_at, work_session_code, schedule_version FROM tasks ORDER BY id ASC"
        ).map_err(storage_error)?;
        statement
            .query_map([], row_to_task)
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    fn relations(
        &mut self,
        codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, TaskRelations>, TaskError> {
        read_task_relations(self.connection, codes)
    }

    fn next_id(&mut self) -> Result<i64, TaskError> {
        next_task_id(self.connection)
    }

    fn create_task(&mut self, task: &Task) -> Result<(), TaskError> {
        insert_task_row(self.connection, task)
    }

    fn update_task(&mut self, task: &Task) -> Result<(), TaskError> {
        update_task_row(self.connection, task)
    }

    fn delete_task(&mut self, task_code: &str) -> Result<(), TaskError> {
        let transaction = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
            self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )
        .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM task_agent_links WHERE task_code = ?1",
                [task_code],
            )
            .map_err(storage_error)?;
        transaction
            .execute(
                "DELETE FROM task_app_links WHERE task_code = ?1",
                [task_code],
            )
            .map_err(storage_error)?;
        transaction
            .execute("DELETE FROM tasks WHERE task_code = ?1", [task_code])
            .map_err(storage_error)?;
        transaction.commit().map_err(storage_error)
    }

    fn list_user_assignments(
        &mut self,
        _task_code: &str,
    ) -> Result<Vec<TaskUserAssignment>, TaskError> {
        Ok(Vec::new())
    }

    fn replace_user_assignments(
        &mut self,
        _task_code: &str,
        _entries: &[TaskUserAssignment],
    ) -> Result<Vec<TaskUserAssignment>, TaskError> {
        Ok(Vec::new())
    }

    fn list_agent_links(&mut self, task_code: &str) -> Result<Vec<TaskCodeLink>, TaskError> {
        self.list_code_links("task_agent_links", "agent_code", task_code)
    }
    fn replace_agent_links(
        &mut self,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<Vec<TaskCodeLink>, TaskError> {
        self.replace_code_links("task_agent_links", "agent_code", task_code, entries)?;
        self.list_agent_links(task_code)
    }
    fn list_app_links(&mut self, task_code: &str) -> Result<Vec<TaskCodeLink>, TaskError> {
        self.list_code_links("task_app_links", "app_code", task_code)
    }
    fn replace_app_links(
        &mut self,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<Vec<TaskCodeLink>, TaskError> {
        self.replace_code_links("task_app_links", "app_code", task_code, entries)?;
        self.list_app_links(task_code)
    }
    /// One transaction covers the task row and every link table, so a task and
    /// its links commit together instead of one fsync per method.
    fn create_task_with_links(
        &mut self,
        task: &Task,
        _user_assignments: &[TaskUserAssignment],
        agent_links: &[TaskCodeLink],
        app_links: &[TaskCodeLink],
    ) -> Result<(), TaskError> {
        let transaction = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self.connection, rusqlite::TransactionBehavior::Immediate).map_err(storage_error)?;
        insert_task_row(&transaction, task)?;
        let mut next_id = next_task_id(&transaction)?;
        replace_code_links_row(
            &transaction,
            "task_agent_links",
            "agent_code",
            &task.task_code,
            agent_links,
            &mut next_id,
        )?;
        replace_code_links_row(
            &transaction,
            "task_app_links",
            "app_code",
            &task.task_code,
            app_links,
            &mut next_id,
        )?;
        transaction.commit().map_err(storage_error)
    }
    fn update_task_with_links(
        &mut self,
        task: &Task,
        _user_assignments: Option<&[TaskUserAssignment]>,
        agent_links: Option<&[TaskCodeLink]>,
        app_links: Option<&[TaskCodeLink]>,
    ) -> Result<(), TaskError> {
        let transaction = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self.connection, rusqlite::TransactionBehavior::Immediate).map_err(storage_error)?;
        update_task_row(&transaction, task)?;
        if let Some(entries) = agent_links {
            let mut next_id = next_task_id(&transaction)?;
            replace_code_links_row(
                &transaction,
                "task_agent_links",
                "agent_code",
                &task.task_code,
                entries,
                &mut next_id,
            )?;
        }
        if let Some(entries) = app_links {
            let mut next_id = next_task_id(&transaction)?;
            replace_code_links_row(
                &transaction,
                "task_app_links",
                "app_code",
                &task.task_code,
                entries,
                &mut next_id,
            )?;
        }
        transaction.commit().map_err(storage_error)
    }
}

fn next_task_id(connection: &Connection) -> Result<i64, TaskError> {
    let max_id = connection
        .query_row(
            "SELECT max(id) FROM (SELECT id FROM tasks UNION ALL SELECT id FROM task_agent_links UNION ALL SELECT id FROM task_app_links)",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(storage_error)?
        .unwrap_or(734_500_000_000_000_000);
    Ok(max_id + 1)
}

fn insert_task_row(connection: &Connection, task: &Task) -> Result<(), TaskError> {
    connection
        .execute(
            "INSERT INTO tasks (id, task_code, workspace_code, parent_task_code, title, description, execution_result, flow_code, status, previous_status, priority, task_type, source_type, source_url, agent_code, model_code, progress_value, created_at, updated_at, archived_at, owner_assistant_code, due_at, planned_start_at, next_followup_at, work_session_code, schedule_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
            params![task.id.value(), task.task_code, task.workspace_code, task.parent_task_code, task.title, task.description, task.execution_result, task.flow_code, task.status, task.previous_status, task_priority_to_storage(task.priority), task_type_to_storage(task.task_type), task_source_type_to_storage(task.source_type), task.source_url, task.agent_code, task.model_code, i64::from(task.progress), timestamp_to_millis(&task.created_at), timestamp_to_millis(&task.updated_at), option_timestamp_to_millis(task.archived_at.as_deref()), task.owner_assistant_code, task.due_at, task.planned_start_at, task.next_followup_at, task.work_session_code, task.schedule_version],
        )
        .map_err(storage_error)?;
    Ok(())
}

fn update_task_row(connection: &Connection, task: &Task) -> Result<(), TaskError> {
    let changed = connection
        .execute(
            "UPDATE tasks SET parent_task_code = ?2, title = ?3, description = ?4, execution_result = ?5, flow_code = ?6, status = ?7, previous_status = ?8, priority = ?9, task_type = ?10, source_type = ?11, source_url = ?12, agent_code = ?13, model_code = ?14, progress_value = ?15, updated_at = ?16, archived_at = ?17, due_at = ?18, planned_start_at = ?19, next_followup_at = ?20, work_session_code = ?21, schedule_version = schedule_version + 1 WHERE task_code = ?1 AND schedule_version = ?22",
            params![task.task_code, task.parent_task_code, task.title, task.description, task.execution_result, task.flow_code, task.status, task.previous_status, task_priority_to_storage(task.priority), task_type_to_storage(task.task_type), task_source_type_to_storage(task.source_type), task.source_url, task.agent_code, task.model_code, i64::from(task.progress), timestamp_to_millis(&task.updated_at), option_timestamp_to_millis(task.archived_at.as_deref()), task.due_at, task.planned_start_at, task.next_followup_at, task.work_session_code, task.schedule_version],
        )
        .map_err(storage_error)?;
    if changed == 0 {
        return Err(TaskError::InvalidRequest);
    }
    Ok(())
}

/// Delete and re-insert link rows inside an already open transaction.
/// `next_id` carries the id allocation across multiple link tables.
fn replace_code_links_row(
    connection: &Connection,
    table: &str,
    code_column: &str,
    task_code: &str,
    entries: &[TaskCodeLink],
    next_id: &mut i64,
) -> Result<(), TaskError> {
    let delete_sql = format!("DELETE FROM {table} WHERE task_code = ?1");
    let insert_sql = format!(
        "INSERT INTO {table} (id, task_code, {code_column}, linked_at) VALUES (?1, ?2, ?3, ?4)"
    );
    connection
        .execute(&delete_sql, [task_code])
        .map_err(storage_error)?;
    for entry in entries {
        connection
            .execute(
                &insert_sql,
                params![
                    *next_id,
                    task_code,
                    entry.code,
                    timestamp_to_millis(&entry.linked_at)
                ],
            )
            .map_err(storage_error)?;
        *next_id += 1;
    }
    Ok(())
}

fn page_task_executions(
    connection: &Connection,
    task_code: &str,
    current: usize,
    size: usize,
) -> Result<PagedTaskRecords<TaskExecutionRecord>, TaskError> {
    let total = connection
        .query_row(
            "SELECT COUNT(*) FROM task_execution_records WHERE task_code = ?1",
            [task_code],
            |row| row.get::<_, usize>(0),
        )
        .map_err(storage_error)?;
    let offset = current.saturating_sub(1).saturating_mul(size);
    let mut statement = connection
        .prepare(
            "SELECT execution_code, task_code, session_code, status, summary, started_at, finished_at, created_at FROM task_execution_records WHERE task_code = ?1 ORDER BY started_at DESC, execution_code DESC LIMIT ?2 OFFSET ?3",
        )
        .map_err(storage_error)?;
    let records = statement
        .query_map(
            params![task_code, size as i64, offset as i64],
            row_to_task_execution,
        )
        .map_err(storage_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage_error)?;
    Ok(PagedTaskRecords {
        current,
        size,
        total,
        records,
    })
}

fn row_to_task_execution(row: &Row<'_>) -> rusqlite::Result<TaskExecutionRecord> {
    let status: String = row.get(3)?;
    let status = match status.as_str() {
        "Running" => TaskExecutionStatus::Running,
        "Success" => TaskExecutionStatus::Success,
        "Failed" => TaskExecutionStatus::Failed,
        _ => return Err(task_mapping_error(TaskError::InvalidRequest)),
    };
    let started_at: i64 = row.get(5)?;
    let finished_at: Option<i64> = row.get(6)?;
    let created_at: i64 = row.get(7)?;
    Ok(TaskExecutionRecord {
        execution_code: row.get(0)?,
        task_code: row.get(1)?,
        session_code: row.get(2)?,
        status,
        summary: row.get(4)?,
        started_at: timestamp_from_millis(started_at),
        finished_at: option_timestamp_from_millis(finished_at),
        created_at: timestamp_from_millis(created_at),
    })
}

fn task_execution_status_to_storage(status: TaskExecutionStatus) -> &'static str {
    match status {
        TaskExecutionStatus::Running => "Running",
        TaskExecutionStatus::Success => "Success",
        TaskExecutionStatus::Failed => "Failed",
    }
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let priority: String = row.get(10)?;
    let task_type: String = row.get(11)?;
    let source_type: String = row.get(12)?;
    let progress: i64 = row.get(16)?;
    Ok(Task {
        id: SnowflakeId::new(row.get(0)?),
        task_code: row.get(1)?,
        workspace_code: row.get(2)?,
        owner_assistant_code: row.get(20)?,
        due_at: row.get(21)?,
        planned_start_at: row.get(22)?,
        next_followup_at: row.get(23)?,
        work_session_code: row.get(24)?,
        schedule_version: row.get(25)?,
        parent_task_code: row.get(3)?,
        title: row.get(4)?,
        description: row.get(5)?,
        execution_result: row.get(6)?,
        flow_code: row.get(7)?,
        status: row.get(8)?,
        previous_status: row.get(9)?,
        priority: task_priority_from_storage(&priority).map_err(task_mapping_error)?,
        task_type: task_type_from_storage(&task_type).map_err(task_mapping_error)?,
        source_type: task_source_type_from_storage(&source_type).map_err(task_mapping_error)?,
        source_url: row.get(13)?,
        agent_code: row.get(14)?,
        model_code: row.get(15)?,
        progress: progress.clamp(0, 100) as u8,
        created_at: timestamp_from_millis(row.get(17)?),
        updated_at: timestamp_from_millis(row.get(18)?),
        archived_at: option_timestamp_from_millis(row.get::<_, Option<i64>>(19)?),
    })
}

fn storage_error(error: rusqlite::Error) -> TaskError {
    tracing::error!("task sqlite error: {error}");
    TaskError::StorageFailure
}

fn task_mapping_error(error: TaskError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

impl SqliteTaskRepository {
    pub fn connect(database_url: &str) -> Result<Self, TaskError> {
        let connection = crate::adapters::outbound::persistence::sqlite::framework_storage::SharedDatabase::open(database_url).map_err(storage_error)?;
        let database_url = database_url.to_owned();
        connection.call(
            move |connection| {
                SqliteTaskRepositorySql::initialize(&database_url, connection).map(|_| ())
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )?;
        Ok(Self { connection })
    }
}
impl TaskRepository for SqliteTaskRepository {
    fn reader(&self) -> std::sync::Arc<dyn TaskReadRepository> {
        std::sync::Arc::new(SqliteTaskReader {
            connection: self.connection.clone(),
        })
    }
    fn prune_finished_assistant_tasks(&mut self, cutoff_ms: i64) -> Result<usize, TaskError> {
        self.connection.call(
            move |connection| {
                let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
                    connection,
                    rusqlite::TransactionBehavior::Immediate,
                )
                .map_err(storage_error)?;
                let mut deleted = 0;
                loop {
                    let codes = {
                        let mut query = tx.prepare(
                            "SELECT task_code FROM tasks t WHERE owner_assistant_code IS NOT NULL \
                             AND (status = ?1 OR archived_at IS NOT NULL) AND updated_at <= ?2 \
                             AND NOT EXISTS (SELECT 1 FROM tasks child WHERE child.parent_task_code = t.task_code)",
                        ).map_err(storage_error)?;
                        let rows = query
                            .query_map(
                                params![crate::domain::task::CLOSED_STATUS_CODE, cutoff_ms],
                                |row| row.get::<_, String>(0),
                            )
                            .map_err(storage_error)?;
                        rows.collect::<Result<Vec<_>, _>>().map_err(storage_error)?
                    };
                    if codes.is_empty() {
                        break;
                    }
                    for code in codes {
                        tx.execute("DELETE FROM task_agent_links WHERE task_code = ?1", [&code])
                            .map_err(storage_error)?;
                        tx.execute("DELETE FROM task_app_links WHERE task_code = ?1", [&code])
                            .map_err(storage_error)?;
                        deleted += tx.execute("DELETE FROM tasks WHERE task_code = ?1", [&code])
                            .map_err(storage_error)?;
                    }
                }
                tx.commit().map_err(storage_error)?;
                Ok(deleted)
            },
            || TaskError::StorageFailure,
        )
    }
    fn claim_assistant_task(
        &mut self,
        task_code: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<bool, TaskError> {
        let task_code = task_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.claim_assistant_task(
                    &task_code,
                    expected_version,
                    now_ms,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }

    fn bind_assistant_work_session(
        &mut self,
        task_code: &str,
        session_code: &str,
    ) -> Result<bool, TaskError> {
        let task_code = task_code.to_owned();
        let session_code = session_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }
                    .bind_assistant_work_session(&task_code, &session_code)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }

    fn begin_task_execution(
        &mut self,
        task: &Task,
        execution: &TaskExecutionRecord,
    ) -> Result<(), TaskError> {
        let task = task.clone();
        let execution = execution.clone();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }
                    .begin_task_execution(&task, &execution)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }

    fn finish_task_execution(
        &mut self,
        task: &Task,
        execution_code: &str,
        status: TaskExecutionStatus,
        summary: Option<&str>,
        finished_at: &str,
    ) -> Result<(), TaskError> {
        let task = task.clone();
        let execution_code = execution_code.to_owned();
        let summary = summary.map(str::to_owned);
        let finished_at = finished_at.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.finish_task_execution(
                    &task,
                    &execution_code,
                    status,
                    summary.as_deref(),
                    &finished_at,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }

    fn list_task_flows(&mut self) -> Result<Vec<TaskFlow>, TaskError> {
        self.connection.call_read(
            move |connection| SqliteTaskRepositorySql { connection }.list_task_flows(),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn get_task_flow(&mut self, flow_code: &str) -> Result<Option<TaskFlow>, TaskError> {
        let flow_code = flow_code.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.get_task_flow(&flow_code),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn create_task_flow(&mut self, flow: &TaskFlow) -> Result<(), TaskError> {
        let flow = flow.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.create_task_flow(&flow),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn update_task_flow(&mut self, flow: &TaskFlow) -> Result<(), TaskError> {
        let flow = flow.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.update_task_flow(&flow),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn delete_task_flow(&mut self, flow_code: &str) -> Result<(), TaskError> {
        let flow_code = flow_code.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.delete_task_flow(&flow_code),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn workspace_task_flow_code(
        &mut self,
        workspace_code: &str,
    ) -> Result<Option<String>, TaskError> {
        let workspace_code = workspace_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.workspace_task_flow_code(&workspace_code)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn assign_workspace_task_flow(
        &mut self,
        workspace_code: &str,
        flow_code: &str,
    ) -> Result<(), TaskError> {
        let workspace_code = workspace_code.to_owned();
        let flow_code = flow_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }
                    .assign_workspace_task_flow(&workspace_code, &flow_code)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn update_workspace_task_flow(
        &mut self,
        workspace_code: &str,
        flow_code: &str,
    ) -> Result<(), TaskError> {
        let workspace_code = workspace_code.to_owned();
        let flow_code = flow_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }
                    .update_workspace_task_flow(&workspace_code, &flow_code)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn list_tasks(&mut self) -> Result<Vec<Task>, TaskError> {
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.list_tasks(),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn relations(
        &mut self,
        codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, TaskRelations>, TaskError> {
        let codes = codes.to_vec();
        self.connection.call_read(
            move |connection| SqliteTaskRepositorySql { connection }.relations(&codes),
            || TaskError::StorageFailure,
        )
    }
    fn next_id(&mut self) -> Result<i64, TaskError> {
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.next_id(),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn create_task(&mut self, task: &Task) -> Result<(), TaskError> {
        let task = task.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.create_task(&task),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn update_task(&mut self, task: &Task) -> Result<(), TaskError> {
        let task = task.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.update_task(&task),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn delete_task(&mut self, task_code: &str) -> Result<(), TaskError> {
        let task_code = task_code.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.delete_task(&task_code),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn create_task_with_links(
        &mut self,
        task: &Task,
        user_assignments: &[TaskUserAssignment],
        agent_links: &[TaskCodeLink],
        app_links: &[TaskCodeLink],
    ) -> Result<(), TaskError> {
        let task = task.to_owned();
        let user_assignments = user_assignments.to_owned();
        let agent_links = agent_links.to_owned();
        let app_links = app_links.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.create_task_with_links(
                    &task,
                    &user_assignments,
                    &agent_links,
                    &app_links,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn update_task_with_links(
        &mut self,
        task: &Task,
        user_assignments: Option<&[TaskUserAssignment]>,
        agent_links: Option<&[TaskCodeLink]>,
        app_links: Option<&[TaskCodeLink]>,
    ) -> Result<(), TaskError> {
        let task = task.to_owned();
        let user_assignments = user_assignments.map(<[TaskUserAssignment]>::to_owned);
        let agent_links = agent_links.map(<[TaskCodeLink]>::to_owned);
        let app_links = app_links.map(<[TaskCodeLink]>::to_owned);
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.update_task_with_links(
                    &task,
                    user_assignments.as_deref(),
                    agent_links.as_deref(),
                    app_links.as_deref(),
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn list_user_assignments(
        &mut self,
        _task_code: &str,
    ) -> Result<Vec<TaskUserAssignment>, TaskError> {
        let _task_code = _task_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.list_user_assignments(&_task_code)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn replace_user_assignments(
        &mut self,
        _task_code: &str,
        _entries: &[TaskUserAssignment],
    ) -> Result<Vec<TaskUserAssignment>, TaskError> {
        let _task_code = _task_code.to_owned();
        let _entries = _entries.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }
                    .replace_user_assignments(&_task_code, &_entries)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn list_agent_links(&mut self, task_code: &str) -> Result<Vec<TaskCodeLink>, TaskError> {
        let task_code = task_code.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.list_agent_links(&task_code),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn replace_agent_links(
        &mut self,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<Vec<TaskCodeLink>, TaskError> {
        let task_code = task_code.to_owned();
        let entries = entries.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.replace_agent_links(&task_code, &entries)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn list_app_links(&mut self, task_code: &str) -> Result<Vec<TaskCodeLink>, TaskError> {
        let task_code = task_code.to_owned();
        self.connection.call(
            move |connection| SqliteTaskRepositorySql { connection }.list_app_links(&task_code),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn replace_app_links(
        &mut self,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<Vec<TaskCodeLink>, TaskError> {
        let task_code = task_code.to_owned();
        let entries = entries.to_owned();
        self.connection.call(
            move |connection| {
                SqliteTaskRepositorySql { connection }.replace_app_links(&task_code, &entries)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
}

fn read_task_relations(
    connection: &Connection,
    codes: &[String],
) -> Result<std::collections::BTreeMap<String, TaskRelations>, TaskError> {
    let mut result = std::collections::BTreeMap::<String, TaskRelations>::new();
    if codes.is_empty() {
        return Ok(result);
    }
    let selected = serde_json::to_string(codes).map_err(|_| TaskError::StorageFailure)?;
    for (table, column, agents) in [
        ("task_agent_links", "agent_code", true),
        ("task_app_links", "app_code", false),
    ] {
        let mut q = connection.prepare(&format!("SELECT task_code,{column},linked_at FROM {table} WHERE task_code IN (SELECT value FROM json_each(?1)) ORDER BY task_code,{column}")).map_err(storage_error)?;
        let rows = q
            .query_map([&selected], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    TaskCodeLink {
                        code: r.get(1)?,
                        linked_at: timestamp_from_millis(r.get(2)?),
                    },
                ))
            })
            .map_err(storage_error)?;
        for row in rows {
            let (task, link) = row.map_err(storage_error)?;
            let relations = result.entry(task).or_default();
            if agents {
                relations.agents.push(link);
            } else {
                relations.apps.push(link);
            }
        }
    }
    Ok(result)
}
