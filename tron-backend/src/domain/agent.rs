mod text_patch;
pub use text_patch::{FilePatch, apply_text_hunks, parse_text_patch};
mod agui;
mod application_context;
mod collaboration;
mod context;
mod error;
mod history;
mod kind;
mod management;
mod message;
mod model_config;
mod model_exchange;
mod run;
mod tool;
pub(crate) use tool::validate_workspace_request;
mod tool_assignment;
pub use tool_assignment::ToolAssignmentStrategy;
mod tool_schema;

pub use agui::{
    AgentAguiEvent, AgentAguiEventSnapshot, AgentAguiProtocolEvent, AgentAguiStreamState,
};
pub use application_context::{
    ApplicationDirectoryContext, ApplicationDirectoryStatus, GitWorktreeContext,
};
pub use collaboration::{
    AgentAssignment, AgentExecutionMode, AgentRunPlanStep, CollaborationArtifact,
    CollaborationDecision, CollaborationRun, CollaborationRunStatus, CollaborationStep,
    CollaborationStepExecutionMode, CollaborationStepStatus, HandoffContext,
};
pub use context::{
    AgentCollaborationOwner, AgentExecutionScope, AgentPageContext, AgentPageTarget,
    AgentPromptPrefixSource, AgentRunContext, AgentRunLimits, AgentShortTermMemoryRunContext,
    AgentToolAssignment, AutoModelCandidate, AutoModelRouteContext, EffectiveSkill, EffectiveTool,
    EffectiveVariable, ToolExecutionMode,
};
pub use error::AgentError;
pub use history::{
    AgentRunHistoryError, AgentRunHistoryQuery, AgentRunHistoryRecord, PagedAgentRunHistoryRecords,
};
pub use kind::{AgentKind, AgentModelProtocol, AgentRuntimeKind};
pub use management::{
    AgentManagementError, AgentRuntimeType, InternalAgentPolicySource, InternalAgentToolPolicy,
    ManagedAgentSource, ManagedAgentType, ManagedSystemAgentRecord, McpAccessMode,
    NativeToolConfig, NativeToolScheme, PlatformInternalAgentConfig,
    RuntimeAdapterCapabilityRecord, SystemAgentModelRecord, SystemAgentRegistryEntry,
    SystemAgentStatus, SystemAgentSyncResult, ToolAssignmentMode, ToolAvailabilityState,
    ToolCatalogIssue, ToolCatalogRecord, ToolDomainRecord, ToolOperationKind,
    WorkspaceInternalAgentDefinition, WorkspaceInternalAgentMcpServerAssignment,
    WorkspaceInternalAgentPolicy, WorkspaceInternalAgentToolAssignment, WorkspaceSystemAgentPolicy,
};
pub use message::{AgentMessage, AgentMessagePart};
pub use model_config::AgentModelConfig;
pub use model_exchange::{
    ModelFinish, ModelOutputItem, ModelRequest, ModelResponse, ModelStreamEvent,
    ModelStreamEventKind, ModelTokenUsage, TokenEstimateSnapshot, TokenUsageAuditSnapshot,
    ToolCall,
};
pub(crate) use model_exchange::{responses_function_call_item_json, responses_input_item_json};
pub use run::{
    AgentChargedUsage, AgentNativeExecutionSnapshot, AgentNativeInteraction,
    AgentNativeInteractionKind, AgentNativeRunOutcome, AgentNativeRunStatus, AgentRunResult,
    AgentRunStatus,
};
pub use tool::{
    InProcessTool, Tool, ToolError, ToolExecutionControl, ToolExecutionFuture, ToolExecutionResult,
    ToolObservation, ToolRegistry, ToolRequest,
};
pub use tool_schema::PreparedToolSchema;
