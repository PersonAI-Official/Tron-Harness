mod admission;
mod archives;
pub use archives::ArchiveError;
mod attachments;
pub(crate) mod composer_input;
mod dto;
mod goal_loop;
pub(crate) mod history_error;
pub(crate) mod model_resolution;
mod port;
mod run_control;
mod service;
mod steer;
pub(crate) mod subagent_error;
mod title;

pub use dto::{
    ChatResumeEntry, ChatSendResult, ChatSessionAttachmentUploadCommand, ChatSessionQuery,
    ChatSteerResult, CreateChatSessionCommand, SendChatMessageCommand, SteerChatMessageCommand,
    UpdateChatSessionCommand,
};
pub use port::{ChatContainer, ChatContainerSnapshot, ChatRepository, ChatSessionTitle};
pub use run_control::{
    ActiveChatRunSnapshot, ChatActiveRunState, ChatRunControlError, GetActiveChatRunResult,
    StopChatRunCommand, StopChatRunResult,
};
pub use service::management::{
    ChatSessionBatchItem, ChatSessionBatchResult, ChatSessionManagementQuery,
    ChatSessionMoveCommand, ChatSessionStopError, ManagedChatSession,
};
pub use service::{ArtifactContent, ChatSendError, ChatService};
pub use steer::ChatSteerError;

pub(crate) mod a2ui_error;
