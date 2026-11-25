use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    fmt::{self, Display, Formatter},
    sync::{Arc, Mutex, Weak},
};

use tokio::sync::watch;
#[path = "active_run_group.rs"]
mod group;

use crate::domain::chat::ChatMessageAttachment;

#[derive(Debug, Clone, PartialEq)]
pub struct AgentSteerInput {
    pub page_context: Option<super::dto::AgentPageContext>,
    pub annotations: Vec<eliteforge_agent_core::MessageAnnotation>,
    pub steer_code: String,
    pub requested_skills: Vec<String>,
    pub content: String,
    pub attachments: Vec<ChatMessageAttachment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveRunError {
    AlreadyActive,
    SessionArchived,
    NoActiveRun,
    RunMismatch { active_run_code: String },
    RuntimeUnsupported { runtime_adapter_key: String },
    RunStarting,
    RunFinishing,
    RegistryUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveRunState {
    Running,
    Finishing,
}

impl ActiveRunState {
    pub const fn code(self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Finishing => "FINISHING",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRunSnapshot {
    pub run_code: String,
    pub runtime_adapter_key: String,
    pub supports_steer: bool,
    pub operation_kind: Option<String>,
    pub state: ActiveRunState,
}

impl Display for ActiveRunError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyActive => formatter.write_str("session already has an active run"),
            Self::SessionArchived => formatter.write_str("session is archived"),
            Self::NoActiveRun => formatter.write_str("session has no active run"),
            Self::RunMismatch { active_run_code } => write!(
                formatter,
                "expected run does not match active run `{active_run_code}`"
            ),
            Self::RuntimeUnsupported {
                runtime_adapter_key,
            } => write!(
                formatter,
                "runtime adapter `{runtime_adapter_key}` does not support steer"
            ),
            Self::RunStarting => formatter.write_str("active run is starting"),
            Self::RunFinishing => formatter.write_str("active run is finishing"),
            Self::RegistryUnavailable => formatter.write_str("active run registry is unavailable"),
        }
    }
}

impl Error for ActiveRunError {}

#[derive(Clone, Default)]
pub struct ActiveRunRegistry {
    inner: Arc<ActiveRunRegistryInner>,
}

#[derive(Default)]
struct ActiveRunRegistryInner {
    entries: Mutex<BTreeMap<String, Arc<ActiveRunEntry>>>,
    admission: Mutex<()>,
    archived_sessions: Mutex<std::collections::BTreeSet<String>>,
    groups: Mutex<BTreeMap<String, Arc<group::ActiveRunGroup>>>,
}

struct ActiveRunEntry {
    run_code: String,
    runtime_adapter_key: String,
    supports_steer: bool,
    cancellation_sender: watch::Sender<bool>,
    pending_input: tokio::sync::Notify,
    state: Mutex<ActiveRunMutableState>,
}

#[derive(Default)]
struct ActiveRunMutableState {
    starting: bool,
    finishing: bool,
    context_compaction: bool,
    compaction_phase: bool,
    pending: VecDeque<AgentSteerInput>,
}

pub struct ActiveRunGuard {
    registry: Weak<ActiveRunRegistryInner>,
    entry: Arc<ActiveRunEntry>,
    session_code: String,
}

/// Serializes a multi-session mutation with all run admission.
pub(crate) struct ActiveRunAdmission<'a> {
    registry: &'a ActiveRunRegistry,
    _guard: std::sync::MutexGuard<'a, ()>,
}

impl ActiveRunAdmission<'_> {
    pub(crate) fn ensure_idle(&self, session_code: &str) -> Result<(), ActiveRunError> {
        if self
            .registry
            .inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .contains(session_code)
        {
            return Err(ActiveRunError::SessionArchived);
        }
        if self
            .registry
            .inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .contains_key(session_code)
            || self.registry.group_snapshot(session_code)?.is_some()
        {
            return Err(ActiveRunError::AlreadyActive);
        }
        Ok(())
    }

    pub(crate) fn retire(&self, sessions: &[String]) -> Result<(), ActiveRunError> {
        let mut retired = self
            .registry
            .inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        retired.extend(sessions.iter().cloned());
        Ok(())
    }

    pub(crate) fn reactivate(&self, session_code: &str) -> Result<(), ActiveRunError> {
        self.registry
            .inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .remove(session_code);
        Ok(())
    }
}

#[derive(Clone)]
pub struct ActiveRunControl {
    entry: Arc<ActiveRunEntry>,
}

pub struct ActiveRunCancellation {
    receiver: watch::Receiver<bool>,
}

impl ActiveRunRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn lock_admission(&self) -> Result<ActiveRunAdmission<'_>, ActiveRunError> {
        let guard = self
            .inner
            .admission
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        Ok(ActiveRunAdmission {
            registry: self,
            _guard: guard,
        })
    }

    /// Hold admission while changing session execution scope or placement.
    /// This closes the gap between checking for a run and persisting a change.
    pub(crate) fn idle_session_guard(
        &self,
        session_code: &str,
    ) -> Result<std::sync::MutexGuard<'_, ()>, ActiveRunError> {
        let guard = self
            .inner
            .admission
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if self
            .inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .contains(session_code)
        {
            return Err(ActiveRunError::SessionArchived);
        }
        if self
            .inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .contains_key(session_code)
            || self.group_snapshot(session_code)?.is_some()
        {
            return Err(ActiveRunError::AlreadyActive);
        }
        Ok(guard)
    }

    pub fn register(
        &self,
        session_code: &str,
        run_code: &str,
        runtime_adapter_key: &str,
        supports_steer: bool,
    ) -> Result<ActiveRunGuard, ActiveRunError> {
        self.insert(
            session_code,
            run_code,
            runtime_adapter_key,
            supports_steer,
            false,
            false,
        )
    }

    pub fn reserve(
        &self,
        session_code: &str,
        run_code: &str,
        runtime_adapter_key: &str,
        supports_steer: bool,
    ) -> Result<ActiveRunGuard, ActiveRunError> {
        self.insert(
            session_code,
            run_code,
            runtime_adapter_key,
            supports_steer,
            true,
            false,
        )
    }

    pub(crate) fn reserve_context_compaction(
        &self,
        session: &str,
        run: &str,
        adapter: &str,
        steer: bool,
    ) -> Result<ActiveRunGuard, ActiveRunError> {
        self.insert(session, run, adapter, steer, true, true)
    }

    fn insert(
        &self,
        session_code: &str,
        run_code: &str,
        runtime_adapter_key: &str,
        supports_steer: bool,
        starting: bool,
        context_compaction: bool,
    ) -> Result<ActiveRunGuard, ActiveRunError> {
        let _admission = self
            .inner
            .admission
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        let mut entries = self
            .inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if self
            .inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .contains(session_code)
        {
            return Err(ActiveRunError::SessionArchived);
        }
        if entries.contains_key(session_code) {
            return Err(ActiveRunError::AlreadyActive);
        }
        self.admit_group_child(session_code, run_code, runtime_adapter_key)?;
        let (cancellation_sender, _) = watch::channel(false);
        let entry = Arc::new(ActiveRunEntry {
            run_code: run_code.to_string(),
            runtime_adapter_key: runtime_adapter_key.to_string(),
            supports_steer,
            cancellation_sender,
            pending_input: tokio::sync::Notify::new(),
            state: Mutex::new(ActiveRunMutableState {
                starting,
                context_compaction,
                ..ActiveRunMutableState::default()
            }),
        });
        entries.insert(session_code.to_string(), entry.clone());
        Ok(ActiveRunGuard {
            registry: Arc::downgrade(&self.inner),
            entry,
            session_code: session_code.to_string(),
        })
    }

    /// Atomically block new runs for an idle session. Existing reservations
    /// win the race and make archiving fail with AlreadyActive.
    pub fn retire_session(&self, session_code: &str) -> Result<(), ActiveRunError> {
        let _admission = self
            .inner
            .admission
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if self
            .inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .contains_key(session_code)
            || self.group_snapshot(session_code)?.is_some()
        {
            return Err(ActiveRunError::AlreadyActive);
        }
        self.inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .insert(session_code.to_string());
        Ok(())
    }

    /// Reopen run admission after a successful restore or failed archive write.
    pub fn reactivate_session(&self, session_code: &str) -> Result<(), ActiveRunError> {
        let _admission = self
            .inner
            .admission
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        self.inner
            .archived_sessions
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .remove(session_code);
        Ok(())
    }

    pub fn is_retired(&self, session_code: &str) -> Result<bool, ActiveRunError> {
        self.inner
            .archived_sessions
            .lock()
            .map(|sessions| sessions.contains(session_code))
            .map_err(|_| ActiveRunError::RegistryUnavailable)
    }

    pub fn control(
        &self,
        session_code: &str,
        expected_run_code: &str,
    ) -> Result<ActiveRunControl, ActiveRunError> {
        let entry = self.entry(session_code)?;
        validate_expected_run(&entry, expected_run_code)?;
        Ok(ActiveRunControl { entry })
    }

    pub fn accept_steer<T, E, F>(
        &self,
        session_code: &str,
        expected_run_code: &str,
        accept: F,
    ) -> Result<Result<T, E>, ActiveRunError>
    where
        F: FnOnce() -> Result<(T, AgentSteerInput), E>,
    {
        let entry = self.entry(session_code)?;
        validate_expected_run(&entry, expected_run_code)?;
        entry.accept_steer(accept)
    }

    pub fn snapshot(
        &self,
        session_code: &str,
    ) -> Result<Option<ActiveRunSnapshot>, ActiveRunError> {
        loop {
            let entry = {
                self.inner
                    .entries
                    .lock()
                    .map_err(|_| ActiveRunError::RegistryUnavailable)?
                    .get(session_code)
                    .cloned()
            };
            let Some(entry) = entry else {
                return self.group_snapshot(session_code);
            };
            let (starting, finishing, compacting) = {
                let state = entry
                    .state
                    .lock()
                    .map_err(|_| ActiveRunError::RegistryUnavailable)?;
                (
                    state.starting,
                    state.finishing,
                    state.context_compaction || state.compaction_phase,
                )
            };
            let still_current = self
                .inner
                .entries
                .lock()
                .map_err(|_| ActiveRunError::RegistryUnavailable)?
                .get(session_code)
                .is_some_and(|current| Arc::ptr_eq(current, &entry));
            if still_current {
                return Ok(Some(ActiveRunSnapshot {
                    run_code: entry.run_code.clone(),
                    runtime_adapter_key: entry.runtime_adapter_key.clone(),
                    supports_steer: entry.supports_steer && !starting && !finishing && !compacting,
                    operation_kind: compacting.then(|| "ContextCompaction".into()),
                    state: if finishing {
                        ActiveRunState::Finishing
                    } else {
                        ActiveRunState::Running
                    },
                }));
            }
        }
    }

    pub fn request_stop(
        &self,
        session_code: &str,
        expected_run_code: &str,
    ) -> Result<ActiveRunSnapshot, ActiveRunError> {
        let group = self.stop_group(session_code, expected_run_code)?;
        let entry = match self.entry(session_code) {
            Ok(entry) => entry,
            Err(ActiveRunError::NoActiveRun) if group.is_some() => {
                return Ok(group.expect("group snapshot"));
            }
            Err(error) => return Err(error),
        };
        validate_expected_run(&entry, expected_run_code)?;
        {
            let mut state = entry
                .state
                .lock()
                .map_err(|_| ActiveRunError::RegistryUnavailable)?;
            if state.finishing || state.context_compaction || state.compaction_phase {
                return Err(ActiveRunError::RunFinishing);
            }
            state.finishing = true;
            state.pending.clear();
        }
        entry.cancellation_sender.send_replace(true);
        Ok(ActiveRunSnapshot {
            run_code: entry.run_code.clone(),
            runtime_adapter_key: entry.runtime_adapter_key.clone(),
            supports_steer: entry.supports_steer,
            operation_kind: None,
            state: ActiveRunState::Finishing,
        })
    }

    /// Cancel every in-process run before Electron tears down the profile
    /// sidecar. This gives the run executor a chance to persist an interrupted
    /// terminal state instead of leaving a run looking active after a switch.
    pub fn request_stop_all(&self) -> Result<usize, ActiveRunError> {
        let entries = self
            .inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        let mut stopped = self.stop_groups()?;
        for entry in entries.values() {
            let mut state = entry
                .state
                .lock()
                .map_err(|_| ActiveRunError::RegistryUnavailable)?;
            if !state.finishing {
                state.finishing = true;
                state.pending.clear();
                entry.cancellation_sender.send_replace(true);
                stopped += 1;
            }
        }
        Ok(stopped)
    }

    pub fn activate_reservation(
        &self,
        guard: &ActiveRunGuard,
        session_code: &str,
        run_code: &str,
        runtime_adapter_key: &str,
        supports_steer: bool,
    ) -> Result<(), ActiveRunError> {
        let registry = guard
            .registry
            .upgrade()
            .ok_or(ActiveRunError::RegistryUnavailable)?;
        if !Arc::ptr_eq(&registry, &self.inner) || guard.session_code != session_code {
            return Err(ActiveRunError::RegistryUnavailable);
        }
        validate_expected_run(&guard.entry, run_code)?;
        if guard.entry.runtime_adapter_key != runtime_adapter_key
            || guard.entry.supports_steer != supports_steer
        {
            return Err(ActiveRunError::RegistryUnavailable);
        }
        let is_current = self
            .inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .get(session_code)
            .is_some_and(|entry| Arc::ptr_eq(entry, &guard.entry));
        if !is_current {
            return Err(ActiveRunError::NoActiveRun);
        }
        let mut state = guard
            .entry
            .state
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if state.finishing {
            return Err(ActiveRunError::RunFinishing);
        }
        state.starting = false;
        Ok(())
    }

    fn entry(&self, session_code: &str) -> Result<Arc<ActiveRunEntry>, ActiveRunError> {
        self.inner
            .entries
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?
            .get(session_code)
            .cloned()
            .ok_or(ActiveRunError::NoActiveRun)
    }
}

impl ActiveRunEntry {
    fn accept_steer<T, E, F>(&self, accept: F) -> Result<Result<T, E>, ActiveRunError>
    where
        F: FnOnce() -> Result<(T, AgentSteerInput), E>,
    {
        if !self.supports_steer {
            return Err(ActiveRunError::RuntimeUnsupported {
                runtime_adapter_key: self.runtime_adapter_key.clone(),
            });
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if state.finishing || state.context_compaction || state.compaction_phase {
            return Err(ActiveRunError::RunFinishing);
        }
        if state.starting {
            return Err(ActiveRunError::RunStarting);
        }
        match accept() {
            Ok((value, input)) => {
                state.pending.push_back(input);
                self.pending_input.notify_one();
                Ok(Ok(value))
            }
            Err(error) => Ok(Err(error)),
        }
    }
}

impl ActiveRunControl {
    pub(super) async fn wait_for_pending(&self) -> Result<Vec<AgentSteerInput>, ActiveRunError> {
        loop {
            let notified = self.entry.pending_input.notified();
            let inputs = self.take_pending()?;
            if !inputs.is_empty() {
                return Ok(inputs);
            }
            notified.await;
        }
    }

    pub fn take_pending(&self) -> Result<Vec<AgentSteerInput>, ActiveRunError> {
        let mut state = self
            .entry
            .state
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if state.finishing || state.context_compaction || state.compaction_phase {
            return Ok(Vec::new());
        }
        Ok(state.pending.drain(..).collect())
    }

    pub fn take_pending_or_begin_finishing(&self) -> Result<Vec<AgentSteerInput>, ActiveRunError> {
        let mut state = self
            .entry
            .state
            .lock()
            .map_err(|_| ActiveRunError::RegistryUnavailable)?;
        if state.finishing {
            return Ok(Vec::new());
        }
        if state.pending.is_empty() {
            state.finishing = true;
            return Ok(Vec::new());
        }
        Ok(state.pending.drain(..).collect())
    }
}

impl ActiveRunGuard {
    pub fn cancellation(&self) -> ActiveRunCancellation {
        ActiveRunCancellation {
            receiver: self.entry.cancellation_sender.subscribe(),
        }
    }
}

impl ActiveRunCancellation {
    pub async fn cancelled(&mut self) {
        loop {
            if *self.receiver.borrow_and_update() {
                return;
            }
            if self.receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Drop for ActiveRunGuard {
    fn drop(&mut self) {
        if let Ok(mut state) = self.entry.state.lock() {
            state.finishing = true;
        }
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let Ok(mut entries) = registry.entries.lock() else {
            return;
        };
        let matches_guard = entries
            .get(&self.session_code)
            .is_some_and(|entry| Arc::ptr_eq(entry, &self.entry));
        if matches_guard {
            entries.remove(&self.session_code);
        }
    }
}

fn validate_expected_run(
    entry: &ActiveRunEntry,
    expected_run_code: &str,
) -> Result<(), ActiveRunError> {
    if entry.run_code == expected_run_code {
        return Ok(());
    }
    Err(ActiveRunError::RunMismatch {
        active_run_code: entry.run_code.clone(),
    })
}

impl eliteforge_agent_core::ContextCompactionObserver for ActiveRunControl {
    fn state_changed(&self, active: bool) {
        if let Ok(mut state) = self.entry.state.lock() {
            state.compaction_phase = active;
        }
    }
}
