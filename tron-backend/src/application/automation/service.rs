use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{SecondsFormat, Utc};
use cron::Schedule;

pub fn preview_schedule(expression: &str) -> Result<Vec<i64>, AutomationError> {
    let normalized = projection::normalized_scheduler_expression(expression)?;
    let schedule = Schedule::from_str(&normalized).map_err(|_| AutomationError::InvalidCron)?;
    Ok(schedule
        .upcoming(Utc)
        .take(5)
        .map(|time| time.timestamp_millis())
        .collect())
}
use tokio::task::JoinHandle;

use crate::{
    application::workspace::WorkspaceService,
    domain::{
        automation::{
            Automation, AutomationConcurrencyMode, AutomationDetail, AutomationError,
            AutomationExecutionDraft, AutomationExecutionListItem, AutomationExecutionListQuery,
            AutomationExecutionRecord, AutomationExecutionStats, AutomationExecutionStatus,
            AutomationLifecycleResult, AutomationListItem, AutomationListQuery, AutomationOptions,
            AutomationStatus, AutomationTriggerSource, PagedAutomationRecords,
            automation_execution_status_to_storage, automation_options,
        },
        workspace::{WorkspaceError, WorkspaceStatus},
    },
    shared::{
        event_bus::{BusEventDraft, EventBusHandle},
        id::SnowflakeId,
    },
};

mod projection;
mod runtime;
mod use_cases;

use self::{projection::*, runtime::AutomationRuntime};

pub const DEFAULT_AUTOMATION_AGENT_CODE: &str = "main-agent";

/// Platform targets reuse the automation scheduler without creating a user
/// automation or a user-owned chat. The callback may only enqueue durable work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinScheduleTarget {
    AssistantHeartbeat,
}
impl BuiltinScheduleTarget {
    fn registry_key(self) -> &'static str {
        match self {
            Self::AssistantHeartbeat => "builtin:assistant-heartbeat",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateAutomationCommand {
    pub workspace_code: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_code: Option<String>,
    pub model_code: Option<String>,
    pub concurrency_mode: Option<AutomationConcurrencyMode>,
    pub prompt: String,
    pub cron_expression: String,
    pub status: Option<AutomationStatus>,
}

#[derive(Debug, Clone)]
pub struct UpdateAutomationCommand {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub agent_code: Option<String>,
    pub model_code: Option<Option<String>>,
    pub concurrency_mode: Option<AutomationConcurrencyMode>,
    pub prompt: Option<String>,
    pub cron_expression: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateAutomationStatusCommand {
    pub status: AutomationStatus,
}

pub trait AutomationRepository: Send {
    fn list_automations(&mut self) -> Result<Vec<Automation>, AutomationError>;
    fn execution_stats(
        &mut self,
        automation_codes: &[String],
    ) -> Result<BTreeMap<String, AutomationExecutionStats>, AutomationError>;
    fn next_id(&mut self) -> Result<i64, AutomationError>;
    fn create_automation(&mut self, automation: &Automation) -> Result<(), AutomationError>;
    fn update_automation(&mut self, automation: &Automation) -> Result<(), AutomationError>;
    fn list_execution_records(
        &mut self,
        automation_code: &str,
    ) -> Result<Vec<AutomationExecutionRecord>, AutomationError>;
    fn record_execution(
        &mut self,
        automation: &Automation,
        draft: &AutomationExecutionDraft,
    ) -> Result<Option<AutomationExecutionRecord>, AutomationError>;
    fn link_execution_session(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        session_code: &str,
    ) -> Result<(), AutomationError>;
    fn recover_interrupted_executions(
        &mut self,
        finished_at: &str,
    ) -> Result<usize, AutomationError>;
    fn prepare_serial_execution(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        finished_at: &str,
    ) -> Result<Option<Vec<AutomationExecutionRecord>>, AutomationError>;
    fn complete_execution_record(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        status: AutomationExecutionStatus,
        summary: Option<String>,
        session_code: Option<String>,
        finished_at: &str,
    ) -> Result<(), AutomationError>;
    fn delete_execution_record(
        &mut self,
        automation_code: &str,
        execution_code: &str,
        deleted_at: &str,
    ) -> Result<AutomationExecutionRecord, AutomationError>;
}

pub trait AutomationExecutionRunner: Send + Sync {
    fn dispatch(&self, automation: Automation, execution: AutomationExecutionRecord);
}

#[derive(Clone)]
pub struct AutomationService {
    repository: Arc<Mutex<Box<dyn AutomationRepository>>>,
    runtime: AutomationRuntime,
    workspaces: WorkspaceService,
    event_bus: EventBusHandle,
    execution_runner: Arc<Mutex<Option<Arc<dyn AutomationExecutionRunner>>>>,
}

impl AutomationService {
    pub fn new(
        repository: impl AutomationRepository + 'static,
        workspaces: WorkspaceService,
    ) -> Self {
        Self::with_event_bus(repository, workspaces, EventBusHandle::disabled())
    }

    pub fn with_event_bus(
        repository: impl AutomationRepository + 'static,
        workspaces: WorkspaceService,
        event_bus: EventBusHandle,
    ) -> Self {
        Self {
            repository: Arc::new(Mutex::new(Box::new(repository))),
            runtime: AutomationRuntime::new(),
            workspaces,
            event_bus,
            execution_runner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn register_builtin_interval(
        &self,
        target: BuiltinScheduleTarget,
        period: Duration,
        enqueue: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<(), AutomationError> {
        self.runtime
            .register_builtin_interval(target, period, enqueue)
    }

    pub fn set_execution_runner(&self, runner: Arc<dyn AutomationExecutionRunner>) {
        *self
            .execution_runner
            .lock()
            .expect("automation execution runner mutex poisoned") = Some(runner);
    }

    fn execute_internal(
        &self,
        automation_code: &str,
        trigger_source: AutomationTriggerSource,
        scheduled_at: Option<String>,
    ) -> Result<Option<AutomationExecutionRecord>, AutomationError> {
        record_execution(
            self.repository.clone(),
            self.event_bus.clone(),
            self.execution_runner.clone(),
            automation_code,
            trigger_source,
            scheduled_at,
        )
    }

    pub fn complete_execution(
        &self,
        automation_code: &str,
        execution_code: &str,
        status: AutomationExecutionStatus,
        summary: Option<String>,
        session_code: Option<String>,
    ) -> Result<(), AutomationError> {
        let finished_at = current_timestamp();
        self.repository
            .lock()
            .expect("automation repository mutex poisoned")
            .complete_execution_record(
                automation_code,
                execution_code,
                status,
                summary,
                session_code,
                &finished_at,
            )
    }

    pub fn link_execution_session(
        &self,
        automation_code: &str,
        execution_code: &str,
        session_code: &str,
    ) -> Result<(), AutomationError> {
        self.repository
            .lock()
            .expect("automation repository mutex poisoned")
            .link_execution_session(automation_code, execution_code, session_code)
    }

    pub(crate) fn prepare_serial_execution(
        &self,
        automation_code: &str,
        execution_code: &str,
    ) -> Result<Option<Vec<AutomationExecutionRecord>>, AutomationError> {
        self.repository
            .lock()
            .expect("automation repository mutex poisoned")
            .prepare_serial_execution(automation_code, execution_code, &current_timestamp())
    }

    fn recover_interrupted_executions(&self, finished_at: &str) -> Result<usize, AutomationError> {
        self.repository
            .lock()
            .expect("automation repository mutex poisoned")
            .recover_interrupted_executions(finished_at)
    }

    fn sync_runtime(&self, automation: &Automation) -> Result<(), AutomationError> {
        if automation.status == AutomationStatus::Active {
            self.runtime.register(
                &automation.automation_code,
                &automation.scheduler_expression,
                self.repository.clone(),
                self.event_bus.clone(),
                self.execution_runner.clone(),
            )?;
        } else {
            self.runtime.unregister(&automation.automation_code);
        }
        Ok(())
    }

    fn filtered_automation_items(
        &self,
        workspace_code: &str,
        query: &AutomationListQuery,
    ) -> Result<Vec<AutomationListItem>, AutomationError> {
        let include_archived = query.include_archived.unwrap_or(false);
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_lowercase();
        let status = query.status;
        let filtered_automations = self
            .all_automations()?
            .into_iter()
            .filter(|automation| automation.workspace_code == workspace_code)
            .filter(|automation| {
                if let Some(status) = status {
                    return automation.status == status;
                }
                include_archived || automation.status != AutomationStatus::Archived
            })
            .filter(|automation| {
                if search.is_empty() {
                    return true;
                }
                [
                    automation.automation_code.as_str(),
                    automation.name.as_str(),
                    automation.description.as_deref().unwrap_or(""),
                    automation.last_execution_summary.as_deref().unwrap_or(""),
                ]
                .join(" ")
                .to_lowercase()
                .contains(&search)
            })
            .collect::<Vec<_>>();
        let automation_codes = filtered_automations
            .iter()
            .map(|automation| automation.automation_code.clone())
            .collect::<Vec<_>>();
        let stats_by_code = self
            .repository
            .lock()
            .expect("automation repository mutex poisoned")
            .execution_stats(&automation_codes)?;
        let mut records = filtered_automations
            .into_iter()
            .map(|automation| {
                let stats = stats_by_code
                    .get(&automation.automation_code)
                    .copied()
                    .unwrap_or_default();
                AutomationListItem {
                    automation_code: automation.automation_code,
                    workspace_code: automation.workspace_code,
                    name: automation.name,
                    description: automation.description,
                    agent_code: automation.agent_code,
                    model_code: automation.model_code,
                    cron_expression: automation.cron_expression,
                    timezone: automation.timezone,
                    status: automation.status,
                    concurrency_mode: automation.concurrency_mode,
                    last_execution_status: automation.last_execution_status,
                    last_execution_summary: automation.last_execution_summary,
                    last_execution_code: automation.last_execution_code,
                    last_started_at: automation.last_started_at,
                    last_finished_at: automation.last_finished_at,
                    next_run_at: next_run_preview(
                        &automation.scheduler_expression,
                        automation.status,
                    ),
                    execution_success_count: stats.success_count,
                    execution_total_count: stats.total_count,
                    execution_success_rate: stats.success_rate(),
                    updated_at: automation.updated_at,
                    archived_at: automation.archived_at,
                }
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(records)
    }

    fn detail_from_automation(
        &self,
        automation: Automation,
    ) -> Result<AutomationDetail, AutomationError> {
        Ok(AutomationDetail {
            id: automation.id.as_decimal_string(),
            automation_code: automation.automation_code,
            workspace_code: automation.workspace_code,
            name: automation.name,
            description: automation.description,
            agent_code: automation.agent_code,
            model_code: automation.model_code,
            prompt: automation.prompt_text,
            cron_expression: automation.cron_expression,
            scheduler_expression: automation.scheduler_expression.clone(),
            timezone: automation.timezone,
            status: automation.status,
            concurrency_mode: automation.concurrency_mode,
            previous_status: automation.previous_status,
            last_execution_status: automation.last_execution_status,
            last_execution_code: automation.last_execution_code,
            last_execution_summary: automation.last_execution_summary,
            last_started_at: automation.last_started_at,
            last_finished_at: automation.last_finished_at,
            next_run_at: next_run_preview(&automation.scheduler_expression, automation.status),
            created_at: automation.created_at,
            updated_at: automation.updated_at,
            archived_at: automation.archived_at,
        })
    }

    fn lifecycle_result(&self, automation: &Automation) -> AutomationLifecycleResult {
        AutomationLifecycleResult {
            archived_at: automation.archived_at.clone(),
            automation_code: automation.automation_code.clone(),
            previous_status: automation.previous_status,
            status: automation.status,
        }
    }

    fn get_automation_record(&self, automation_code: &str) -> Result<Automation, AutomationError> {
        let code = automation_code.trim();
        if code.is_empty() {
            return Err(AutomationError::NotFound);
        }
        self.all_automations()?
            .into_iter()
            .find(|automation| automation.automation_code == code)
            .ok_or(AutomationError::NotFound)
    }

    fn all_automations(&self) -> Result<Vec<Automation>, AutomationError> {
        self.repository
            .lock()
            .expect("automation repository mutex poisoned")
            .list_automations()
    }

    fn ensure_active_workspace(&self, workspace_code: &str) -> Result<(), AutomationError> {
        let detail = self
            .workspaces
            .get_workspace(workspace_code)
            .map_err(map_workspace_error)?;
        if detail.status != WorkspaceStatus::Active {
            return Err(AutomationError::InvalidWorkspace);
        }
        Ok(())
    }

    fn publish_automation_detail_event(
        &self,
        event_name: &'static str,
        summary: &'static str,
        detail: &AutomationDetail,
    ) {
        let _ = self.event_bus.publish(
            BusEventDraft::audit(event_name, "automation_service")
                .subject("automation", detail.automation_code.clone())
                .summary(summary)
                .attr("workspaceCode", detail.workspace_code.clone())
                .attr("status", format!("{:?}", detail.status))
                .attr("cronExpression", detail.cron_expression.clone())
                .attr("timezone", detail.timezone.clone()),
        );
    }

    fn publish_automation_lifecycle_event(
        &self,
        event_name: &'static str,
        summary: &'static str,
        result: &AutomationLifecycleResult,
    ) {
        let _ = self.event_bus.publish(
            BusEventDraft::audit(event_name, "automation_service")
                .subject("automation", result.automation_code.clone())
                .summary(summary)
                .attr("status", format!("{:?}", result.status))
                .attr(
                    "previousStatus",
                    result
                        .previous_status
                        .map(|status| format!("{status:?}"))
                        .unwrap_or_else(|| "None".into()),
                ),
        );
    }
}
