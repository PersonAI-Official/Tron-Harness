use serde::Serialize;

use super::{TaskFlow, TaskFlowStatus, TaskPriority, TaskSourceType, TaskType};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOptions {
    pub flow: TaskFlow,
    pub statuses: Vec<TaskFlowStatus>,
    pub priorities: Vec<TaskPriority>,
    pub task_types: Vec<TaskType>,
    pub source_types: Vec<TaskSourceType>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub task_code: String,
    pub title: String,
    pub status: String,
    pub priority: TaskPriority,
    pub progress: u8,
    pub task_type: TaskType,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    pub id: String,
    pub task_code: String,
    pub workspace_code: Option<String>,
    pub owner_assistant_code: Option<String>,
    pub due_at: Option<i64>,
    pub planned_start_at: Option<i64>,
    pub next_followup_at: Option<i64>,
    pub work_session_code: Option<String>,
    pub schedule_version: i64,

    pub parent_task_code: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub execution_result: Option<String>,
    pub flow_code: String,
    pub status: String,
    pub previous_status: Option<String>,
    pub priority: TaskPriority,
    pub task_type: TaskType,
    pub source_type: TaskSourceType,
    pub source_url: Option<String>,
    pub agent_code: String,
    pub model_code: Option<String>,
    pub progress: u8,
    pub assignee_user_codes: Vec<String>,
    pub agent_codes: Vec<String>,
    pub app_codes: Vec<String>,
    pub parent: Option<TaskSummary>,
    pub subtasks: Vec<TaskSummary>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListItem {
    pub task_code: String,
    pub workspace_code: Option<String>,
    pub owner_assistant_code: Option<String>,
    pub due_at: Option<i64>,
    pub planned_start_at: Option<i64>,
    pub next_followup_at: Option<i64>,
    pub work_session_code: Option<String>,
    pub schedule_version: i64,

    pub parent_task_code: Option<String>,
    pub title: String,
    pub flow_code: String,
    pub status: String,
    pub priority: TaskPriority,
    pub progress: u8,
    pub task_type: TaskType,
    pub source_type: TaskSourceType,
    pub source_url: Option<String>,
    pub agent_code: String,
    pub model_code: Option<String>,
    pub assignee_user_codes: Vec<String>,
    pub agent_codes: Vec<String>,
    pub app_codes: Vec<String>,
    pub subtask_count: usize,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskKanbanColumn {
    pub status: String,
    pub title: String,
    pub tasks: Vec<TaskListItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLifecycleResult {
    pub task_code: String,
    pub status: String,
    pub previous_status: Option<String>,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionResult {
    pub task_code: String,
    pub session_code: String,
    pub run_code: String,
    pub agent_code: String,
    pub model_code: Option<String>,
    pub summary: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TaskExecutionStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionRecord {
    pub execution_code: String,
    pub task_code: String,
    pub session_code: Option<String>,
    pub status: TaskExecutionStatus,
    pub summary: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedTaskRecords<T> {
    pub current: usize,
    pub size: usize,
    pub total: usize,
    pub records: Vec<T>,
}
