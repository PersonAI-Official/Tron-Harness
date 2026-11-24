use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};

use crate::{
    application::automation::AutomationRepository,
    domain::automation::{
        Automation, AutomationConcurrencyMode, AutomationError, AutomationExecutionDraft,
        AutomationExecutionRecord, AutomationExecutionStats, AutomationExecutionStatus,
        automation_concurrency_mode_from_storage, automation_concurrency_mode_to_storage,
        automation_execution_status_to_storage, automation_status_to_storage,
        automation_trigger_source_to_storage,
    },
    shared::id::SnowflakeId,
};

use super::automation_repository_helpers::{
    next_id_in_transaction, row_to_automation, row_to_execution_record, storage_error,
    timestamp_to_millis, update_automation_in_transaction,
};

struct SqliteAutomationRepositorySql<'a> {
    connection: &'a mut Connection,
}
pub struct SqliteAutomationRepository {
    connection: crate::adapters::outbound::persistence::sqlite::framework_storage::SharedDatabase,
}

impl<'a> SqliteAutomationRepositorySql<'a> {
    pub fn initialize(
        _database_url: &str,
        connection: &'a mut Connection,
    ) -> Result<Self, AutomationError> {
        crate::adapters::outbound::persistence::sqlite::schema::initialize_business(connection)
            .map_err(storage_error)?;
        Ok(Self { connection })
    }
}

impl AutomationRepository for SqliteAutomationRepositorySql<'_> {
    fn list_automations(&mut self) -> Result<Vec<Automation>, AutomationError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, automation_code, workspace_code, name, description, agent_code, model_code, prompt_text, cron_expression, scheduler_expression, timezone, status, previous_status, last_execution_status, last_execution_code, last_execution_summary, last_started_at, last_finished_at, created_at, updated_at, archived_at, concurrency_mode FROM automations ORDER BY id ASC",
            )
            .map_err(storage_error)?;
        statement
            .query_map([], row_to_automation)
            .map_err(storage_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage_error)
    }

    fn execution_stats(
        &mut self,
        automation_codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, AutomationExecutionStats>, AutomationError> {
        if automation_codes.is_empty() {
            return Ok(std::collections::BTreeMap::new());
        }
        let placeholders = std::iter::repeat_n("?", automation_codes.len())
            .collect::<Vec<_>>()
            .join(", ");
        let query = format!(
            "SELECT automation_code,
                    SUM(CASE WHEN status IN ('Success', 'Failed', 'Canceled', 'Skipped') THEN 1 ELSE 0 END) AS total_count,
                    SUM(CASE WHEN status = 'Success' THEN 1 ELSE 0 END) AS success_count
             FROM automation_execution_records
             WHERE automation_code IN ({placeholders})
             GROUP BY automation_code"
        );
        let mut statement = self.connection.prepare(&query).map_err(storage_error)?;
        statement
            .query_map(
                params_from_iter(automation_codes.iter().map(String::as_str)),
                |row| {
                    let automation_code: String = row.get(0)?;
                    let total_count: i64 = row.get(1)?;
                    let success_count: i64 = row.get(2)?;
                    Ok((
                        automation_code,
                        AutomationExecutionStats {
                            success_count: success_count.max(0) as u64,
                            total_count: total_count.max(0) as u64,
                        },
                    ))
                },
            )
            .map_err(storage_error)?
            .collect::<Result<std::collections::BTreeMap<_, _>, _>>()
            .map_err(storage_error)
    }

    fn next_id(&mut self) -> Result<i64, AutomationError> {
        let max_id = self
            .connection
            .query_row(
                "SELECT max(id) FROM (SELECT id FROM automations UNION ALL SELECT id FROM automation_execution_records)",
                [],
                |row| row.get::<_, Option<i64>>(0),
            )
            .map_err(storage_error)?
            .unwrap_or(734_600_000_000_000_000);
        Ok(max_id + 1)
    }

    fn create_automation(&mut self, automation: &Automation) -> Result<(), AutomationError> {
        self.connection.execute(
            "INSERT INTO automations (id, automation_code, workspace_code, name, description, agent_code, model_code, prompt_text, cron_expression, scheduler_expression, timezone, status, previous_status, last_execution_status, last_execution_code, last_execution_summary, last_started_at, last_finished_at, created_at, updated_at, archived_at, concurrency_mode) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            params![
                automation.id.value(),
                automation.automation_code,
                automation.workspace_code,
                automation.name,
                automation.description,
                automation.agent_code,
                automation.model_code,
                automation.prompt_text,
                automation.cron_expression,
                automation.scheduler_expression,
                automation.timezone,
                automation_status_to_storage(automation.status),
                automation.previous_status.map(automation_status_to_storage),
                automation.last_execution_status.map(automation_execution_status_to_storage),
                automation.last_execution_code,
                automation.last_execution_summary,
                automation.last_started_at.as_deref().map(timestamp_to_millis),
                automation.last_finished_at.as_deref().map(timestamp_to_millis),
                timestamp_to_millis(&automation.created_at),
                timestamp_to_millis(&automation.updated_at),
                automation.archived_at.as_deref().map(timestamp_to_millis),
                automation_concurrency_mode_to_storage(automation.concurrency_mode),
            ],
        ).map_err(storage_error)?;
        Ok(())
    }

    fn update_automation(&mut self, automation: &Automation) -> Result<(), AutomationError> {
        self.connection.execute(
            "UPDATE automations SET workspace_code = ?2, name = ?3, description = ?4, agent_code = ?5, model_code = ?6, prompt_text = ?7, cron_expression = ?8, scheduler_expression = ?9, timezone = ?10, status = ?11, previous_status = ?12, last_execution_status = ?13, last_execution_code = ?14, last_execution_summary = ?15, last_started_at = ?16, last_finished_at = ?17, updated_at = ?18, archived_at = ?19, concurrency_mode = ?20 WHERE automation_code = ?1",
            params![
                automation.automation_code,
                automation.workspace_code,
                automation.name,
                automation.description,
                automation.agent_code,
                automation.model_code,
                automation.prompt_text,
                automation.cron_expression,
                automation.scheduler_expression,
                automation.timezone,
                automation_status_to_storage(automation.status),
                automation.previous_status.map(automation_status_to_storage),
                automation.last_execution_status.map(automation_execution_status_to_storage),
                automation.last_execution_code,
                automation.last_execution_summary,
                automation.last_started_at.as_deref().map(timestamp_to_millis),
                automation.last_finished_at.as_deref().map(timestamp_to_millis),
                timestamp_to_millis(&automation.updated_at),
                automation.archived_at.as_deref().map(timestamp_to_millis),
                automation_concurrency_mode_to_storage(automation.concurrency_mode),
            ],
        ).map_err(storage_error)?;
        Ok(())
    }

    fn list_execution_records(
        &mut self,
        automation_code: &str,
    ) -> Result<Vec<AutomationExecutionRecord>, AutomationError> {
        let records = {
            let mut statement = self.connection.prepare(
                "SELECT id, execution_code, automation_code, workspace_code, session_code, trigger_source, status, summary, scheduled_at, started_at, finished_at, created_at FROM automation_execution_records WHERE automation_code = ?1 ORDER BY started_at DESC, id DESC"
            ).map_err(storage_error)?;
            statement
                .query_map([automation_code], row_to_execution_record)
                .map_err(storage_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage_error)?
        };
        Ok(records)
    }

    fn record_execution(
        &mut self,
        automation: &Automation,
        draft: &AutomationExecutionDraft,
    ) -> Result<Option<AutomationExecutionRecord>, AutomationError> {
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self
            .connection, TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        let next_id = next_id_in_transaction(&tx)?;
        let record = AutomationExecutionRecord {
            id: SnowflakeId::new(next_id),
            execution_code: format!("AEX-{next_id}"),
            automation_code: draft.automation_code.clone(),
            workspace_code: draft.workspace_code.clone(),
            session_code: draft.session_code.clone(),
            trigger_source: draft.trigger_source,
            status: draft.status,
            summary: draft.summary.clone(),
            scheduled_at: draft.scheduled_at.clone(),
            started_at: draft.started_at.clone(),
            finished_at: draft.finished_at.clone(),
            created_at: draft.created_at.clone(),
        };
        let inserted = tx.execute(
            "INSERT OR IGNORE INTO automation_execution_records (id, execution_code, automation_code, workspace_code, session_code, trigger_source, status, summary, scheduled_at, started_at, finished_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.id.value(),
                record.execution_code,
                record.automation_code,
                record.workspace_code,
                record.session_code,
                automation_trigger_source_to_storage(record.trigger_source),
                automation_execution_status_to_storage(record.status),
                record.summary,
                record.scheduled_at.as_deref().map(timestamp_to_millis),
                timestamp_to_millis(&record.started_at),
                record.finished_at.as_deref().map(timestamp_to_millis),
                timestamp_to_millis(&record.created_at),
            ],
        ).map_err(storage_error)?;
        if inserted == 0 {
            tx.commit().map_err(storage_error)?;
            return Ok(None);
        }

        let mut updated = automation.clone();
        updated.last_execution_status = Some(record.status);
        updated.last_execution_code = Some(record.execution_code.clone());
        updated.last_execution_summary = record.summary.clone();
        updated.last_started_at = Some(record.started_at.clone());
        updated.last_finished_at = record.finished_at.clone();
        updated.updated_at = draft.created_at.clone();
        update_automation_in_transaction(&tx, &updated)?;
        tx.commit().map_err(storage_error)?;
        Ok(Some(record))
    }

    fn link_execution_session(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        session_code: &str,
    ) -> Result<(), AutomationError> {
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
            &self.connection,
            TransactionBehavior::Immediate,
        )
        .map_err(storage_error)?;
        let changed = tx
            .execute(
                "UPDATE automation_execution_records
                 SET session_code = ?3
                 WHERE automation_code = ?1 AND execution_code = ?2 AND status = ?4",
                params![
                    automation_code,
                    execution_code,
                    session_code,
                    automation_execution_status_to_storage(AutomationExecutionStatus::Running),
                ],
            )
            .map_err(storage_error)?;
        if changed == 0 {
            tx.commit().map_err(storage_error)?;
            return Err(AutomationError::NotFound);
        }
        tx.commit().map_err(storage_error)?;
        Ok(())
    }

    fn recover_interrupted_executions(
        &mut self,
        finished_at: &str,
    ) -> Result<usize, AutomationError> {
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
            &self.connection,
            TransactionBehavior::Immediate,
        )
        .map_err(storage_error)?;
        let running_status =
            automation_execution_status_to_storage(AutomationExecutionStatus::Running);
        let canceled_status =
            automation_execution_status_to_storage(AutomationExecutionStatus::Canceled);
        let running_records = {
            let mut statement = tx
                .prepare(
                    "SELECT automation_code, execution_code
                     FROM automation_execution_records
                     WHERE status = ?1",
                )
                .map_err(storage_error)?;
            statement
                .query_map([running_status], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(storage_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage_error)?
        };
        let finished_at_millis = timestamp_to_millis(finished_at);
        for (automation_code, execution_code) in &running_records {
            let changed = tx
                .execute(
                    "UPDATE automation_execution_records
                     SET status = ?3, summary = NULL, finished_at = ?4
                     WHERE automation_code = ?1 AND execution_code = ?2 AND status = ?5",
                    params![
                        automation_code,
                        execution_code,
                        canceled_status,
                        finished_at_millis,
                        running_status,
                    ],
                )
                .map_err(storage_error)?;
            if changed > 0 {
                tx.execute(
                    "UPDATE automations
                     SET last_execution_status = ?3,
                         last_execution_summary = NULL,
                         last_finished_at = ?4,
                         updated_at = ?4
                     WHERE automation_code = ?1 AND last_execution_code = ?2",
                    params![
                        automation_code,
                        execution_code,
                        canceled_status,
                        finished_at_millis,
                    ],
                )
                .map_err(storage_error)?;
            }
        }
        tx.commit().map_err(storage_error)?;
        Ok(running_records.len())
    }

    fn prepare_serial_execution(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        finished_at: &str,
    ) -> Result<Option<Vec<AutomationExecutionRecord>>, AutomationError> {
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(
            &self.connection,
            TransactionBehavior::Immediate,
        )
        .map_err(storage_error)?;
        let current = tx
            .query_row(
                "SELECT id, execution_code, automation_code, workspace_code, session_code, trigger_source, status, summary, scheduled_at, started_at, finished_at, created_at
                 FROM automation_execution_records
                 WHERE automation_code = ?1 AND execution_code = ?2",
                params![automation_code, execution_code],
                row_to_execution_record,
            )
            .optional()
            .map_err(storage_error)?;
        let Some(current) =
            current.filter(|record| record.status == AutomationExecutionStatus::Running)
        else {
            tx.commit().map_err(storage_error)?;
            return Ok(None);
        };
        let current_id = current.id.value();
        let configured_mode = tx
            .query_row(
                "SELECT concurrency_mode FROM automations WHERE automation_code = ?1",
                [automation_code],
                |row| row.get::<_, String>(0),
            )
            .map_err(storage_error)?;
        if automation_concurrency_mode_from_storage(&configured_mode)
            .map_err(|_| AutomationError::StorageFailure)?
            != AutomationConcurrencyMode::Serial
        {
            tx.commit().map_err(storage_error)?;
            return Ok(Some(Vec::new()));
        }
        let has_newer_execution: bool = tx
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM automation_execution_records
                    WHERE automation_code = ?1 AND id > ?2
                 )",
                params![automation_code, current_id],
                |row| row.get(0),
            )
            .map_err(storage_error)?;
        let finished_at_millis = timestamp_to_millis(finished_at);
        let canceled_status =
            automation_execution_status_to_storage(AutomationExecutionStatus::Canceled);
        if has_newer_execution {
            tx.execute(
                "UPDATE automation_execution_records
                 SET status = ?3, summary = NULL, finished_at = ?4
                 WHERE automation_code = ?1 AND execution_code = ?2 AND status = ?5",
                params![
                    automation_code,
                    execution_code,
                    canceled_status,
                    finished_at_millis,
                    automation_execution_status_to_storage(AutomationExecutionStatus::Running),
                ],
            )
            .map_err(storage_error)?;
            tx.commit().map_err(storage_error)?;
            return Ok(None);
        }

        let previous_running = {
            let mut statement = tx
                .prepare(
                    "SELECT id, execution_code, automation_code, workspace_code, session_code, trigger_source, status, summary, scheduled_at, started_at, finished_at, created_at
                     FROM automation_execution_records
                     WHERE automation_code = ?1 AND id < ?2 AND status = ?3
                     ORDER BY id DESC",
                )
                .map_err(storage_error)?;
            statement
                .query_map(
                    params![
                        automation_code,
                        current_id,
                        automation_execution_status_to_storage(AutomationExecutionStatus::Running),
                    ],
                    row_to_execution_record,
                )
                .map_err(storage_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage_error)?
        };
        for previous in &previous_running {
            tx.execute(
                "UPDATE automation_execution_records
                 SET status = ?3, summary = NULL, finished_at = ?4
                 WHERE automation_code = ?1 AND execution_code = ?2 AND status = ?5",
                params![
                    automation_code,
                    previous.execution_code,
                    canceled_status,
                    finished_at_millis,
                    automation_execution_status_to_storage(AutomationExecutionStatus::Running),
                ],
            )
            .map_err(storage_error)?;
            tx.execute(
                "UPDATE automations
                 SET last_execution_status = ?3,
                     last_execution_summary = NULL,
                     last_finished_at = ?4,
                     updated_at = ?4
                 WHERE automation_code = ?1 AND last_execution_code = ?2",
                params![
                    automation_code,
                    previous.execution_code,
                    canceled_status,
                    finished_at_millis,
                ],
            )
            .map_err(storage_error)?;
        }
        tx.commit().map_err(storage_error)?;
        Ok(Some(previous_running))
    }

    fn complete_execution_record(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        status: AutomationExecutionStatus,
        summary: Option<String>,
        session_code: Option<String>,
        finished_at: &str,
    ) -> Result<(), AutomationError> {
        let effective_summary = summary;
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self
            .connection, TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        let changed = tx
            .execute(
                "UPDATE automation_execution_records
                 SET status = ?3, summary = ?4, session_code = COALESCE(?5, session_code), finished_at = ?6
                 WHERE automation_code = ?1 AND execution_code = ?2 AND status = ?7",
                params![
                    automation_code,
                    execution_code,
                    automation_execution_status_to_storage(status),
                    effective_summary.as_deref(),
                    session_code.as_deref(),
                    timestamp_to_millis(finished_at),
                    automation_execution_status_to_storage(AutomationExecutionStatus::Running),
                ],
            )
            .map_err(storage_error)?;
        if changed == 0 {
            tx.commit().map_err(storage_error)?;
            return Err(AutomationError::NotFound);
        }
        tx.execute(
            "UPDATE automations
             SET last_execution_status = ?3,
                 last_execution_summary = ?4,
                 last_finished_at = ?5,
                 updated_at = ?5
             WHERE automation_code = ?1 AND last_execution_code = ?2",
            params![
                automation_code,
                execution_code,
                automation_execution_status_to_storage(status),
                effective_summary.as_deref(),
                timestamp_to_millis(finished_at),
            ],
        )
        .map_err(storage_error)?;
        tx.commit().map_err(storage_error)?;
        Ok(())
    }

    fn delete_execution_record(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        deleted_at: &str,
    ) -> Result<AutomationExecutionRecord, AutomationError> {
        let tx = crate::adapters::outbound::persistence::sqlite::framework_storage::business_transaction(&self
            .connection, TransactionBehavior::Immediate)
            .map_err(storage_error)?;
        let deleted = tx
            .query_row(
                "SELECT id, execution_code, automation_code, workspace_code, session_code, trigger_source, status, summary, scheduled_at, started_at, finished_at, created_at
                 FROM automation_execution_records
                 WHERE automation_code = ?1 AND execution_code = ?2",
                params![automation_code, execution_code],
                row_to_execution_record,
            )
            .optional()
            .map_err(storage_error)?
            .ok_or(AutomationError::NotFound)?;

        tx.execute(
            "DELETE FROM automation_execution_records WHERE automation_code = ?1 AND execution_code = ?2",
            params![automation_code, execution_code],
        )
        .map_err(storage_error)?;

        let latest = tx
            .query_row(
                "SELECT id, execution_code, automation_code, workspace_code, session_code, trigger_source, status, summary, scheduled_at, started_at, finished_at, created_at
                 FROM automation_execution_records
                 WHERE automation_code = ?1
                 ORDER BY started_at DESC, id DESC
                 LIMIT 1",
                params![automation_code],
                row_to_execution_record,
            )
            .optional()
            .map_err(storage_error)?;

        match latest {
            Some(record) => {
                tx.execute(
                    "UPDATE automations
                     SET last_execution_status = ?2,
                         last_execution_code = ?3,
                         last_execution_summary = ?4,
                         last_started_at = ?5,
                         last_finished_at = ?6,
                         updated_at = ?7
                     WHERE automation_code = ?1",
                    params![
                        automation_code,
                        automation_execution_status_to_storage(record.status),
                        record.execution_code,
                        record.summary,
                        timestamp_to_millis(&record.started_at),
                        record.finished_at.as_deref().map(timestamp_to_millis),
                        timestamp_to_millis(deleted_at),
                    ],
                )
                .map_err(storage_error)?;
            }
            None => {
                tx.execute(
                    "UPDATE automations
                     SET last_execution_status = NULL,
                         last_execution_code = NULL,
                         last_execution_summary = NULL,
                         last_started_at = NULL,
                         last_finished_at = NULL,
                         updated_at = ?2
                     WHERE automation_code = ?1",
                    params![automation_code, timestamp_to_millis(deleted_at)],
                )
                .map_err(storage_error)?;
            }
        }

        tx.commit().map_err(storage_error)?;
        Ok(deleted)
    }
}

impl SqliteAutomationRepository {
    pub fn connect(database_url: &str) -> Result<Self, AutomationError> {
        let connection = crate::adapters::outbound::persistence::sqlite::framework_storage::SharedDatabase::open(database_url).map_err(storage_error)?;
        let database_url = database_url.to_owned();
        connection.call(
            move |connection| {
                SqliteAutomationRepositorySql::initialize(&database_url, connection).map(|_| ())
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )?;
        Ok(Self { connection })
    }
}
impl AutomationRepository for SqliteAutomationRepository {
    fn list_automations(&mut self) -> Result<Vec<Automation>, AutomationError> {
        self.connection.call(
            move |connection| SqliteAutomationRepositorySql { connection }.list_automations(),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn execution_stats(
        &mut self,
        automation_codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, AutomationExecutionStats>, AutomationError> {
        let automation_codes = automation_codes.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.execution_stats(&automation_codes)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn next_id(&mut self) -> Result<i64, AutomationError> {
        self.connection.call(
            move |connection| SqliteAutomationRepositorySql { connection }.next_id(),
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn create_automation(&mut self, automation: &Automation) -> Result<(), AutomationError> {
        let automation = automation.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.create_automation(&automation)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn update_automation(&mut self, automation: &Automation) -> Result<(), AutomationError> {
        let automation = automation.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.update_automation(&automation)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn list_execution_records(
        &mut self,
        automation_code: &str,
    ) -> Result<Vec<AutomationExecutionRecord>, AutomationError> {
        let automation_code = automation_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }
                    .list_execution_records(&automation_code)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn record_execution(
        &mut self,
        automation: &Automation,
        draft: &AutomationExecutionDraft,
    ) -> Result<Option<AutomationExecutionRecord>, AutomationError> {
        let automation = automation.to_owned();
        let draft = draft.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.record_execution(&automation, &draft)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn link_execution_session(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        session_code: &str,
    ) -> Result<(), AutomationError> {
        let automation_code = automation_code.to_owned();
        let execution_code = execution_code.to_owned();
        let session_code = session_code.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.link_execution_session(
                    &automation_code,
                    &execution_code,
                    &session_code,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn recover_interrupted_executions(
        &mut self,
        finished_at: &str,
    ) -> Result<usize, AutomationError> {
        let finished_at = finished_at.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }
                    .recover_interrupted_executions(&finished_at)
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn prepare_serial_execution(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        finished_at: &str,
    ) -> Result<Option<Vec<AutomationExecutionRecord>>, AutomationError> {
        let automation_code = automation_code.to_owned();
        let execution_code = execution_code.to_owned();
        let finished_at = finished_at.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.prepare_serial_execution(
                    &automation_code,
                    &execution_code,
                    &finished_at,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn complete_execution_record(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        status: AutomationExecutionStatus,
        summary: Option<String>,
        session_code: Option<String>,
        finished_at: &str,
    ) -> Result<(), AutomationError> {
        let automation_code = automation_code.to_owned();
        let execution_code = execution_code.to_owned();
        let finished_at = finished_at.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.complete_execution_record(
                    &automation_code,
                    &execution_code,
                    status,
                    summary,
                    session_code,
                    &finished_at,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
    fn delete_execution_record(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        deleted_at: &str,
    ) -> Result<AutomationExecutionRecord, AutomationError> {
        let automation_code = automation_code.to_owned();
        let execution_code = execution_code.to_owned();
        let deleted_at = deleted_at.to_owned();
        self.connection.call(
            move |connection| {
                SqliteAutomationRepositorySql { connection }.delete_execution_record(
                    &automation_code,
                    &execution_code,
                    &deleted_at,
                )
            },
            || storage_error(rusqlite::Error::InvalidQuery),
        )
    }
}
