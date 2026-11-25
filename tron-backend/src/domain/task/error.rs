use std::{error::Error, fmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    NotFound,
    WorkspaceRequired,
    InvalidWorkspace,
    InvalidParent,
    AssigneeNotInWorkspace,
    InvalidApp,
    ActiveSubtaskExists,
    SubtaskExists,
    ArchivedMutationDenied,
    FlowNotFound,
    FlowCodeConflict,
    FlowInUse,
    WorkspaceFlowHasOpenTasks,
    StatusNotInFlow,
    InvalidProgress,
    InvalidRequest,
    SessionRunActive,
    StorageFailure,
}

impl fmt::Display for TaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("task was not found"),
            Self::WorkspaceRequired => formatter.write_str("workspaceCode is required"),
            Self::InvalidWorkspace => formatter.write_str("task workspace is invalid"),
            Self::InvalidParent => formatter.write_str("task parent is invalid"),
            Self::AssigneeNotInWorkspace => formatter.write_str("task assignee is invalid"),
            Self::InvalidApp => formatter.write_str("task app is invalid"),
            Self::ActiveSubtaskExists => formatter.write_str("task has active subtasks"),
            Self::SubtaskExists => formatter.write_str("task has subtasks"),
            Self::ArchivedMutationDenied => formatter.write_str("archived task cannot be modified"),
            Self::FlowNotFound => formatter.write_str("task flow was not found"),
            Self::FlowCodeConflict => formatter.write_str("task flow code already exists"),
            Self::FlowInUse => formatter.write_str("task flow is used by a workspace"),
            Self::WorkspaceFlowHasOpenTasks => {
                formatter.write_str("close every task before switching the task flow")
            }
            Self::StatusNotInFlow => formatter.write_str("task status is not in the task flow"),
            Self::InvalidProgress => formatter.write_str("task progress is invalid"),
            Self::InvalidRequest => formatter.write_str("task request is invalid"),
            Self::SessionRunActive => formatter.write_str("linked chat session is running"),
            Self::StorageFailure => formatter.write_str("task storage failure"),
        }
    }
}

impl Error for TaskError {}
