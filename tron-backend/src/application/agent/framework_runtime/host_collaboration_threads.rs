//! Runtime bindings over durable Thread identities. Native sessions use shared storage.
use super::*;
use eliteforge_agent_framework::RunControl;
use std::sync::RwLock;
use tokio::sync::watch;
#[path = "host_collaboration_thread_store.rs"]
mod store;
use store::ThreadRecord;
pub(super) use store::owner;

#[path = "host_collaboration_thread_feedback.rs"]
mod feedback;
#[path = "host_collaboration_thread_intervention.rs"]
mod intervention;
pub use feedback::{AssistantThreadCatalogItem, ThreadFeedback, ThreadFeedbackSink};

pub(in crate::application::agent::framework_runtime::host) type Threads =
    RwLock<BTreeMap<String, Arc<Thread>>>;
type Completion = (fw::ThreadStatus, Result<fw::ThreadResult, String>);
pub(in crate::application::agent::framework_runtime::host) struct Thread {
    context: AgentRunContext,
    agent: Agent,
    transition: Mutex<()>,
    state: std::sync::Mutex<ThreadState>,
}
struct ThreadState {
    summary: fw::ThreadSummary,
    completion: watch::Receiver<Option<Completion>>,
    origin_execution_id: fw::RunId,
    record: ThreadRecord,
    /// Control token of the run currently owned by the background wait task.
    /// Session management uses it to stop a live thread explicitly.
    control: Option<RunControl>,
}

impl Runtime {
    async fn thread(&self, id: &str) -> Result<Arc<Thread>, fw::AgentError> {
        let host = self.host()?;
        let cached = host
            .0
            .collaboration_threads
            .read()
            .unwrap()
            .get(id)
            .cloned();
        if let Some(thread) = cached {
            let authorized = {
                let state = thread.state.lock().unwrap();
                !state.record.closed && state.record.authorized(&self.context)
            };
            if !authorized {
                return Err(failure("thread not found"));
            }
            self.authorize(
                thread
                    .context
                    .agent_code
                    .as_deref()
                    .ok_or_else(|| failure("thread target missing"))?,
            )?;
            return Ok(thread);
        }
        let record = host
            .thread_record(id)
            .await?
            .filter(|record| !record.closed && record.authorized(&self.context))
            .ok_or_else(|| failure("thread not found"))?;
        self.authorize(&record.agent_code)?;
        let mut context = self.context.clone();
        context.agent_code = Some(record.agent_code.clone());
        context.conversation_code = Some(id.into());
        context.stream_key = Some(id.into());
        context.execution_scope = record.execution_scope.clone();
        context.collaboration_owner = Some(record.owner.clone());
        let agent = host
            .collaboration_agent(
                &record.workspace_code,
                &record.agent_code,
                self.builder.agent_management(),
            )
            .await?;
        let (_, completion) = watch::channel(record.completion.clone());
        let thread = Arc::new(Thread {
            context,
            agent,
            transition: Mutex::new(()),
            state: std::sync::Mutex::new(ThreadState {
                summary: record.summary.clone(),
                completion,
                origin_execution_id: fw::RunId::nil(),
                record,
                control: None,
            }),
        });
        let thread = host
            .0
            .collaboration_threads
            .write()
            .unwrap()
            .entry(id.into())
            .or_insert_with(|| thread.clone())
            .clone();
        Ok(thread)
    }
    async fn ensure_registered(
        &self,
        id: &str,
        expected: &Arc<Thread>,
    ) -> Result<(), fw::AgentError> {
        if !Arc::ptr_eq(&self.thread(id).await?, expected) {
            return Err(failure("thread not found"));
        }
        Ok(())
    }
    async fn launch(
        &self,
        thread: Arc<Thread>,
        request: fw::RunRequest,
        parent: Option<&fw::RunContext>,
        cause: fw::ThreadRunCause,
        interaction_id: Option<fw::RunId>,
    ) -> Result<fw::ThreadSummary, fw::AgentError> {
        let host = self.host()?;
        let _admission = host.0.config_lock.lock().await;
        host.ensure_open().map_err(failure)?;
        self.authorize(
            thread
                .context
                .agent_code
                .as_deref()
                .ok_or_else(|| failure("thread target missing"))?,
        )?;
        let run_id = request.run_id.unwrap_or_else(fw::RunId::new_v4);
        let mut request = request;
        request.run_id = Some(run_id);
        let mut record = thread.state.lock().unwrap().record.clone();
        record.execution_sequence = record
            .execution_sequence
            .checked_add(1)
            .ok_or_else(|| failure("Thread execution sequence overflow"))?;
        record.run_admitted_at_ms = fw::lifecycle_time_ms();
        record.resumed_interaction_id = interaction_id;
        record.summary.status = fw::ThreadStatus::Running;
        record.summary.latest_run_id = Some(run_id);
        record.completion = None;
        record.feedback_pending = false;
        host.persist_thread(&mut record, false).await?;
        thread.state.lock().unwrap().record = record;
        let handle = match thread.agent.start(request) {
            Ok(handle) => handle,
            Err(error) => {
                let mut record = thread.state.lock().unwrap().record.clone();
                record.summary.status = fw::ThreadStatus::Failed;
                record.completion = Some((fw::ThreadStatus::Failed, Err(error.to_string())));
                record.feedback_pending = record.owner.kind == "ASSISTANT";
                host.persist_thread(&mut record, false).await?;
                let mut state = thread.state.lock().unwrap();
                state.summary = record.summary.clone();
                let (_, completion) = watch::channel(record.completion.clone());
                state.completion = completion;
                state.record = record;
                return Err(error);
            }
        };
        let (sender, receiver) = watch::channel(None);
        // Retain the run's control token before the handle moves into the
        // background owner; session management cancels live threads through it.
        let control = thread.agent.control(&handle).ok();
        let summary = {
            let mut state = thread.state.lock().unwrap();
            state.summary.status = fw::ThreadStatus::Running;
            state.summary.latest_run_id = Some(run_id);
            state.completion = receiver;
            state.origin_execution_id =
                parent.map_or_else(fw::RunId::nil, fw::RunContext::execution_id);
            state.control = control;
            state.summary.clone()
        };
        let origin = parent.cloned();
        let returned = summary.clone();
        let feedback_host = host.clone();
        let execution_sequence = thread.state.lock().unwrap().record.execution_sequence;
        // Move the RunHandle into its background owner before another await.
        // Cancelling the initiating tool/Run must not cancel this independent Thread.
        crate::app::telemetry::spawn_traced(async move {
            if let Some(origin) = &origin {
                origin
                    .emit(fw::AgentEvent::Collaboration(Box::new(
                        fw::CollaborationEvent::ThreadRunStarted {
                            session_id: summary.session_id.clone(),
                            run_id,
                            cause,
                            interaction_id,
                        },
                    )))
                    .await;
            }
            let result = thread
                .agent
                .wait(handle)
                .await
                .map(|result| fw::ThreadResult::from_run(run_id, &result));
            let status = match &result {
                Ok(result) => fw::ThreadStatus::from(&result.status),
                Err(error) if matches!(error.primary(), fw::AgentError::Cancelled) => {
                    fw::ThreadStatus::Cancelled
                }
                Err(_) => fw::ThreadStatus::Failed,
            };
            let _transition = thread.transition.lock().await;
            let mut record = thread.state.lock().unwrap().record.clone();
            if record.execution_sequence == execution_sequence
                && record.summary.latest_run_id == Some(run_id)
            {
                record.summary.status = status.clone();
                record.completion = Some((
                    status.clone(),
                    result
                        .as_ref()
                        .map(Clone::clone)
                        .map_err(ToString::to_string),
                ));
                record.feedback_pending = record.owner.kind == "ASSISTANT";
                // Completion and pending feedback are one durable write. Sink failure is retried.
                if let Err(error) = feedback_host.persist_thread(&mut record, false).await {
                    tracing::error!(session_id = %record.summary.session_id, %error, "persist Thread completion failed; native run remains authoritative");
                } else {
                    let mut state = thread.state.lock().unwrap();
                    state.summary.status = status.clone();
                    state.record = record;
                }
            }
            if let Err(error) = feedback_host.flush_thread_feedback().await {
                tracing::warn!(%error, "Thread feedback remains pending");
            }
            if let Some(origin) = &origin {
                if thread
                    .context
                    .collaboration_owner
                    .as_ref()
                    .is_none_or(|owner| owner.kind != "ASSISTANT")
                {
                    if let Ok(result) = &result {
                        if result.interactions.iter().any(|interaction| {
                            interaction
                                .presentation
                                .as_ref()
                                .is_some_and(|value| value["kind"] == "ask_caller")
                        }) {
                            feedback_host.notify_native_run(
                                &origin.run_id.to_string(),
                                &tron_i18n::t!(
                                    "agent_runtime.collaboration.thread_awaiting_answer",
                                    locale = thread.context.locale.as_str(),
                                    thread_id = summary.session_id,
                                    interactions = serde_json::to_string(&result.interactions)
                                        .unwrap_or_default()
                                ),
                                thread.context.locale.as_str(),
                            );
                        }
                    }
                }
                origin
                    .emit(fw::AgentEvent::Collaboration(Box::new(
                        fw::CollaborationEvent::ThreadRunFinished {
                            session_id: summary.session_id,
                            run_id,
                            status: status.clone(),
                            interactions: result
                                .as_ref()
                                .map(|r| r.interactions.clone())
                                .unwrap_or_default(),
                            error: result.as_ref().err().map(ToString::to_string),
                        },
                    )))
                    .await;
            }
            sender.send_replace(Some((status, result.map_err(|error| error.to_string()))));
        });
        Ok(returned)
    }
}

#[async_trait::async_trait]
impl AgentCollaborationRuntime for Runtime {
    async fn agent_details(
        &self,
        code: &str,
        parent: &fw::RunContext,
    ) -> Result<serde_json::Value, fw::AgentError> {
        self.read_agent_details(code, parent).await
    }

    async fn spawn_child(
        &self,
        request: fw::RunRequest,
        parent: &fw::RunContext,
    ) -> Result<fw::ChildRunResult, fw::AgentError> {
        let summary = self.spawn_registered_child(request, parent, false).await?;
        self.wait_detached_child(&summary.child_run_id.to_string())
            .await
    }
    async fn spawn_child_detached(
        &self,
        request: fw::RunRequest,
        parent: &fw::RunContext,
    ) -> Result<fw::DetachedChildSummary, fw::AgentError> {
        self.spawn_detached_child(request, parent).await
    }
    async fn wait_child(
        &self,
        child_run_id: fw::RunId,
        _parent: &fw::RunContext,
    ) -> Result<fw::ChildRunResult, fw::AgentError> {
        self.wait_detached_child(&child_run_id.to_string()).await
    }
    async fn send_child(
        &self,
        child_run_id: fw::RunId,
        message: String,
        _parent: &fw::RunContext,
    ) -> Result<fw::DetachedChildSummary, fw::AgentError> {
        self.send_detached_child(&child_run_id.to_string(), &message)
            .await
    }
    async fn answer_child(
        &self,
        child_run_id: fw::RunId,
        interaction: fw::RunId,
        value: serde_json::Value,
        parent: &fw::RunContext,
    ) -> Result<fw::DetachedChildSummary, fw::AgentError> {
        self.answer_detached_child(&child_run_id.to_string(), interaction, value, parent)
            .await
    }
    async fn cancel_child(
        &self,
        child_run_id: fw::RunId,
        _parent: &fw::RunContext,
    ) -> Result<fw::DetachedChildSummary, fw::AgentError> {
        self.cancel_detached_child(&child_run_id.to_string()).await
    }
    async fn read_child(
        &self,
        child_run_id: fw::RunId,
        limit: usize,
    ) -> Result<fw::ChildRunView, fw::AgentError> {
        self.read_detached_child(&child_run_id.to_string(), limit)
            .await
    }
    async fn list_children(
        &self,
        _parent: &fw::RunContext,
    ) -> Result<Vec<fw::DetachedChildSummary>, fw::AgentError> {
        self.list_detached_children().await
    }
    async fn create_thread(
        &self,
        request: fw::RunRequest,
        title: Option<String>,
        parent: &fw::RunContext,
    ) -> Result<fw::ThreadSummary, fw::AgentError> {
        if title
            .as_ref()
            .is_some_and(|s| s.trim().is_empty() || s.len() > 200)
        {
            return Err(failure("invalid thread title"));
        }
        let id = fw::RunId::new_v4().to_string();
        let (agent, request, prepared) = self
            .prepare_new(request, id.clone(), parent, ExecutionLimits::Request)
            .await?;
        let (_, completion) = watch::channel(None);
        let mut record = ThreadRecord {
            summary: fw::ThreadSummary {
                session_id: id.clone(),
                title: title.clone(),
                status: fw::ThreadStatus::Running,
                latest_run_id: request.run_id,
            },
            workspace_code: prepared.workspace_code.clone(),
            user_code: prepared.user_code.clone(),
            agent_code: prepared
                .agent_code
                .clone()
                .ok_or_else(|| failure("thread target missing"))?,
            owner: owner(&self.context),
            initiator_agent_code: self.context.agent_code.clone(),
            origin_session_code: self
                .context
                .collaboration_owner
                .as_ref()
                .and_then(|owner| owner.origin_session_code.clone())
                .unwrap_or_else(|| parent.session_id.clone()),
            execution_scope: prepared.execution_scope.clone(),
            completion: None,
            closed: false,
            feedback_pending: false,
            revision: 0,
            run_admitted_at_ms: fw::lifecycle_time_ms(),
            execution_sequence: 0,
            resumed_interaction_id: None,
        };
        let host = self.host()?;
        let admission = host.0.config_lock.lock().await;
        if host.0.collaboration_threads.read().unwrap().len() >= 128 {
            return Err(failure(
                "thread capacity exceeded; close completed threads first",
            ));
        }
        host.persist_thread(&mut record, true).await?;
        let thread = Arc::new(Thread {
            context: prepared.clone(),
            agent,
            transition: Mutex::new(()),
            state: std::sync::Mutex::new(ThreadState {
                summary: fw::ThreadSummary {
                    session_id: id.clone(),
                    title,
                    status: fw::ThreadStatus::Running,
                    latest_run_id: request.run_id,
                },
                completion,
                origin_execution_id: parent.execution_id(),
                record,
                control: None,
            }),
        });
        host.0
            .collaboration_threads
            .write()
            .unwrap()
            .insert(id.clone(), thread.clone());
        drop(admission);
        let _transition = thread.transition.lock().await;
        let created = thread.state.lock().unwrap().summary.clone();
        parent
            .emit(fw::AgentEvent::Collaboration(Box::new(
                fw::CollaborationEvent::ThreadCreated { summary: created },
            )))
            .await;
        match self
            .launch(
                thread.clone(),
                request,
                Some(parent),
                fw::ThreadRunCause::Created,
                None,
            )
            .await
        {
            Ok(summary) => Ok(summary),
            Err(error) => {
                host.0.collaboration_threads.write().unwrap().remove(&id);
                Err(error)
            }
        }
    }
    async fn send_thread(
        &self,
        id: &str,
        request: fw::RunRequest,
        parent: &fw::RunContext,
    ) -> Result<fw::ThreadSummary, fw::AgentError> {
        let thread = self.thread(id).await?;
        let _transition = thread.transition.lock().await;
        self.ensure_registered(id, &thread).await?;
        {
            let state = thread.state.lock().unwrap();
            if state.summary.status == fw::ThreadStatus::Running {
                return Err(fw::AgentError::SessionBusy(id.into()));
            }
            if state.summary.status == fw::ThreadStatus::AwaitingInput {
                return Err(failure("thread is awaiting input; use thread_answer"));
            }
        }
        if request.agent_code.is_some() {
            return Err(failure("thread target cannot be changed"));
        }
        let (_, request, _) = self
            .prepare_existing(
                request,
                id.into(),
                &thread.context,
                parent,
                ExecutionLimits::Request,
            )
            .await?;
        self.launch(
            thread.clone(),
            request,
            Some(parent),
            fw::ThreadRunCause::Message,
            None,
        )
        .await
    }
    async fn answer_thread(
        &self,
        id: &str,
        interaction: fw::RunId,
        value: serde_json::Value,
        parent: &fw::RunContext,
    ) -> Result<fw::ThreadSummary, fw::AgentError> {
        let thread = self.thread(id).await?;
        let _transition = thread.transition.lock().await;
        self.ensure_registered(id, &thread).await?;
        if thread.state.lock().unwrap().summary.status != fw::ThreadStatus::AwaitingInput {
            return Err(failure("thread is not awaiting input"));
        }
        {
            let state = thread.state.lock().unwrap();
            if !state.record.completion.as_ref().is_some_and(|(_, result)| {
                result.as_ref().is_ok_and(|result| {
                    result
                        .interactions
                        .iter()
                        .any(|request| request.id == interaction)
                })
            }) {
                return Err(failure(
                    "interaction is not pending on the current thread execution",
                ));
            }
        }
        let request = thread
            .agent
            .answer_with_provenance(
                id,
                interaction,
                value,
                Some(self.answer_provenance(parent, interaction)),
            )
            .await?;
        let (_, request, _) = self
            .prepare_existing(
                request,
                id.into(),
                &thread.context,
                parent,
                ExecutionLimits::Request,
            )
            .await?;
        self.launch(
            thread.clone(),
            request,
            Some(parent),
            fw::ThreadRunCause::InteractionResponse,
            Some(interaction),
        )
        .await
    }
    async fn wait_thread(
        &self,
        id: &str,
        run_id: Option<fw::RunId>,
        parent: &fw::RunContext,
    ) -> Result<fw::ThreadResult, fw::AgentError> {
        let thread = self.thread(id).await?;
        let (mut completion, origin_execution_id) = {
            let state = thread.state.lock().unwrap();
            if run_id.is_some_and(|id| Some(id) != state.summary.latest_run_id) {
                return Err(failure("requested run is not the latest thread run"));
            }
            (state.completion.clone(), state.origin_execution_id)
        };
        loop {
            let completed = completion.borrow_and_update().clone();
            if let Some((status, result)) = completed {
                self.thread(id).await?;
                if parent.execution_id() != origin_execution_id {
                    let run_id = run_id
                        .or_else(|| result.as_ref().ok().map(|r| r.run_id))
                        .or_else(|| thread.state.lock().unwrap().summary.latest_run_id)
                        .ok_or_else(|| failure("thread has no run"))?;
                    parent
                        .emit(fw::AgentEvent::Collaboration(Box::new(
                            fw::CollaborationEvent::ThreadRunFinished {
                                session_id: id.into(),
                                run_id,
                                status,
                                interactions: result
                                    .as_ref()
                                    .map(|r| r.interactions.clone())
                                    .unwrap_or_default(),
                                error: result.as_ref().err().cloned(),
                            },
                        )))
                        .await;
                }
                return result.map_err(fw::AgentError::Run);
            }
            completion
                .changed()
                .await
                .map_err(|_| failure("thread execution owner terminated"))?;
        }
    }
    async fn read_thread(&self, id: &str, limit: usize) -> Result<fw::ThreadView, fw::AgentError> {
        let thread = self.thread(id).await?;
        let mut conversation = thread
            .agent
            .conversation(&fw::ConversationKey::new(id))
            .await?;
        if conversation.messages.len() > limit {
            conversation.messages = conversation
                .messages
                .split_off(conversation.messages.len() - limit);
        }
        self.thread(id).await?;
        let summary = thread.state.lock().unwrap().summary.clone();
        Ok(fw::ThreadView {
            summary,
            conversation,
        })
    }
    async fn list_threads(
        &self,
        _parent: &fw::RunContext,
    ) -> Result<Vec<fw::ThreadSummary>, fw::AgentError> {
        let host = self.host()?;
        let authorized: std::collections::HashSet<_> = self
            .directory()?
            .into_iter()
            .map(|entry| entry.agent_code)
            .collect();
        Ok(host
            .thread_records()
            .await?
            .into_iter()
            .filter(|record| {
                !record.closed
                    && record.authorized(&self.context)
                    && authorized.contains(&record.agent_code)
            })
            .map(|record| record.summary)
            .collect())
    }
    async fn close_thread(&self, id: &str, parent: &fw::RunContext) -> Result<(), fw::AgentError> {
        let thread = self.thread(id).await?;
        let _transition = thread.transition.lock().await;
        self.ensure_registered(id, &thread).await?;
        if thread.state.lock().unwrap().summary.status == fw::ThreadStatus::Running {
            return Err(fw::AgentError::SessionBusy(id.into()));
        }
        let host = self.host()?;
        let mut record = thread.state.lock().unwrap().record.clone();
        host.invalidate_file_session(id)
            .await
            .map_err(fw::AgentError::Run)?;
        record.closed = true;
        host.persist_thread(&mut record, false).await?;
        thread.state.lock().unwrap().record = record;
        host.0.collaboration_threads.write().unwrap().remove(id);
        parent
            .emit(fw::AgentEvent::Collaboration(Box::new(
                fw::CollaborationEvent::ThreadClosed {
                    session_id: id.into(),
                },
            )))
            .await;
        Ok(())
    }
}

impl FrameworkHost {
    /// Stop the live Threads a chat session spawned. Durable records for
    /// threads no longer owned by this process are settled by background
    /// recovery, so only in-process runs are addressed here.
    pub async fn cancel_threads_for_sessions(
        &self,
        origin_session_codes: &[String],
    ) -> Result<usize, fw::AgentError> {
        if origin_session_codes.is_empty() {
            return Ok(0);
        }
        let running = self
            .running_thread_ids_for_origins(origin_session_codes)
            .await?;
        if running.is_empty() {
            return Ok(0);
        }
        let threads = self.0.collaboration_threads.read().unwrap();
        let mut cancelled = 0;
        for id in running {
            let Some(thread) = threads.get(&id) else {
                continue;
            };
            let control = thread.state.lock().unwrap().control.clone();
            let Some(control) = control else {
                continue;
            };
            if let Err(error) = thread.agent.cancel(&control) {
                tracing::warn!(thread_id = %id, %error, "thread cancellation rejected");
                continue;
            }
            cancelled += 1;
        }
        Ok(cancelled)
    }

    /// Origins that still have at least one running Thread, restricted to the
    /// requested origin set. Session management polls this while waiting for
    /// stop requests to settle.
    pub async fn running_thread_origins(&self, origin_session_codes: &[String]) -> Vec<String> {
        if origin_session_codes.is_empty() {
            return Vec::new();
        }
        self.running_thread_origins_for_sessions(origin_session_codes)
            .await
            .unwrap_or_default()
    }
}
