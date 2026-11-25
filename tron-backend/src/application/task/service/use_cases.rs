use crate::domain::task::{TASK_PRIORITIES, TASK_SOURCE_TYPES, TASK_TYPES};

use super::flows::normalized_flow_code;
use super::*;

impl TaskService {
    pub fn options(&self, workspace_code: &str) -> Result<TaskOptions, TaskError> {
        let flow = self.task_flow_for_workspace(workspace_code)?;
        Ok(TaskOptions {
            statuses: flow.statuses.clone(),
            flow,
            priorities: TASK_PRIORITIES.to_vec(),
            task_types: TASK_TYPES.to_vec(),
            source_types: TASK_SOURCE_TYPES.to_vec(),
        })
    }

    pub fn list_tasks(
        &self,
        query: TaskListQuery,
    ) -> Result<PagedTaskRecords<TaskListItem>, TaskError> {
        let current = query.current.unwrap_or(1).max(1);
        let size = query.size.unwrap_or(20).clamp(1, 100);
        let start = current.saturating_sub(1).saturating_mul(size);
        let (records, total) = self.filtered_task_page(&query, Some((start, size)))?;
        Ok(PagedTaskRecords {
            current,
            size,
            total,
            records,
        })
    }

    pub fn list_task_executions(
        &self,
        task_code: &str,
        current: usize,
        size: usize,
    ) -> Result<PagedTaskRecords<TaskExecutionRecord>, TaskError> {
        self.get_task_record(task_code)?;
        let current = current.max(1);
        let size = size.clamp(1, 100);
        self.reader
            .page_task_executions(task_code, current, size)
    }

    pub fn kanban(&self, query: TaskListQuery) -> Result<Vec<TaskKanbanColumn>, TaskError> {
        let workspace_code = query
            .workspace_code
            .as_deref()
            .and_then(|value| normalized_required(value.to_owned()))
            .ok_or(TaskError::WorkspaceRequired)?;
        let flow = self.task_flow_for_workspace(&workspace_code)?;
        let mut items = self.filtered_task_items(&query)?;
        sort_kanban_task_items(&mut items);
        Ok(flow
            .statuses
            .into_iter()
            .map(|status| TaskKanbanColumn {
                tasks: items
                    .iter()
                    .filter(|item| item.status == status.status_code)
                    .cloned()
                    .collect(),
                title: status.name,
                status: status.status_code,
            })
            .collect())
    }

    pub fn create_task(&self, command: CreateTaskCommand) -> Result<TaskDetail, TaskError> {
        let workspace_code = optional_trimmed(command.workspace_code);
        let owner_assistant_code = optional_trimmed(command.owner_assistant_code);
        if let Some(workspace) = workspace_code.as_deref() {
            self.ensure_active_workspace(workspace)?;
        } else if command.task_type != TaskType::General {
            return Err(TaskError::WorkspaceRequired);
        }
        if command.due_at.is_some_and(|value| value < 0) {
            return Err(TaskError::InvalidRequest);
        }
        let title = normalized_required(command.title).ok_or(TaskError::InvalidRequest)?;
        let parent_task_code = optional_trimmed(command.parent_task_code);
        if let Some(parent_code) = parent_task_code.as_deref() {
            self.ensure_valid_parent(
                parent_code,
                workspace_code.as_deref(),
                owner_assistant_code.as_deref(),
            )?;
        }
        let flow = match workspace_code.as_deref() {
            Some(workspace) => self.task_flow_for_workspace(workspace)?,
            None => self.get_task_flow(crate::domain::task::DEFAULT_TASK_FLOW_CODE)?,
        };
        let assignees = self.validated_assignees(
            workspace_code.as_deref().unwrap_or_default(),
            command.assignee_user_codes,
        )?;
        let agent_links = normalized_code_links(command.agent_codes);
        let app_links =
            self.validate_scope_app_links(workspace_code.as_deref(), command.app_codes)?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let planned_start_at = owner_assistant_code
            .as_ref()
            .map(|_| default_planned_start(now_ms, command.due_at));
        // Parallel PersonAI sessions share this service; allocate and persist under one lock.
        let mut repository = self
            .repository
            .lock()
            .expect("task repository mutex poisoned");
        let id = repository.next_id()?;
        let task_code = format!("TSK-{id}");
        let agent_code = optional_trimmed(command.agent_code)
            .or_else(|| owner_assistant_code.clone())
            .unwrap_or_else(|| "main-agent".to_string());
        let task = Task {
            id: SnowflakeId::new(id),
            task_code: task_code.clone(),
            workspace_code,
            owner_assistant_code,
            due_at: command.due_at,
            planned_start_at,
            next_followup_at: planned_start_at,
            work_session_code: None,
            schedule_version: 0,
            parent_task_code,
            title,
            description: optional_trimmed(command.description),
            execution_result: None,
            flow_code: flow.flow_code,
            status: crate::domain::task::NOT_STARTED_STATUS_CODE.to_string(),
            previous_status: None,
            priority: command.priority,
            task_type: command.task_type,
            source_type: command.source_type,
            source_url: optional_source_url(command.source_url),
            agent_code,
            model_code: optional_trimmed(command.model_code),
            progress: 0,
            created_at: current_timestamp(),
            updated_at: current_timestamp(),
            archived_at: None,
        };
        repository.create_task_with_links(&task, &assignees, &agent_links, &app_links)?;
        drop(repository);
        let detail = self.get_task(&task_code)?;
        self.publish_task_detail_event("task.created", "Created task", &detail);
        Ok(detail)
    }

    pub fn get_task(&self, task_code: &str) -> Result<TaskDetail, TaskError> {
        self.detail_from_task(self.get_task_record(task_code)?)
    }

    pub fn update_task(
        &self,
        task_code: &str,
        command: UpdateTaskCommand,
    ) -> Result<TaskDetail, TaskError> {
        self.update_task_with_status(task_code, command, None)
    }

    /// Validate the requested status before committing any other task fields.
    pub fn update_task_with_status(
        &self,
        task_code: &str,
        command: UpdateTaskCommand,
        status_update: Option<UpdateTaskStatusCommand>,
    ) -> Result<TaskDetail, TaskError> {
        let mut task = self.get_task_record(task_code)?;
        if task.archived_at.is_some() {
            return Err(TaskError::ArchivedMutationDenied);
        }
        if let Some(status_update) = status_update {
            let status =
                normalized_flow_code(&status_update.status).ok_or(TaskError::InvalidRequest)?;
            let flow = self.get_task_flow(&task.flow_code)?;
            if !Self::flow_contains_status(&flow, &status) {
                return Err(TaskError::StatusNotInFlow);
            }
            task.progress = if status == crate::domain::task::CLOSED_STATUS_CODE {
                100
            } else {
                status_update
                    .progress
                    .unwrap_or(if task.status == crate::domain::task::CLOSED_STATUS_CODE {
                        0
                    } else {
                        task.progress
                    })
                    .min(100)
            };
            task.status = status;
        }
        if let Some(title) = command.title {
            task.title = normalized_required(title).ok_or(TaskError::InvalidRequest)?;
        }
        if let Some(description) = command.description {
            task.description = description.and_then(|value| optional_trimmed(Some(value)));
        }
        if let Some(priority) = command.priority {
            task.priority = priority;
        }
        if let Some(task_type) = command.task_type {
            task.task_type = task_type;
        }
        if let Some(source_type) = command.source_type {
            task.source_type = source_type;
        }
        if let Some(source_url) = command.source_url {
            task.source_url = optional_source_url(source_url);
        }
        if let Some(agent_code) = command.agent_code {
            task.agent_code =
                normalized_required(agent_code).unwrap_or_else(|| "main-agent".to_string());
        }
        if let Some(model_code) = command.model_code {
            task.model_code = model_code.and_then(|value| optional_trimmed(Some(value)));
        }
        if let Some(due_at) = command.due_at {
            if due_at.is_some_and(|value| value < 0) {
                return Err(TaskError::InvalidRequest);
            }
            task.due_at = due_at;
            if task.owner_assistant_code.is_some() {
                let next = default_planned_start(chrono::Utc::now().timestamp_millis(), due_at);
                task.planned_start_at = Some(next);
                task.next_followup_at = Some(next);
            }
        }
        if task.workspace_code.is_none() && task.task_type != TaskType::General {
            return Err(TaskError::WorkspaceRequired);
        }
        if task.status == crate::domain::task::CLOSED_STATUS_CODE {
            task.next_followup_at = None;
        } else if task.owner_assistant_code.is_some() && task.next_followup_at.is_none() {
            task.next_followup_at = Some(chrono::Utc::now().timestamp_millis());
        }
        task.updated_at = current_timestamp();
        let assignees = command
            .assignee_user_codes
            .map(|codes| {
                self.validated_assignees(task.workspace_code.as_deref().unwrap_or_default(), codes)
            })
            .transpose()?;
        let agent_links = command.agent_codes.map(normalized_code_links);
        let app_links = command
            .app_codes
            .map(|codes| self.validate_scope_app_links(task.workspace_code.as_deref(), codes))
            .transpose()?;
        {
            let mut repository = self
                .repository
                .lock()
                .expect("task repository mutex poisoned");
            repository.update_task_with_links(
                &task,
                assignees.as_deref(),
                agent_links.as_deref(),
                app_links.as_deref(),
            )?;
        }
        let detail = self.get_task(task_code)?;
        self.publish_task_detail_event("task.updated", "Updated task", &detail);
        Ok(detail)
    }

    pub fn update_status(
        &self,
        task_code: &str,
        command: UpdateTaskStatusCommand,
    ) -> Result<TaskDetail, TaskError> {
        let mut task = self.get_task_record(task_code)?;
        if task.archived_at.is_some() {
            return Err(TaskError::ArchivedMutationDenied);
        }
        let status = normalized_flow_code(&command.status).ok_or(TaskError::InvalidRequest)?;
        let flow = self.get_task_flow(&task.flow_code)?;
        if !Self::flow_contains_status(&flow, &status) {
            return Err(TaskError::StatusNotInFlow);
        }
        let reopening = task.status == crate::domain::task::CLOSED_STATUS_CODE
            && status != crate::domain::task::CLOSED_STATUS_CODE;
        task.status = status.clone();
        task.progress = if status == crate::domain::task::CLOSED_STATUS_CODE {
            100
        } else {
            command
                .progress
                .unwrap_or(if reopening { 0 } else { task.progress })
                .min(100)
        };
        if task.status == crate::domain::task::CLOSED_STATUS_CODE {
            task.next_followup_at = None;
        } else if task.owner_assistant_code.is_some() && task.next_followup_at.is_none() {
            task.next_followup_at = Some(chrono::Utc::now().timestamp_millis());
        }
        task.updated_at = current_timestamp();
        self.repository
            .lock()
            .expect("task repository mutex poisoned")
            .update_task(&task)?;
        let detail = self.get_task(task_code)?;
        self.publish_task_detail_event("task.status_updated", "Updated task status", &detail);
        Ok(detail)
    }

    pub fn begin_task_execution(
        &self,
        task_code: &str,
        session_code: &str,
        execution_code: &str,
    ) -> Result<TaskDetail, TaskError> {
        let mut task = self.get_task_record(task_code)?;
        if task.archived_at.is_some() {
            return Err(TaskError::ArchivedMutationDenied);
        }
        let session_code =
            normalized_required(session_code.to_owned()).ok_or(TaskError::InvalidRequest)?;
        let execution_code =
            normalized_required(execution_code.to_owned()).ok_or(TaskError::InvalidRequest)?;

        if task.status == crate::domain::task::NOT_STARTED_STATUS_CODE {
            let flow = self.get_task_flow(&task.flow_code)?;
            if let Some(status) = flow
                .statuses
                .iter()
                .find(|status| status.status_code == crate::domain::task::IN_PROGRESS_STATUS_CODE)
                .or_else(|| {
                    flow.statuses.iter().find(|status| {
                        status.status_code != crate::domain::task::NOT_STARTED_STATUS_CODE
                            && status.status_code != crate::domain::task::CLOSED_STATUS_CODE
                    })
                })
            {
                task.status = status.status_code.clone();
            }
        }

        task.work_session_code = Some(session_code);
        task.updated_at = current_timestamp();
        let now = current_timestamp();
        let execution = TaskExecutionRecord {
            execution_code,
            task_code: task.task_code.clone(),
            session_code: task.work_session_code.clone(),
            status: TaskExecutionStatus::Running,
            summary: None,
            started_at: now.clone(),
            finished_at: None,
            created_at: now,
        };
        self.repository
            .lock()
            .expect("task repository mutex poisoned")
            .begin_task_execution(&task, &execution)?;
        let detail = self.get_task(task_code)?;
        self.publish_task_detail_event("task.execution_started", "Started task execution", &detail);
        Ok(detail)
    }

    pub fn finish_task_execution(
        &self,
        task_code: &str,
        execution_code: &str,
        status: TaskExecutionStatus,
        summary: Option<String>,
    ) -> Result<TaskDetail, TaskError> {
        let mut task = self.get_task_record(task_code)?;
        let summary = summary.and_then(|value| normalized_required(value));
        if status == TaskExecutionStatus::Success {
            task.execution_result = summary.clone();
        }
        task.updated_at = current_timestamp();
        let finished_at = current_timestamp();
        self.repository
            .lock()
            .expect("task repository mutex poisoned")
            .finish_task_execution(
                &task,
                execution_code,
                status,
                summary.as_deref(),
                &finished_at,
            )?;
        let detail = self.get_task(task_code)?;
        self.publish_task_detail_event(
            "task.execution_recorded",
            "Recorded task execution result",
            &detail,
        );
        Ok(detail)
    }

    pub fn record_execution_result(
        &self,
        task_code: &str,
        execution_result: String,
    ) -> Result<TaskDetail, TaskError> {
        let mut task = self.get_task_record(task_code)?;
        if task.archived_at.is_some() {
            return Err(TaskError::ArchivedMutationDenied);
        }
        task.execution_result = optional_trimmed(Some(execution_result));
        task.updated_at = current_timestamp();
        self.repository
            .lock()
            .expect("task repository mutex poisoned")
            .update_task(&task)?;
        let detail = self.get_task(task_code)?;
        self.publish_task_detail_event(
            "task.execution_recorded",
            "Recorded task execution result",
            &detail,
        );
        Ok(detail)
    }

    pub fn archive_task(&self, task_code: &str) -> Result<TaskLifecycleResult, TaskError> {
        let task = self.get_task_record(task_code)?;
        self.archive_task_record(task)
    }

    pub(super) fn archive_task_record(
        &self,
        mut task: Task,
    ) -> Result<TaskLifecycleResult, TaskError> {
        if task.archived_at.is_none() {
            let has_active_subtasks = self.all_tasks()?.iter().any(|candidate| {
                candidate.parent_task_code.as_deref() == Some(&task.task_code)
                    && candidate.archived_at.is_none()
            });
            if has_active_subtasks {
                return Err(TaskError::ActiveSubtaskExists);
            }
            task.previous_status = Some(task.status);
            task.status = crate::domain::task::CLOSED_STATUS_CODE.to_string();
            task.archived_at = Some(current_timestamp());
            task.next_followup_at = None;
            task.updated_at = current_timestamp();
            self.repository
                .lock()
                .expect("task repository mutex poisoned")
                .update_task(&task)?;
        }
        let result = self.lifecycle_result(&task.task_code)?;
        self.publish_task_lifecycle_event("task.archived", "Archived task", &result);
        Ok(result)
    }

    pub fn restore_task(&self, task_code: &str) -> Result<TaskLifecycleResult, TaskError> {
        let mut task = self.get_task_record(task_code)?;
        if task.archived_at.is_some() {
            let flow = self.get_task_flow(&task.flow_code)?;
            task.status = task
                .previous_status
                .filter(|status| Self::flow_contains_status(&flow, status))
                .unwrap_or_else(|| crate::domain::task::NOT_STARTED_STATUS_CODE.to_string());
            task.previous_status = None;
            task.archived_at = None;
            if task.owner_assistant_code.is_some() {
                task.next_followup_at = Some(chrono::Utc::now().timestamp_millis());
            }
            task.updated_at = current_timestamp();
            self.repository
                .lock()
                .expect("task repository mutex poisoned")
                .update_task(&task)?;
        }
        let result = self.lifecycle_result(task_code)?;
        self.publish_task_lifecycle_event("task.restored", "Restored task", &result);
        Ok(result)
    }

    pub(crate) fn confirm_task_session_owner(&self, task_code: &str) -> Result<(), TaskError> {
        let _repository = self
            .repository
            .lock()
            .expect("task repository mutex poisoned");
        self.get_task_record(task_code).map(|_| ())
    }

    pub fn delete_task(
        &self,
        task_code: &str,
        chats: &crate::application::chat::ChatService,
    ) -> Result<(), TaskError> {
        let mut repository = self
            .repository
            .lock()
            .expect("task repository mutex poisoned");
        let task = self.get_task_record(task_code)?;
        let has_subtasks = self.all_tasks()?.iter().any(|candidate| {
            candidate.parent_task_code.as_deref() == Some(task.task_code.as_str())
        });
        if has_subtasks {
            return Err(TaskError::SubtaskExists);
        }
        chats
            .purge_task_sessions(&task.task_code, task.work_session_code.as_deref())
            .map_err(|error| {
                tracing::warn!(%error, task_code, "Task session cleanup failed");
                match error {
                    crate::domain::chat::ChatError::SessionRunActive => TaskError::SessionRunActive,
                    _ => TaskError::StorageFailure,
                }
            })?;
        repository.delete_task(&task.task_code)?;
        drop(repository);
        self.publish_task_deleted_event(&task);
        Ok(())
    }
}

fn sort_kanban_task_items(items: &mut [TaskListItem]) {
    items.sort_by(compare_kanban_task_items);
}

fn compare_kanban_task_items(left: &TaskListItem, right: &TaskListItem) -> std::cmp::Ordering {
    kanban_priority_rank(left.priority)
        .cmp(&kanban_priority_rank(right.priority))
        .then_with(|| {
            match (
                task_code_sequence(&left.task_code),
                task_code_sequence(&right.task_code),
            ) {
                (Some(left_sequence), Some(right_sequence)) => right_sequence.cmp(&left_sequence),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => right.task_code.cmp(&left.task_code),
            }
        })
        .then_with(|| right.task_code.cmp(&left.task_code))
}

fn kanban_priority_rank(priority: TaskPriority) -> u8 {
    match priority {
        TaskPriority::ExtraHigh => 0,
        TaskPriority::High => 1,
        TaskPriority::Medium => 2,
        TaskPriority::Low => 3,
    }
}

fn task_code_sequence(task_code: &str) -> Option<i64> {
    let digits = task_code
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<Vec<_>>();
    if digits.is_empty() {
        return None;
    }
    digits
        .into_iter()
        .rev()
        .collect::<String>()
        .parse::<i64>()
        .ok()
}
