use serde::{Deserialize, Serialize};

pub const DEFAULT_TASK_FLOW_CODE: &str = "DEFAULT";
pub const NOT_STARTED_STATUS_CODE: &str = "NOT_STARTED";
pub const IN_PROGRESS_STATUS_CODE: &str = "IN_PROGRESS";
pub const CLOSED_STATUS_CODE: &str = "CLOSED";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFlowStatus {
    pub status_code: String,
    pub name: String,
    pub position: usize,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFlow {
    pub flow_code: String,
    pub name: String,
    pub statuses: Vec<TaskFlowStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFlowDraft {
    pub flow_code: String,
    pub name: String,
    pub statuses: Vec<TaskFlowStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTaskFlow {
    pub workspace_code: String,
    pub flow: TaskFlow,
}
