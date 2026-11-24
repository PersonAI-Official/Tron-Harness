use std::{
    collections::{BTreeSet, HashSet},
    sync::{Arc, Mutex},
};

use crate::{
    application::{app::AppService, workspace::WorkspaceService},
    domain::{
        app::AppError,
        task::{
            PagedTaskRecords, Task, TaskCodeLink, TaskDetail, TaskError, TaskExecutionRecord,
            TaskExecutionStatus, TaskFlow, TaskFlowDraft, TaskKanbanColumn, TaskLifecycleResult,
            TaskListItem, TaskListQuery, TaskOptions, TaskPriority, TaskSourceType, TaskSummary,
            TaskType, TaskUserAssignment,
            WorkspaceTaskFlow,
        },
        workspace::WorkspaceError,
    },
    shared::{
        event_bus::{BusEventDraft, EventBusHandle},
        id::SnowflakeId,
        time::current_timestamp,
    },
};

mod assistant_tasks;
mod events;
mod flows;
mod projection;
mod repository_access;
mod task_projection;
mod use_cases;
mod validation;

pub use self::assistant_tasks::CancelAssistantTaskError;
use self::assistant_tasks::default_planned_start;
use self::projection::*;

#[derive(Debug, Clone)]
pub struct CreateTaskCommand {
    pub workspace_code: Option<String>,
    pub owner_assistant_code: Option<String>,
    pub due_at: Option<i64>,
    pub parent_task_code: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub task_type: TaskType,
    pub source_type: TaskSourceType,
    pub source_url: Option<String>,
    pub agent_code: Option<String>,
    pub model_code: Option<String>,
    pub assignee_user_codes: Vec<String>,
    pub agent_codes: Vec<String>,
    pub app_codes: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTaskCommand {
    pub due_at: Option<Option<i64>>,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<TaskPriority>,
    pub task_type: Option<TaskType>,
    pub source_type: Option<TaskSourceType>,
    pub source_url: Option<Option<String>>,
    pub agent_code: Option<String>,
    pub model_code: Option<Option<String>>,
    pub assignee_user_codes: Option<Vec<String>>,
    pub agent_codes: Option<Vec<String>>,
    pub app_codes: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct UpdateTaskStatusCommand {
    pub status: String,
    pub progress: Option<u8>,
}

#[derive(Default)]
pub struct TaskRelations {
    pub assignees: Vec<TaskUserAssignment>,
    pub agents: Vec<TaskCodeLink>,
    pub apps: Vec<TaskCodeLink>,
}

/// A bounded list page and only the descendants needed to compute its progress.
pub struct TaskReadPage {
    pub tasks: Vec<Task>,
    pub page_codes: Vec<String>,
    pub total: usize,
    pub relations: std::collections::BTreeMap<String, TaskRelations>,
}

/// Read handles can share the WAL pool without retaining the service's write mutex.
pub trait TaskReadRepository: Send + Sync {
    fn get_task(&self, code: &str) -> Result<Task, TaskError>;
    /// Filter and page in storage; None explicitly requests the complete filtered set for kanban.
    fn page(
        &self,
        query: &TaskListQuery,
        window: Option<(usize, usize)>,
    ) -> Result<TaskReadPage, TaskError>;
    fn list_tasks(&self) -> Result<Vec<Task>, TaskError>;
    fn page_task_executions(
        &self,
        task_code: &str,
        current: usize,
        size: usize,
    ) -> Result<PagedTaskRecords<TaskExecutionRecord>, TaskError>;
    fn relations(
        &self,
        codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, TaskRelations>, TaskError>;
}

pub trait TaskRepository: Send {
    fn reader(&self) -> Arc<dyn TaskReadRepository>;
    fn prune_finished_assistant_tasks(&mut self, cutoff_ms: i64) -> Result<usize, TaskError>;
    /// Batch only the selected tasks, preserving each link set's ordering.
    fn relations(
        &mut self,
        codes: &[String],
    ) -> Result<std::collections::BTreeMap<String, TaskRelations>, TaskError>;
    fn list_tasks(&mut self) -> Result<Vec<Task>, TaskError>;
    fn claim_assistant_task(
        &mut self,
        task_code: &str,
        expected_version: i64,
        now_ms: i64,
    ) -> Result<bool, TaskError>;
    fn bind_assistant_work_session(
        &mut self,
        task_code: &str,
        session_code: &str,
    ) -> Result<bool, TaskError>;
    fn begin_task_execution(
        &mut self,
        task: &Task,
        execution: &TaskExecutionRecord,
    ) -> Result<(), TaskError>;
    fn finish_task_execution(
        &mut self,
        task: &Task,
        execution_code: &str,
        status: TaskExecutionStatus,
        summary: Option<&str>,
        finished_at: &str,
    ) -> Result<(), TaskError>;
    fn list_task_flows(&mut self) -> Result<Vec<TaskFlow>, TaskError>;
    fn get_task_flow(&mut self, flow_code: &str) -> Result<Option<TaskFlow>, TaskError>;
    fn create_task_flow(&mut self, flow: &TaskFlow) -> Result<(), TaskError>;
    fn update_task_flow(&mut self, flow: &TaskFlow) -> Result<(), TaskError>;
    fn delete_task_flow(&mut self, flow_code: &str) -> Result<(), TaskError>;
    fn workspace_task_flow_code(
        &mut self,
        workspace_code: &str,
    ) -> Result<Option<String>, TaskError>;
    fn assign_workspace_task_flow(
        &mut self,
        workspace_code: &str,
        flow_code: &str,
    ) -> Result<(), TaskError>;
    fn update_workspace_task_flow(
        &mut self,
        workspace_code: &str,
        flow_code: &str,
    ) -> Result<(), TaskError>;
    fn next_id(&mut self) -> Result<i64, TaskError>;
    fn create_task(&mut self, task: &Task) -> Result<(), TaskError>;
    fn update_task(&mut self, task: &Task) -> Result<(), TaskError>;
    /// Delete the task row together with its link sets.
    fn delete_task(&mut self, task_code: &str) -> Result<(), TaskError>;
    /// Create the task row and replace all link sets in one storage
    /// transaction. Every storage adapter must implement this atomic operation.
    fn create_task_with_links(
        &mut self,
        task: &Task,
        user_assignments: &[TaskUserAssignment],
        agent_links: &[TaskCodeLink],
        app_links: &[TaskCodeLink],
    ) -> Result<(), TaskError>;
    /// Update the task row and replace only the provided link sets, all in one
    /// storage transaction.
    fn update_task_with_links(
        &mut self,
        task: &Task,
        user_assignments: Option<&[TaskUserAssignment]>,
        agent_links: Option<&[TaskCodeLink]>,
        app_links: Option<&[TaskCodeLink]>,
    ) -> Result<(), TaskError>;
    fn list_user_assignments(
        &mut self,
        task_code: &str,
    ) -> Result<Vec<TaskUserAssignment>, TaskError>;
    fn replace_user_assignments(
        &mut self,
        task_code: &str,
        entries: &[TaskUserAssignment],
    ) -> Result<Vec<TaskUserAssignment>, TaskError>;
    fn list_agent_links(&mut self, task_code: &str) -> Result<Vec<TaskCodeLink>, TaskError>;
    fn replace_agent_links(
        &mut self,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<Vec<TaskCodeLink>, TaskError>;
    fn list_app_links(&mut self, task_code: &str) -> Result<Vec<TaskCodeLink>, TaskError>;
    fn replace_app_links(
        &mut self,
        task_code: &str,
        entries: &[TaskCodeLink],
    ) -> Result<Vec<TaskCodeLink>, TaskError>;
}

#[derive(Clone)]
pub struct TaskService {
    repository: Arc<Mutex<Box<dyn TaskRepository>>>,
    reader: Arc<dyn TaskReadRepository>,
    workspaces: WorkspaceService,
    apps: AppService,
    event_bus: EventBusHandle,
}

impl TaskService {
    pub fn new(
        repository: impl TaskRepository + 'static,
        workspaces: WorkspaceService,
        apps: AppService,
    ) -> Self {
        Self::with_event_bus(repository, workspaces, apps, EventBusHandle::disabled())
    }

    pub fn with_event_bus(
        repository: impl TaskRepository + 'static,
        workspaces: WorkspaceService,
        apps: AppService,
        event_bus: EventBusHandle,
    ) -> Self {
        let reader = repository.reader();
        Self {
            repository: Arc::new(Mutex::new(Box::new(repository))),
            reader,
            workspaces,
            apps,
            event_bus,
        }
    }
}
