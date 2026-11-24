use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};

use crate::shared::id::SnowflakeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationStatus {
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Canceled,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationTriggerSource {
    Scheduler,
    Manual,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutomationConcurrencyMode {
    #[default]
    Serial,
    Parallel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Automation {
    pub id: SnowflakeId,
    pub automation_code: String,
    pub workspace_code: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_code: String,
    pub model_code: Option<String>,
    pub prompt_text: String,
    pub cron_expression: String,
    pub scheduler_expression: String,
    pub timezone: String,
    pub status: AutomationStatus,
    pub concurrency_mode: AutomationConcurrencyMode,
    pub previous_status: Option<AutomationStatus>,
    pub last_execution_status: Option<AutomationExecutionStatus>,
    pub last_execution_code: Option<String>,
    pub last_execution_summary: Option<String>,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationExecutionRecord {
    pub id: SnowflakeId,
    pub execution_code: String,
    pub automation_code: String,
    pub workspace_code: String,
    pub session_code: Option<String>,
    pub trigger_source: AutomationTriggerSource,
    pub status: AutomationExecutionStatus,
    pub summary: Option<String>,
    pub scheduled_at: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AutomationExecutionStats {
    pub success_count: u64,
    pub total_count: u64,
}

impl AutomationExecutionStats {
    pub fn success_rate(self) -> Option<f64> {
        if self.total_count == 0 {
            None
        } else {
            Some(self.success_count as f64 / self.total_count as f64)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationExecutionDraft {
    pub automation_code: String,
    pub workspace_code: String,
    pub session_code: Option<String>,
    pub trigger_source: AutomationTriggerSource,
    pub status: AutomationExecutionStatus,
    pub summary: Option<String>,
    pub scheduled_at: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationOptions {
    pub default_status: AutomationStatus,
    pub default_concurrency_mode: AutomationConcurrencyMode,
    pub concurrency_modes: Vec<AutomationConcurrencyMode>,
    pub execution_statuses: Vec<AutomationExecutionStatus>,
    pub statuses: Vec<AutomationStatus>,
    pub timezones: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationListItem {
    pub automation_code: String,
    pub workspace_code: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_code: String,
    pub model_code: Option<String>,
    pub cron_expression: String,
    pub timezone: String,
    pub status: AutomationStatus,
    pub concurrency_mode: AutomationConcurrencyMode,
    pub last_execution_status: Option<AutomationExecutionStatus>,
    pub last_execution_summary: Option<String>,
    pub last_execution_code: Option<String>,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub next_run_at: Option<String>,
    pub execution_success_count: u64,
    pub execution_total_count: u64,
    pub execution_success_rate: Option<f64>,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationDetail {
    pub id: String,
    pub automation_code: String,
    pub workspace_code: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_code: String,
    pub model_code: Option<String>,
    pub prompt: String,
    pub cron_expression: String,
    pub scheduler_expression: String,
    pub timezone: String,
    pub status: AutomationStatus,
    pub concurrency_mode: AutomationConcurrencyMode,
    pub previous_status: Option<AutomationStatus>,
    pub last_execution_status: Option<AutomationExecutionStatus>,
    pub last_execution_code: Option<String>,
    pub last_execution_summary: Option<String>,
    pub last_started_at: Option<String>,
    pub last_finished_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationExecutionListItem {
    pub execution_code: String,
    pub automation_code: String,
    pub workspace_code: String,
    pub session_code: Option<String>,
    pub trigger_source: AutomationTriggerSource,
    pub status: AutomationExecutionStatus,
    pub summary: Option<String>,
    pub scheduled_at: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationLifecycleResult {
    pub archived_at: Option<String>,
    pub automation_code: String,
    pub previous_status: Option<AutomationStatus>,
    pub status: AutomationStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedAutomationRecords<T> {
    pub current: usize,
    pub size: usize,
    pub total: usize,
    pub records: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationListQuery {
    pub workspace_code: Option<String>,
    pub current: Option<usize>,
    pub size: Option<usize>,
    pub search: Option<String>,
    pub status: Option<AutomationStatus>,
    pub include_archived: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomationExecutionListQuery {
    pub current: Option<usize>,
    pub size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationError {
    NotFound,
    WorkspaceRequired,
    InvalidWorkspace,
    NameRequired,
    PromptRequired,
    InvalidCron,
    InvalidStatus,
    ArchivedMutationDenied,
    StorageFailure,
}

impl fmt::Display for AutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("automation was not found"),
            Self::WorkspaceRequired => formatter.write_str("workspaceCode is required"),
            Self::InvalidWorkspace => {
                formatter.write_str("automation workspace must reference an active workspace")
            }
            Self::NameRequired => formatter.write_str("automation name is required"),
            Self::PromptRequired => formatter.write_str("automation prompt is required"),
            Self::InvalidCron => formatter.write_str("automation cron expression is invalid"),
            Self::InvalidStatus => formatter.write_str("automation status is invalid"),
            Self::ArchivedMutationDenied => {
                formatter.write_str("archived automation cannot be modified")
            }
            Self::StorageFailure => formatter.write_str("automation storage failure"),
        }
    }
}

impl Error for AutomationError {}

pub const AUTOMATION_STATUSES: [AutomationStatus; 3] = [
    AutomationStatus::Active,
    AutomationStatus::Paused,
    AutomationStatus::Archived,
];

pub const AUTOMATION_EXECUTION_STATUSES: [AutomationExecutionStatus; 6] = [
    AutomationExecutionStatus::Pending,
    AutomationExecutionStatus::Running,
    AutomationExecutionStatus::Success,
    AutomationExecutionStatus::Failed,
    AutomationExecutionStatus::Canceled,
    AutomationExecutionStatus::Skipped,
];

pub const AUTOMATION_CONCURRENCY_MODES: [AutomationConcurrencyMode; 2] = [
    AutomationConcurrencyMode::Serial,
    AutomationConcurrencyMode::Parallel,
];

pub fn automation_options() -> AutomationOptions {
    AutomationOptions {
        default_status: AutomationStatus::Paused,
        default_concurrency_mode: AutomationConcurrencyMode::Serial,
        concurrency_modes: AUTOMATION_CONCURRENCY_MODES.to_vec(),
        execution_statuses: AUTOMATION_EXECUTION_STATUSES.to_vec(),
        statuses: AUTOMATION_STATUSES.to_vec(),
        timezones: vec!["UTC".into()],
    }
}

pub fn automation_concurrency_mode_to_storage(mode: AutomationConcurrencyMode) -> &'static str {
    match mode {
        AutomationConcurrencyMode::Serial => "Serial",
        AutomationConcurrencyMode::Parallel => "Parallel",
    }
}

pub fn automation_concurrency_mode_from_storage(
    value: &str,
) -> Result<AutomationConcurrencyMode, AutomationError> {
    match value {
        "Serial" => Ok(AutomationConcurrencyMode::Serial),
        "Parallel" => Ok(AutomationConcurrencyMode::Parallel),
        _ => Err(AutomationError::StorageFailure),
    }
}

pub fn automation_status_to_storage(status: AutomationStatus) -> &'static str {
    match status {
        AutomationStatus::Active => "Active",
        AutomationStatus::Paused => "Paused",
        AutomationStatus::Archived => "Archived",
    }
}

pub fn automation_status_from_storage(value: &str) -> Result<AutomationStatus, AutomationError> {
    match value {
        "Active" => Ok(AutomationStatus::Active),
        "Paused" => Ok(AutomationStatus::Paused),
        "Archived" => Ok(AutomationStatus::Archived),
        _ => Err(AutomationError::StorageFailure),
    }
}

pub fn automation_execution_status_to_storage(status: AutomationExecutionStatus) -> &'static str {
    match status {
        AutomationExecutionStatus::Pending => "Pending",
        AutomationExecutionStatus::Running => "Running",
        AutomationExecutionStatus::Success => "Success",
        AutomationExecutionStatus::Failed => "Failed",
        AutomationExecutionStatus::Canceled => "Canceled",
        AutomationExecutionStatus::Skipped => "Skipped",
    }
}

pub fn automation_execution_status_from_storage(
    value: &str,
) -> Result<AutomationExecutionStatus, AutomationError> {
    match value {
        "Pending" => Ok(AutomationExecutionStatus::Pending),
        "Running" => Ok(AutomationExecutionStatus::Running),
        "Success" => Ok(AutomationExecutionStatus::Success),
        "Failed" => Ok(AutomationExecutionStatus::Failed),
        "Canceled" => Ok(AutomationExecutionStatus::Canceled),
        "Skipped" => Ok(AutomationExecutionStatus::Skipped),
        _ => Err(AutomationError::StorageFailure),
    }
}

pub fn automation_trigger_source_to_storage(source: AutomationTriggerSource) -> &'static str {
    match source {
        AutomationTriggerSource::Scheduler => "Scheduler",
        AutomationTriggerSource::Manual => "Manual",
    }
}

pub fn automation_trigger_source_from_storage(
    value: &str,
) -> Result<AutomationTriggerSource, AutomationError> {
    match value {
        "Scheduler" => Ok(AutomationTriggerSource::Scheduler),
        "Manual" => Ok(AutomationTriggerSource::Manual),
        _ => Err(AutomationError::StorageFailure),
    }
}
