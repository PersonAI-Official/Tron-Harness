use serde::Deserialize;

use crate::{
    application::task::{CreateTaskCommand, UpdateTaskCommand, UpdateTaskStatusCommand},
    domain::task::{TaskFlowDraft, TaskFlowStatus, TaskPriority, TaskSourceType, TaskType},
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTaskRequest {
    pub workspace_code: Option<String>,
    pub owner_assistant_code: Option<String>,
    pub due_at: Option<i64>,
    pub parent_task_code: Option<String>,
    pub title: String,
    pub description: Option<String>,
    #[serde(default = "default_task_type")]
    pub task_type: TaskType,
    #[serde(default = "default_source_type")]
    pub source_type: TaskSourceType,
    pub source_url: Option<String>,
    pub agent_code: Option<String>,
    pub model_code: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: TaskPriority,
    #[serde(default)]
    pub assignee_user_codes: Vec<String>,
    #[serde(default)]
    pub agent_codes: Vec<String>,
    #[serde(default)]
    pub app_codes: Vec<String>,
}

impl From<CreateTaskRequest> for CreateTaskCommand {
    fn from(request: CreateTaskRequest) -> Self {
        Self {
            workspace_code: request.workspace_code,
            owner_assistant_code: request.owner_assistant_code,
            due_at: request.due_at,
            parent_task_code: request.parent_task_code,
            title: request.title,
            description: request.description,
            priority: request.priority,
            task_type: request.task_type,
            source_type: request.source_type,
            source_url: request.source_url,
            agent_code: request.agent_code,
            model_code: request.model_code,
            assignee_user_codes: request.assignee_user_codes,
            agent_codes: request.agent_codes,
            app_codes: request.app_codes,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTaskRequest {
    #[serde(default, deserialize_with = "deserialize_optional_deadline")]
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

fn deserialize_optional_deadline<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<i64>>, D::Error> {
    Option::<i64>::deserialize(deserializer).map(Some)
}

impl From<UpdateTaskRequest> for UpdateTaskCommand {
    fn from(request: UpdateTaskRequest) -> Self {
        Self {
            due_at: request.due_at,
            title: request.title,
            description: request.description,
            priority: request.priority,
            task_type: request.task_type,
            source_type: request.source_type,
            source_url: request.source_url,
            agent_code: request.agent_code,
            model_code: request.model_code,
            assignee_user_codes: request.assignee_user_codes,
            agent_codes: request.agent_codes,
            app_codes: request.app_codes,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTaskStatusRequest {
    pub status: String,
    pub progress: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskFlowStatusRequest {
    pub status_code: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskFlowRequest {
    pub flow_code: String,
    pub name: String,
    #[serde(default)]
    pub statuses: Vec<TaskFlowStatusRequest>,
}

impl From<TaskFlowRequest> for TaskFlowDraft {
    fn from(request: TaskFlowRequest) -> Self {
        Self {
            flow_code: request.flow_code,
            name: request.name,
            statuses: request
                .statuses
                .into_iter()
                .enumerate()
                .map(|(position, status)| TaskFlowStatus {
                    status_code: status.status_code,
                    name: status.name,
                    position,
                    required: false,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTaskFlowRequest {
    pub flow_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFlowWorkspaceQuery {
    pub workspace_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskExecutionListQuery {
    pub current: Option<usize>,
    pub size: Option<usize>,
}

impl From<UpdateTaskStatusRequest> for UpdateTaskStatusCommand {
    fn from(request: UpdateTaskStatusRequest) -> Self {
        Self {
            status: request.status,
            progress: request.progress,
        }
    }
}

fn default_task_type() -> TaskType {
    TaskType::General
}
fn default_source_type() -> TaskSourceType {
    TaskSourceType::Internal
}
fn default_priority() -> TaskPriority {
    TaskPriority::Medium
}
