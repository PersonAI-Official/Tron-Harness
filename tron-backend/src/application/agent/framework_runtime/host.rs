use super::runtime_count_limit;
#[path = "command_capture.rs"]
mod command_capture;
use super::{
    context::TronContext,
    model::{HostExecution, HostRunState},
    tools::{AttachmentReader, HarnessTools, LocalScope, ToolAudit},
};
use crate::application::agent::{ActiveRunControl, ModelStreamSink, runtime::ReactAgent};
use crate::domain::agent::{AgentError, AgentRunContext, AgentRunResult};
use command_capture::CommandCapture;
use eliteforge_agent_core as fw;
use eliteforge_agent_framework::{Agent, AgentSelection, Framework};

use eliteforge_agent_official_plugin_tools_local::FileReferenceReaderService;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Mutex, OnceCell};
#[path = "host_cell.rs"]
mod host_cell;
use host_cell::RuntimeCell;

#[path = "host_capability_queue.rs"]
mod capability_queue;
#[path = "host_capability_state.rs"]
mod capability_state;
pub use capability_queue::{CapabilityQueueEnqueueError, CapabilityQueueSnapshot};
#[path = "host_collaboration.rs"]
pub(super) mod collaboration;
#[path = "host_subagents.rs"]
mod subagents;
pub use collaboration::{AssistantThreadCatalogItem, ThreadFeedback, ThreadFeedbackSink};
pub use subagents::{SubagentRecord, SubagentView};
#[path = "host_continuation.rs"]
mod host_continuation;
#[path = "host_control.rs"]
mod host_control;
#[path = "host_driver.rs"]
mod host_driver;
#[path = "host_environment.rs"]
mod host_environment;
#[path = "host_execution.rs"]
mod host_execution;
#[path = "host_sudo.rs"]
mod host_sudo;
#[path = "host_tool_catalog.rs"]
mod host_tool_catalog;
pub(crate) use host_sudo::{PendingSudoInput, SudoInputError};
#[path = "host_connector_tools.rs"]
mod connector_tools;
#[path = "host_external.rs"]
mod host_external;
#[path = "host_history.rs"]
mod host_history;
#[path = "host_import.rs"]
mod host_import;
#[path = "host_interactions.rs"]
mod host_interactions;
#[path = "host_navigation.rs"]
pub(super) mod host_navigation;
#[path = "host_pack_pins.rs"]
mod host_pack_pins;
#[path = "host_pages.rs"]
pub(super) mod host_pages;
#[path = "host_run.rs"]
mod host_run;
#[path = "host_shutdown.rs"]
mod host_shutdown;
#[path = "host_snapshot.rs"]
mod host_snapshot;
#[path = "host_storage.rs"]
mod host_storage;
#[path = "host_summary.rs"]
mod host_summary;
#[path = "host_file_coordination.rs"]
mod file_coordination;
#[path = "host_working_directory.rs"]
mod host_working_directory;
#[path = "host_remote_tools.rs"]
pub(super) mod remote_tools;
use capability_state::{load_capability_state, persist_capability_state};

#[derive(Clone)]
pub struct FrameworkHost(Arc<Host>);
struct Host {
    runtime_config:
        std::sync::RwLock<Option<crate::application::system_config::BackendRuntimeConfig>>,
    model_client: std::sync::RwLock<Option<Arc<dyn crate::application::agent::ModelClient>>>,
    personai: remote_tools::PersonAiState,
    path: PathBuf,
    state_path: PathBuf,
    pack_root: PathBuf,
    capability_state: std::sync::RwLock<BTreeMap<String, bool>>,
    extension_state: std::sync::RwLock<BTreeMap<String, bool>>,
    config_lock: tokio::sync::Mutex<()>,
    pty: eliteforge_agent_capability_tools::PtyService,
    framework: RuntimeCell<Framework>,
    storage: OnceCell<eliteforge_agent_official_plugin_storage_local::LocalStorageRuntime>,
    storage_framework: OnceCell<Framework>,
    storage_access: tokio::sync::RwLock<()>,
    runtime_access: Arc<tokio::sync::RwLock<()>>,
    telemetry: OnceCell<crate::app::telemetry::LocalTelemetry>,
    agent: RuntimeCell<Agent>,
    internal_agents: std::sync::RwLock<BTreeMap<(String, String), Arc<OnceCell<Agent>>>>,
    mcp: OnceCell<crate::application::mcp::McpService>,
    rag: OnceCell<crate::app::AppRagService>,
    skill_registry: OnceCell<crate::application::skill::SkillRegistryService>,
    prompt_service: OnceCell<crate::application::prompt::PromptService>,
    capability_resources: OnceCell<crate::application::capability::CapabilityResources>,
    capability_queue: OnceCell<crate::application::capability::CapabilityQueueService>,
    capability_queue_lock: tokio::sync::Mutex<()>,
    capability_queue_running: std::sync::Mutex<Option<String>>,
    navigation: OnceCell<crate::application::navigation::NavigationService>,
    skills: std::sync::RwLock<BTreeMap<String, Vec<String>>>,
    installed_packs: std::sync::RwLock<BTreeMap<String, Vec<String>>>,
    tool_catalog_issues: std::sync::RwLock<Vec<crate::domain::agent::ToolCatalogIssue>>,
    tool_catalog_metadata: std::sync::RwLock<host_tool_catalog::ToolCatalogMetadata>,
    tools_enabled: Arc<std::sync::atomic::AtomicBool>,
    observe_enabled: Arc<std::sync::atomic::AtomicBool>,
    external_agent: RuntimeCell<Agent>,
    continuation_agent: RuntimeCell<Agent>,
    database:
        OnceCell<crate::adapters::outbound::persistence::sqlite::framework_storage::SharedDatabase>,
    native_bindings: super::native_sessions::Bindings,
    sudo_inputs: host_sudo::SudoInputs,
    closed: std::sync::atomic::AtomicBool,
    control: host_control::NativeRuns,
    shutdown: host_shutdown::OwnedShutdown,
    presentation: std::sync::RwLock<Option<crate::application::agent::AgentAguiStreamService>>,
    resources: Option<crate::application::resource::ResourceService>,
    agent_management: OnceCell<crate::application::agent::AgentManagementService>,
    assistants: OnceCell<crate::application::assistant::AssistantService>,
    memories: OnceCell<crate::app::AppMemoryService>,
    knowledge_bases: OnceCell<crate::application::knowledge_base::KnowledgeBaseService>,
    topics: OnceCell<crate::application::topic::TopicService>,
    integrations: OnceCell<crate::application::connector::IntegrationService>,
    collaboration_builder:
        OnceCell<std::sync::Weak<crate::application::agent::AgentRunContextBuilder>>,
    collaboration_threads: collaboration::Threads,
    collaboration_children: collaboration::Children,
    thread_feedback: OnceCell<Arc<dyn ThreadFeedbackSink>>,
    _temporary: Option<tempfile::TempDir>,
}
impl FrameworkHost {
    pub(crate) fn telemetry_database_path(&self) -> std::path::PathBuf {
        self.0.path.with_extension("telemetry.sqlite3")
    }
    pub(crate) async fn telemetry(
        &self,
    ) -> Result<
        &crate::app::telemetry::LocalTelemetry,
        crate::application::observability::ObservabilityError,
    > {
        self.0
            .telemetry
            .get_or_try_init(|| async {
                crate::app::telemetry::LocalTelemetry::open(&self.telemetry_database_path()).await
            })
            .await
    }
    pub async fn observability(
        &self,
    ) -> Result<
        crate::application::observability::ObservabilityService,
        crate::application::observability::ObservabilityError,
    > {
        self.ensure_open()
            .map_err(|_| crate::application::observability::ObservabilityError::Storage)?;
        Ok(self.telemetry().await?.service())
    }

    pub async fn reset_observability(
        &self,
    ) -> Result<(), crate::application::observability::ObservabilityError> {
        use crate::application::observability::ObservabilityError;
        self.ensure_open()
            .map_err(|_| ObservabilityError::Storage)?;
        let telemetry = self.telemetry().await?;
        telemetry
            .runtime
            .force_flush()
            .await
            .map_err(|_| ObservabilityError::Storage)?;
        telemetry
            .store
            .clear()
            .await
            .map_err(|_| ObservabilityError::Storage)?;
        // The store recreates host-neutral tables; replay the Tron-specific
        // indexes the drop implicitly removed so observability queries keep
        // their original plans after a reset.
        telemetry.recreate_host_indexes().await
    }

    pub async fn collector_connection(
        &self,
    ) -> Result<
        crate::application::observability::CollectorConnection,
        crate::application::observability::ObservabilityError,
    > {
        self.ensure_open()
            .map_err(|_| crate::application::observability::ObservabilityError::Storage)?;
        Ok(self.telemetry().await?.connection())
    }
    pub fn bind_agent_management(
        &self,
        service: crate::application::agent::AgentManagementService,
    ) {
        self.0
            .agent_management
            .set(service)
            .unwrap_or_else(|_| panic!("agent management already bound"));
    }
    pub fn bind_prompt_service(&self, service: crate::application::prompt::PromptService) {
        self.0
            .prompt_service
            .set(service)
            .unwrap_or_else(|_| panic!("prompt service already bound"));
    }

    pub fn bind_skills(
        &self,
        service: crate::application::skill::SkillRegistryService,
        resources: crate::application::capability::CapabilityResources,
        mcp: crate::application::mcp::McpService,
        rag: crate::app::AppRagService,
    ) -> Result<(), String> {
        self.0.mcp.set(mcp).map_err(|_| "MCP already bound")?;
        self.0.rag.set(rag).map_err(|_| "RAG already bound")?;
        self.0
            .skill_registry
            .set(service)
            .map_err(|_| "skills already bound")?;
        self.0
            .capability_resources
            .set(resources)
            .map_err(|_| "capability resources already bound")?;
        // Framework extractions are rebuilt from current packages on demand;
        // drop copies made by earlier binaries so only current content remains.
        let _ = fs::remove_dir_all(
            self.0
                .path
                .with_extension("skills")
                .join("capability-packs"),
        );
        self.reconcile_capability_resources().map(|_| ())
    }
    pub(crate) fn bind_collaboration(
        &self,
        builder: &Arc<crate::application::agent::AgentRunContextBuilder>,
    ) -> Result<(), AgentError> {
        self.0
            .collaboration_builder
            .set(Arc::downgrade(builder))
            .map_err(|_| AgentError::InvalidConfiguration("collaboration is already bound".into()))
    }
    pub fn bind_runtime_config(
        &self,
        runtime: crate::application::system_config::BackendRuntimeConfig,
    ) {
        *self
            .0
            .runtime_config
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(runtime);
    }
    fn invocation_config(&self) -> crate::config::ModelInvocationConfig {
        self.0
            .runtime_config
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|runtime| runtime.snapshot().model_invocation)
            .unwrap_or_default()
    }

    /// Drain owned runs and command processes before shutting down a profile.
    pub fn pty(&self) -> eliteforge_agent_capability_tools::PtyService {
        self.0.pty.clone()
    }

    pub(crate) fn bind_assistants(&self, service: crate::application::assistant::AssistantService) {
        let _ = self.0.assistants.set(service);
    }
    pub(crate) fn assistants(&self) -> Option<crate::application::assistant::AssistantService> {
        self.0.assistants.get().cloned()
    }
    pub(crate) fn attach_memory_backend(
        &self,
        context: &crate::domain::agent::AgentRunContext,
        extensions: &mut eliteforge_agent_core::Extensions,
    ) {
        if let (Some(memory), Some(user)) = (self.memories(), context.user_code.as_deref()) {
            let binding = memory.bind_backend(
                crate::application::memory::MemoryRunContext::new(
                    user,
                    if context.is_owning_assistant() {
                        ""
                    } else {
                        context.execution_workspace_code()
                    },
                ),
                None,
                None,
            );
            extensions.insert(eliteforge_agent_core::LongTermMemoryService(
                binding.backend.clone(),
            ));
            extensions.insert(binding);
        }
    }
    pub(crate) fn bind_memories(&self, service: crate::app::AppMemoryService) {
        self.0.memories.set(service).ok();
    }
    pub(crate) fn memories(&self) -> Option<crate::app::AppMemoryService> {
        self.0.memories.get().cloned()
    }
    pub fn bind_knowledge_bases(
        &self,
        service: crate::application::knowledge_base::KnowledgeBaseService,
    ) -> Result<(), AgentError> {
        self.0.knowledge_bases.set(service).map_err(|_| {
            AgentError::InvalidConfiguration("knowledge base service is already bound".into())
        })
    }

    pub(crate) fn knowledge_bases(
        &self,
    ) -> Option<crate::application::knowledge_base::KnowledgeBaseService> {
        self.0.knowledge_bases.get().cloned()
    }

    pub fn bind_topics(
        &self,
        service: crate::application::topic::TopicService,
    ) -> Result<(), AgentError> {
        self.0
            .topics
            .set(service)
            .map_err(|_| AgentError::InvalidConfiguration("topic service is already bound".into()))
    }
    pub fn bind_integrations(
        &self,
        service: crate::application::connector::IntegrationService,
    ) -> Result<(), AgentError> {
        self.0.integrations.set(service).map_err(|_| {
            AgentError::InvalidConfiguration("connector service is already bound".into())
        })
    }

    pub(crate) fn topics(&self) -> Option<crate::application::topic::TopicService> {
        self.0.topics.get().cloned()
    }

    pub(crate) fn gateway_enabled(&self) -> bool {
        Self::gateway_enabled_for(&self.0.extension_state.read().unwrap())
    }

    pub(super) fn gateway_enabled_for(extensions: &BTreeMap<String, bool>) -> bool {
        extensions.get("tool").copied().unwrap_or(true)
    }

    pub fn capability_state(&self) -> (BTreeMap<String, bool>, BTreeMap<String, bool>) {
        (
            self.0.capability_state.read().unwrap().clone(),
            self.0.extension_state.read().unwrap().clone(),
        )
    }

    pub fn pack_root(&self) -> &Path {
        &self.0.pack_root
    }

    async fn invalidate_environment(&self) -> Result<(), String> {
        // New runs are excluded by config_lock; drain outstanding runtime
        // readers before closing the old assembly. Storage is not replaced.
        let _exclusive = self.0.runtime_access.write().await;
        // Preflight has excluded active and durable users. Release package runtimes and
        // MCP leases before removing package files; session data remains in shared storage.
        for agent in self.initialized_internal_agents().into_iter().chain(
            [self.0.external_agent.get(), self.0.continuation_agent.get()]
                .into_iter()
                .flatten(),
        ) {
            let report = agent.close().await;
            if !report.is_success() {
                return Err(format!("Agent cleanup failed: {report:?}"));
            }
        }
        self.0.agent.take();
        self.0.external_agent.take();
        self.0.continuation_agent.take();
        self.0.internal_agents.write().unwrap().clear();
        if let Some(framework) = self.0.framework.get() {
            let report = framework.close().await;
            if !report.is_success() {
                return Err(format!("capability runtime cleanup failed: {report:?}"));
            }
            self.0.framework.take();
        }
        Ok(())
    }
    pub async fn import_pack(
        &self,
        bytes: Vec<u8>,
        locale: &str,
    ) -> Result<crate::application::agent::CapabilityPackInfo, String> {
        self.import_pack_checked(bytes, None, locale.to_owned())
            .await
    }
    pub async fn import_center_pack(
        &self,
        bytes: Vec<u8>,
        manifest: serde_json::Value,
        expected_version: Option<String>,
        locale: &str,
    ) -> Result<crate::application::agent::CapabilityPackInfo, String> {
        self.import_pack_checked(bytes, Some((manifest, expected_version)), locale.to_owned())
            .await
    }
    async fn import_pack_checked(
        &self,
        bytes: Vec<u8>,
        expected: Option<(serde_json::Value, Option<String>)>,
        locale: String,
    ) -> Result<crate::application::agent::CapabilityPackInfo, String> {
        let _guard = self.0.config_lock.lock().await;
        self.ensure_external_idle()?;
        let package_id = {
            use std::io::Read;
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|e| e.to_string())?;
            let mut data = String::new();
            archive
                .by_name("manifest.json")
                .map_err(|e| e.to_string())?
                .take(1024 * 1024)
                .read_to_string(&mut data)
                .map_err(|e| e.to_string())?;
            let manifest: serde_json::Value =
                serde_json::from_str(&data).map_err(|e| e.to_string())?;
            if let Some((reviewed, installed_version)) = &expected {
                self.validate_center_import(&manifest, reviewed, installed_version.as_deref())?;
            }
            manifest
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("missing capability package id")?
                .to_owned()
        };
        if Self::builtin_pack(&package_id) {
            return Err("bundled capability package ids cannot be imported".into());
        }
        let bytes = self.validate_pack_import(bytes).await?;
        self.check_pack_references(&package_id).await?;
        self.invalidate_environment().await?;
        let manager =
            eliteforge_agent_capability_runtime::CapabilityPackManager::new(&self.0.pack_root)
                .map_err(|e| e.to_string())?;
        for version in manager
            .installed()
            .map_err(|e| e.to_string())?
            .iter()
            .filter(|v| v.manifest.id == package_id)
        {
            manager
                .check_remove(&package_id, &version.manifest.version)
                .map_err(|e| e.to_string())?;
        }
        self.invalidate_tool_metadata();
        let root = self.0.pack_root.clone();
        let pack = tokio::task::spawn_blocking(move || {
            crate::application::agent::install_capability_pack(&root, &bytes, &locale)
        })
        .await
        .map_err(|e| e.to_string())??;
        {
            let mut packs = self.0.capability_state.write().unwrap();
            packs.insert(pack.id.clone(), false);
            persist_capability_state(
                &self.0.state_path,
                &packs,
                &self.0.extension_state.read().unwrap(),
            )?;
        }
        if let Some(navigation) = self.0.navigation.get() {
            navigation.set_pack_enabled(&pack.id, false)?;
        }
        if let Some(resources) = self.0.capability_resources.get() {
            resources.transition(&pack.id, false, &[])?;
        }
        if let Some(resources) = self.0.capability_resources.get() {
            resources.remove(&pack.id)?;
        }
        let report = self.reconcile_capability_resources()?;
        self.remove_retired_skill_indexes(&report.removed_skills)
            .await;
        self.sync_pack_indexes(&pack.id, false).await?;
        self.sync_navigation_packs()?;
        self.invalidate_environment().await?;
        self.framework().await.map_err(|e| e.to_string())?;
        Ok(pack)
    }
    async fn remove_retired_skill_indexes(&self, names: &[String]) {
        let (Some(skills), Some(rag)) = (self.0.skill_registry.get(), self.0.rag.get()) else {
            return;
        };
        let Ok(active) =
            skills.list_registry(crate::domain::skill::RegistrySkillListQuery { search: None })
        else {
            return;
        };
        let active = active
            .into_iter()
            .map(|skill| skill.skill_name)
            .collect::<std::collections::BTreeSet<_>>();
        let indexer =
            crate::application::skill::SkillRagIndexService::new(skills.clone(), rag.clone());
        for name in names.iter().filter(|name| !active.contains(*name)) {
            if let Err(error) = indexer.remove_skill_index(name).await {
                tracing::warn!(skill = %name, %error, "failed to remove retired skill index");
            }
        }
    }
    async fn sync_pack_indexes(&self, id: &str, enabled: bool) -> Result<(), String> {
        if let (Some(skills), Some(rag)) = (self.0.skill_registry.get(), self.0.rag.get()) {
            let indexer =
                crate::application::skill::SkillRagIndexService::new(skills.clone(), rag.clone());
            for skill in skills
                .list_registry(crate::domain::skill::RegistrySkillListQuery { search: None })
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter(|s| s.provider_pack_id.as_deref() == Some(id))
            {
                let state = if enabled {
                    indexer.index_registry_skill(&skill.skill_name).await
                } else {
                    indexer.disable_skill(&skill.skill_name).await
                }
                .map_err(|e| e.to_string())?;
                if state.vector_status == crate::domain::skill::SkillRagIndexStatus::Failed
                    || state.graph_status == crate::domain::skill::SkillRagIndexStatus::Failed
                {
                    return Err(state
                        .last_error
                        .unwrap_or_else(|| "skill index update failed".into()));
                }
            }
        }
        Ok(())
    }
    async fn check_pack_references(&self, id: &str) -> Result<(), String> {
        self.ensure_external_idle()?;
        // Interrupted runs never release their pins. Reconcile them first so a
        // dead pin cannot block every later removal of the package.
        self.prune_durable_pins(Some(id)).await?;
        let manager =
            eliteforge_agent_capability_runtime::CapabilityPackManager::new(&self.0.pack_root)
                .map_err(|e| e.to_string())?;
        if manager
            .durable_pins()
            .map_err(|e| e.to_string())?
            .iter()
            .any(|p| p.versions.iter().any(|v| v.package == id))
        {
            return Err("capability pack is referenced by a durable run".into());
        }
        Ok(())
    }
    pub async fn set_pack_enabled(&self, id: &str, enabled: bool) -> Result<(), String> {
        let _guard = self.0.config_lock.lock().await;
        // Enabling or disabling only changes the Agent selection; package files
        // and durable pins stay untouched, so a pending run can still resume.
        self.ensure_external_idle()?;
        let mut packs = self.0.capability_state.read().unwrap().clone();
        let extensions = self.0.extension_state.read().unwrap().clone();
        let was_changed = packs.get(id).copied().unwrap_or(Self::builtin_pack(id)) != enabled;
        packs.insert(id.into(), enabled);
        if let Some(f) = self.0.framework.get() {
            self.main_selection(&f, &packs, &extensions)?;
        }
        let agents = if let Some(m) = self.0.agent_management.get() {
            m.persist_internal_agent_defaults()
                .map_err(|e| format!("{e:?}"))?;
            m.list_platform_internal_agents()
                .map_err(|e| format!("{e:?}"))?
                .into_iter()
                .map(|a| a.agent_code)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let result = async {
            if let Some(navigation) = self.0.navigation.get() {
                navigation.set_pack_enabled(id, enabled)?;
            }
            if let Some(resources) = self.0.capability_resources.get() {
                resources.transition(id, enabled, &agents)?;
            }
            if !enabled || was_changed {
                self.sync_pack_indexes(id, enabled).await?;
            }
            // Both durable configuration and native Agent selections must succeed.
            persist_capability_state(&self.0.state_path, &packs, &extensions)?;
            for agent in self.initialized_internal_agents() {
                let selection = self.main_selection(&agent.framework(), &packs, &extensions)?;
                agent
                    .reconfigure(agent.framework().agent(selection))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Ok::<(), String>(())
        }
        .await;
        if let Err(error) = result {
            packs.insert(id.into(), false);
            if let Some(navigation) = self.0.navigation.get() {
                let _ = navigation.set_pack_enabled(id, false);
            }
            if let Some(resources) = self.0.capability_resources.get() {
                let _ = resources.transition(id, false, &agents);
            }
            let _ = persist_capability_state(&self.0.state_path, &packs, &extensions);
            *self.0.capability_state.write().unwrap() = packs;
            return Err(error);
        }
        *self.0.capability_state.write().unwrap() = packs;
        Ok(())
    }
    /// Remove every installed version of `id`. Without `force`, a resumable run
    /// still pinned to the package blocks removal; `force` releases those pins
    /// first so the package can always be removed on explicit request.
    pub async fn uninstall_pack(&self, id: &str, force: bool) -> Result<(), String> {
        let _guard = self.0.config_lock.lock().await;
        if Self::builtin_pack(id) {
            return Err("bundled capability packs cannot be uninstalled".into());
        }
        if force {
            self.ensure_external_idle()?;
            self.release_pack_pins(id)?;
        } else {
            self.check_pack_references(id).await?;
        }
        let manager =
            eliteforge_agent_capability_runtime::CapabilityPackManager::new(&self.0.pack_root)
                .map_err(|e| e.to_string())?;
        let versions = manager
            .installed()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|p| p.manifest.id == id)
            .collect::<Vec<_>>();
        // Release this host's idle leases before checking cross-process version locks.
        // No persisted package state or assignments change when preflight rejects.
        self.invalidate_environment().await?;
        for version in &versions {
            manager
                .check_remove(id, &version.manifest.version)
                .map_err(|e| e.to_string())?;
        }
        let mut packs = self.0.capability_state.read().unwrap().clone();
        let extensions = self.0.extension_state.read().unwrap().clone();
        packs.remove(id);
        if let Some(navigation) = self.0.navigation.get() {
            if let Err(error) = navigation.set_pack_enabled(id, false) {
                tracing::warn!(%error, pack = id, "capability navigation could not be disabled");
            }
        }
        persist_capability_state(&self.0.state_path, &packs, &extensions)?;
        *self.0.capability_state.write().unwrap() = packs;
        if let Some(resources) = self.0.capability_resources.get() {
            if let Err(error) = resources.transition(id, false, &[]) {
                tracing::warn!(%error, pack = id, "capability resources could not be disabled");
            }
        }
        self.invalidate_environment().await?;
        self.invalidate_tool_metadata();
        // Package files are the deliverable: a failed auxiliary cleanup (skill
        // index, navigation, resource rows) must not keep the pack installed.
        let mut failures = Vec::new();
        for version in versions {
            if let Err(error) = manager.remove(id, &version.manifest.version) {
                failures.push(error.to_string());
            }
        }
        if let Some(resources) = self.0.capability_resources.get() {
            if let Err(error) = resources.remove(id) {
                tracing::warn!(%error, pack = id, "capability resources could not be removed");
            }
        }
        if let Some(navigation) = self.0.navigation.get() {
            if let Err(error) = navigation.remove_pack(id) {
                tracing::warn!(%error, pack = id, "capability navigation could not be removed");
            }
        }
        self.0.skills.write().unwrap().remove(id);
        // Only purge leftovers once no installed version remains; a version that
        // another runtime still holds must keep its files intact.
        if failures.is_empty() {
            self.purge_pack_artifacts(id);
        }
        match self.reconcile_capability_resources() {
            Ok(report) => {
                self.remove_retired_skill_indexes(&report.removed_skills)
                    .await;
            }
            Err(error) => {
                tracing::error!(%error, pack = id, "capability resources failed to reconcile after uninstall");
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    /// Delete files and caches the capability runtime leaves behind after a
    /// version removal, so nothing of an uninstalled pack can be resurrected.
    fn purge_pack_artifacts(&self, id: &str) {
        let root = &self.0.pack_root;
        for path in [root.join("packs").join(id), root.join("data").join(id)] {
            if path.exists() {
                if let Err(error) = fs::remove_dir_all(&path) {
                    tracing::warn!(path = %path.display(), %error, "capability pack files remain");
                }
            }
        }
        let locks = root.join("locks");
        if let Ok(entries) = fs::read_dir(&locks) {
            let prefix = format!("{id}@");
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.starts_with(&prefix) && name.ends_with(".lock") {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        // The framework keeps separate extractions of embedded package skills.
        let component = host_environment::pack_cache_component(id);
        let caches = [
            self.0
                .path
                .with_extension("skills")
                .join("capability-packs")
                .join(&component),
            self.0
                .path
                .with_extension("skills-catalog")
                .join(&component),
        ];
        for path in caches {
            if path.exists() {
                if let Err(error) = fs::remove_dir_all(&path) {
                    tracing::warn!(path = %path.display(), %error, "capability skill cache remains");
                }
            }
        }
    }

    pub fn tool_policy_smart(&self) -> bool {
        self.0
            .extension_state
            .read()
            .unwrap()
            .get(tron_harness_capability::tool_policy::SMART_MODE_KEY)
            .copied()
            .unwrap_or(true)
    }

    pub async fn set_tool_policy_smart(&self, smart: bool) -> Result<(), String> {
        let _guard = self.0.config_lock.lock().await;
        self.ensure_open()?;
        let packs = self.0.capability_state.read().unwrap().clone();
        let mut extensions = self.0.extension_state.read().unwrap().clone();
        extensions.insert(
            tron_harness_capability::tool_policy::SMART_MODE_KEY.into(),
            smart,
        );
        persist_capability_state(&self.0.state_path, &packs, &extensions)?;
        *self.0.extension_state.write().unwrap() = extensions;
        Ok(())
    }

    fn attach_tool_policy(&self, request: &mut fw::RunRequest, locale: &str) {
        use tron_harness_capability::tool_policy::{CommandRiskReviewerService, ToolPolicyConfig};
        let smart = self.tool_policy_smart();
        request.extensions.insert(ToolPolicyConfig { smart });
        request.extensions.insert(
            match crate::shared::locale::SupportedLocale::from_raw(locale) {
                crate::shared::locale::SupportedLocale::EnUs => {
                    tron_harness_capability::TronToolPolicyLocale::EnUs
                }
                crate::shared::locale::SupportedLocale::ZhCn => {
                    tron_harness_capability::TronToolPolicyLocale::ZhCn
                }
            },
        );
        if smart {
            request
                .extensions
                .insert(CommandRiskReviewerService(Arc::new(
                    super::command_risk_review::SmartCommandReviewer,
                )));
            // Each tool in a batch may require one auxiliary review. Preserve the
            // original answer/retry allowance and all run-wide SDK ceilings.
            request.limits.max_model_calls_per_step = request
                .limits
                .max_model_calls_per_step
                .map(|limit| limit.saturating_add(request.limits.max_tool_calls));
        }
    }

    pub async fn set_extension_enabled(&self, point: &str, enabled: bool) -> Result<(), String> {
        let _guard = self.0.config_lock.lock().await;
        self.ensure_external_idle()?;
        if matches!(point, "storage" | "model") && !enabled {
            return Err(format!("{point} is a required Framework service"));
        }
        let packs = self.0.capability_state.read().unwrap().clone();
        let mut extensions = self.0.extension_state.read().unwrap().clone();
        let previous = extensions.clone();
        extensions.insert(point.to_owned(), enabled);
        persist_capability_state(&self.0.state_path, &packs, &extensions)?;
        for agent in self.initialized_internal_agents() {
            let selection = match self.main_selection(&agent.framework(), &packs, &extensions) {
                Ok(selection) => selection,
                Err(error) => {
                    let _ = persist_capability_state(&self.0.state_path, &packs, &previous);
                    return Err(error);
                }
            };
            let target = agent.framework().agent(selection);
            if let Err(error) = agent.reconfigure(target).await {
                let _ = persist_capability_state(&self.0.state_path, &packs, &previous);
                return Err(format!("{error:?}"));
            }
        }
        self.0.tools_enabled.store(
            extensions.get("tool").copied().unwrap_or(true),
            std::sync::atomic::Ordering::Release,
        );
        self.0.observe_enabled.store(
            extensions.get("observe").copied().unwrap_or(true),
            std::sync::atomic::Ordering::Release,
        );
        *self.0.extension_state.write().unwrap() = extensions;
        Ok(())
    }

    fn ensure_external_idle(&self) -> Result<(), String> {
        self.ensure_open()?;
        for agent in self.initialized_internal_agents() {
            if !agent
                .reconfiguration_blockers()
                .map_err(|error| error.to_string())?
                .is_empty()
            {
                return Err("internal agent is busy".into());
            }
        }
        if let Some(agent) = self.0.continuation_agent.get() {
            if !agent
                .reconfiguration_blockers()
                .map_err(|error| error.to_string())?
                .is_empty()
            {
                return Err("native continuation is busy".into());
            }
        }
        if let Some(agent) = self.0.external_agent.get()
            && !agent
                .reconfiguration_blockers()
                .map_err(|e| e.to_string())?
                .is_empty()
        {
            return Err("external runtime is busy".into());
        }
        Ok(())
    }
    fn prepare_artifacts(&self) -> Result<(), String> {
        self.prepare_artifacts_at(&self.0.path.with_extension("artifacts"))
    }
    fn prepare_artifacts_at(&self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn ensure_open(&self) -> Result<(), String> {
        if self.0.closed.load(std::sync::atomic::Ordering::Acquire) {
            return Err("framework host is closed".into());
        }
        Ok(())
    }

    pub fn persistent(
        path: impl Into<PathBuf>,
        resources: Option<crate::application::resource::ResourceService>,
    ) -> Self {
        let path = path.into();
        let state_path = path.with_extension("capabilities.json");
        let pack_root = path.with_extension("capability-packs");
        let (packs, extensions) = load_capability_state(&state_path);
        Self(Arc::new(Host {
            personai: Default::default(),
            runtime_config: Default::default(),
            model_client: Default::default(),
            path,
            state_path,
            pack_root,
            capability_state: std::sync::RwLock::new(packs),
            extension_state: std::sync::RwLock::new(extensions),
            config_lock: tokio::sync::Mutex::new(()),
            framework: RuntimeCell::new(),
            storage: OnceCell::new(),
            storage_framework: OnceCell::new(),
            storage_access: Default::default(),
            runtime_access: Default::default(),
            telemetry: OnceCell::new(),
            agent: RuntimeCell::new(),
            internal_agents: Default::default(),
            mcp: OnceCell::new(),
            rag: OnceCell::new(),
            skill_registry: OnceCell::new(),
            prompt_service: OnceCell::new(),
            capability_resources: OnceCell::new(),
            capability_queue: OnceCell::new(),
            capability_queue_lock: tokio::sync::Mutex::new(()),
            capability_queue_running: std::sync::Mutex::new(None),
            navigation: OnceCell::new(),
            skills: Default::default(),
            installed_packs: Default::default(),
            tool_catalog_issues: Default::default(),
            tool_catalog_metadata: Default::default(),
            tools_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            observe_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            external_agent: RuntimeCell::new(),
            continuation_agent: RuntimeCell::new(),
            database: OnceCell::new(),
            native_bindings: Default::default(),
            sudo_inputs: Default::default(),
            closed: Default::default(),
            control: Default::default(),
            shutdown: Default::default(),
            presentation: Default::default(),
            pty: Default::default(),
            resources,
            assistants: OnceCell::new(),
            memories: OnceCell::new(),
            knowledge_bases: OnceCell::new(),
            topics: OnceCell::new(),
            integrations: OnceCell::new(),
            agent_management: OnceCell::new(),
            collaboration_builder: OnceCell::new(),
            collaboration_threads: Default::default(),
            collaboration_children: Default::default(),
            thread_feedback: OnceCell::new(),
            _temporary: None,
        }))
    }
    pub fn temporary(
        resources: Option<crate::application::resource::ResourceService>,
    ) -> Result<Self, AgentError> {
        let directory = tempfile::tempdir()
            .map_err(|error| AgentError::InvalidConfiguration(error.to_string()))?;
        let path = directory.path().join("framework.sqlite3");
        Ok(Self(Arc::new(Host {
            personai: Default::default(),
            runtime_config: Default::default(),
            model_client: Default::default(),
            state_path: path.with_extension("capabilities.json"),
            pack_root: path.with_extension("capability-packs"),
            path,
            capability_state: std::sync::RwLock::new(BTreeMap::new()),
            extension_state: std::sync::RwLock::new(BTreeMap::new()),
            config_lock: tokio::sync::Mutex::new(()),
            framework: RuntimeCell::new(),
            storage: OnceCell::new(),
            storage_framework: OnceCell::new(),
            storage_access: Default::default(),
            runtime_access: Default::default(),
            telemetry: OnceCell::new(),
            agent: RuntimeCell::new(),
            internal_agents: Default::default(),
            mcp: OnceCell::new(),
            rag: OnceCell::new(),
            skill_registry: OnceCell::new(),
            prompt_service: OnceCell::new(),
            capability_resources: OnceCell::new(),
            capability_queue: OnceCell::new(),
            capability_queue_lock: tokio::sync::Mutex::new(()),
            capability_queue_running: std::sync::Mutex::new(None),
            navigation: OnceCell::new(),
            skills: Default::default(),
            installed_packs: Default::default(),
            tool_catalog_issues: Default::default(),
            tool_catalog_metadata: Default::default(),
            tools_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            observe_enabled: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            external_agent: RuntimeCell::new(),
            continuation_agent: RuntimeCell::new(),
            database: OnceCell::new(),
            native_bindings: Default::default(),
            sudo_inputs: Default::default(),
            closed: Default::default(),
            control: Default::default(),
            shutdown: Default::default(),
            presentation: Default::default(),
            pty: Default::default(),
            resources,
            assistants: OnceCell::new(),
            memories: OnceCell::new(),
            knowledge_bases: OnceCell::new(),
            topics: OnceCell::new(),
            integrations: OnceCell::new(),
            agent_management: OnceCell::new(),
            collaboration_builder: OnceCell::new(),
            collaboration_threads: Default::default(),
            collaboration_children: Default::default(),
            thread_feedback: OnceCell::new(),
            _temporary: Some(directory),
        })))
    }
    pub fn for_database(
        database: &str,
        resources: crate::application::resource::ResourceService,
    ) -> Result<Self, AgentError> {
        if database == ":memory:" {
            return Self::temporary(Some(resources));
        }
        if database.trim().is_empty() {
            return Err(AgentError::InvalidConfiguration(
                "database path is required".into(),
            ));
        }
        Ok(Self::persistent(PathBuf::from(database), Some(resources)))
    }
    async fn agent(&self) -> Result<Agent, fw::AgentError> {
        self.0
            .agent
            .get_or_try_init(|| async {
                let framework = self.framework().await?;
                let (packs, extensions) = self.capability_state();
                let selection = self
                    .main_selection(&framework, &packs, &extensions)
                    .map_err(fw::AgentError::InvalidConfiguration)?;
                framework.agent(selection).build().await
            })
            .await
    }

    fn initialized_internal_agents(&self) -> Vec<Agent> {
        self.0
            .agent
            .get()
            .into_iter()
            .chain(
                self.0
                    .internal_agents
                    .read()
                    .unwrap()
                    .values()
                    .filter_map(|cell| cell.get().cloned()),
            )
            .collect()
    }

    async fn internal_agent(&self, workspace: &str, code: &str) -> Result<Agent, fw::AgentError> {
        self.internal_agent_profile(workspace, code, false).await
    }

    async fn internal_agent_profile(
        &self,
        workspace: &str,
        code: &str,
        onboarding: bool,
    ) -> Result<Agent, fw::AgentError> {
        let key = (
            workspace.to_owned(),
            if onboarding {
                format!("{}::onboarding", code.to_ascii_lowercase())
            } else {
                code.to_ascii_lowercase()
            },
        );
        let cell = self
            .0
            .internal_agents
            .write()
            .unwrap()
            .entry(key)
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        cell.get_or_try_init(|| async {
            self.ensure_open()
                .map_err(fw::AgentError::InvalidConfiguration)?;
            let framework = self.framework().await?;
            let (packs, extensions) = self.capability_state();
            let mut selection = self
                .main_selection(&framework, &packs, &extensions)
                .map_err(fw::AgentError::InvalidConfiguration)?;
            if onboarding {
                let providers = selection.tool_providers.as_deref().unwrap_or_default();
                if !providers.iter().any(|p| p == "tron.harness.tools")
                    || !providers.iter().any(|p| p == "agent.a2ui")
                {
                    return Err(fw::AgentError::InvalidConfiguration(
                        "Soul onboarding requires Tron tools and A2UI".into(),
                    ));
                }
                selection.tool_providers = Some(vec![
                    "tron.harness.tools".into(),
                    "tron.onboarding-ui".into(),
                    // Onboarding keeps the stable gateway pair for its model
                    // requests; all other wrapped providers drop off.
                    "framework.tools.discovery".into(),
                ]);
                selection.skills_from_host = false;
                // A2UI negotiation uses SkillAccess even when no skill tools are leased.
                selection.skills = Some(vec![
                    eliteforge_agent_official_plugin_a2ui::A2UI_SKILL_NAME.into(),
                ]);
                selection.context_sources = Some(vec![]);
            }
            framework.agent(selection).build().await
        })
        .await
        .cloned()
    }
}
struct HostSteering {
    control: ActiveRunControl,
}
#[async_trait::async_trait]
impl fw::LiveInputSource for HostSteering {
    async fn drain(
        &self,
        run: &fw::RunContext,
        seal: bool,
    ) -> Result<Vec<fw::Content>, fw::AgentError> {
        Ok(self
            .drain_inputs(run, seal)
            .await?
            .into_iter()
            .map(|input| input.content)
            .collect())
    }
    async fn drain_inputs(
        &self,
        run: &fw::RunContext,
        seal: bool,
    ) -> Result<Vec<fw::LiveInput>, fw::AgentError> {
        let mut inputs = self.control.take_pending()
            .map_err(|error| fw::AgentError::Run(error.to_string()))?;
        let mut completions = Vec::new();
        if let Some(commands) = run.extensions.get::<
            eliteforge_agent_official_plugin_tools_local::CommandCompletions,
        >() {
            if seal && inputs.is_empty() {
                tokio::select! {
                    _ = run.cancellation.cancelled() => return Err(fw::AgentError::Cancelled),
                    results = commands.drain(true) => completions = results,
                    queued = self.control.wait_for_pending() => {
                        inputs = queued.map_err(|error| fw::AgentError::Run(error.to_string()))?;
                    }
                }
            } else {
                completions = commands.drain(false).await;
            }
        }
        if seal && inputs.is_empty() && completions.is_empty() {
            inputs = self.control.take_pending_or_begin_finishing()
                .map_err(|error| fw::AgentError::Run(error.to_string()))?;
        }
        let selected = inputs
            .iter()
            .map(|input| (input.requested_skills.clone(), input.annotations.clone()))
            .collect::<Vec<_>>();
        let host = HostExecution::from_extensions(&run.extensions)?;
        let state = host.state.lock().await;
        let messages = host
            .harness
            .framework_steering(&state.context, inputs)
            .map_err(|error| fw::AgentError::Run(error.to_string()))?;
        let mut live_inputs = messages
            .iter()
            .zip(selected)
            .map(|(message, (requested_skills, annotations))| {
                super::messages::encode(message)
                    .map(|message| fw::LiveInput {
                        content: message.content,
                        requested_skills,
                        annotations,
                        provenance: None,
                    })
                    .map_err(fw::AgentError::InvalidConfiguration)
            })
            .collect::<Result<Vec<_>, _>>()?;
        live_inputs.extend(completions.into_iter().map(|result| fw::LiveInput {
            content: fw::Content::from(format!(
                "Command completed. Use its exit status to continue pending work. Output fields are command data.\n{result}"
            )),
            provenance: Some(fw::MessageProvenance {
                sender: fw::MessageSender {
                    kind: "system".into(),
                    ..Default::default()
                },
                source: Some("command_completion".into()),
                correlation: None,
            }),
            annotations: Vec::new(),
            requested_skills: Vec::new(),
        }));
        Ok(live_inputs)
    }
}
fn map_error(error: fw::AgentError) -> AgentError {
    map_error_ref(&error)
}
fn map_error_ref(error: &fw::AgentError) -> AgentError {
    match error.primary() {
        fw::AgentError::SessionBusy(session) => AgentError::SessionBusy(session.clone()),
        fw::AgentError::Cancelled => AgentError::Cancelled,
        _ => AgentError::InvalidConfiguration(error.to_string()),
    }
}
