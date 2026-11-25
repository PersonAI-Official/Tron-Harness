use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};

mod assistant_runner;
mod automation_runner;
mod capability_queue_runner;
mod routing;
mod startup;
mod state_extractors;

use axum::{Router, routing::get};
use socketioxide::SocketIo;

use crate::{
    adapters::{
        inbound::http::{
            auth::require_profile, frontend::routes as frontend_routes,
            mcp::routes::transport_routes as mcp_transport_routes,
        },
        inbound::socket::terminal as terminal_socket,
        outbound::{
            config::YamlSystemConfigStore,
            crypto::VariableCipherProvider,
            model::{
                HttpLlmRuntimeClient, HttpModelClient, HttpModelProviderClient,
                PersonAiModelProviderClient,
            },
            persistence::{
                GrafeoStoreCore, SqliteAgentEventReplayStore, SqliteAgentManagementRepository,
                SqliteAgentRunRepository, SqliteAppRepository, SqliteAutomationRepository,
                SqliteChatRepository, SqliteConnectionRepository, SqliteDirectoryRepository,
                SqliteMcpRepository, SqliteMemoryWorkDomainRepository, SqliteModelRepository,
                SqliteNavigationRepository, SqliteProjectRepository, SqlitePromptRepository,
                SqliteRagRepository, SqliteResourceRepository, SqliteSkillRegistryRepository,
                SqliteTaskRepository, SqliteTopicRepository, SqliteVariableRepository,
                SqliteWorkspaceRepository,
            },
            resource_storage::LocalObjectStorage,
            s3_storage::S3ObjectStorage,
        },
    },
    application::agent::{
        ActiveRunRegistry, AgentAguiReplayStore, AgentAguiStreamService, AgentInvocationService,
        AgentManagementService, AgentRunHistoryService, ModelClient, register_default_agent_tools,
    },
    application::app::AppService,
    application::automation::AutomationService,
    application::chat::ChatService,
    application::connector::IntegrationService,
    application::directory::DirectoryService,
    application::graph::{GraphExplorerService, GraphService, OntologyService},
    application::llm::LlmRuntimeService,
    application::mcp::McpService,
    application::memory::{MemoryService, MemoryWorkDomainService},
    application::model::ModelService,
    application::model_provider::{ModelProviderService, ProviderAwareLlmRuntimeResolver},
    application::model_router::LlmModelRouter,
    application::navigation::NavigationService,
    application::profile::ProfileRegistryService,
    application::project::ProjectService,
    application::prompt::PromptService,
    application::rag::{DefaultRagRuntime, RagService},
    application::resource::{ObjectStorage, ResourceService},
    application::short_term_memory::ShortTermMemoryService,
    application::skill::SkillRegistryService,
    application::system_config::SystemConfigService,
    application::task::TaskService,
    application::terminal::TerminalService,
    application::topic::TopicService,
    application::variable::VariableService,
    application::workspace::WorkspaceService,
    config::{AppConfig, ResourceStorageProfile},
    domain::agent::ToolRegistry,
    health::{health, readiness},
    openapi::routes as openapi_routes,
    shared::{
        event_bus::{AguiEventSink, EventBusHandle, EventBusRuntimeBuilder},
        http_client::HttpClientPool,
        id::{IdGenerator, SnowflakeIdProvider, SnowflakedSnowflakeIdProvider},
    },
};

pub use self::startup::{AppBootstrapError, AppReadiness, AppReadinessStatus};
use self::{
    automation_runner::MainAgentAutomationRunner,
    routing::{build_profile_routes, build_protected_routes},
    startup::{build_graph_core, build_graph_services, build_rag_service},
};

static GLOBAL_APP_STATE: OnceLock<AppState> = OnceLock::new();
static GLOBAL_GRAPH_CORE: OnceLock<GrafeoStoreCore> = OnceLock::new();
static GLOBAL_GRAPH_SERVICES: OnceLock<(GraphService, OntologyService, GraphExplorerService)> =
    OnceLock::new();

pub type AppRagService = RagService<Arc<Mutex<SqliteRagRepository>>, DefaultRagRuntime>;

pub type AppMemoryService = MemoryService<Arc<Mutex<SqliteRagRepository>>, DefaultRagRuntime>;

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

/// Services shared by routes, middleware and background workers. Cloning the
/// HTTP state must not clone every service's configuration and lookup tables.
#[derive(Clone)]
pub struct AppStateInner {
    pub assistant_memory: crate::application::assistant_memory::AssistantMemoryService,
    pub assistants: crate::application::assistant::AssistantService,
    pub config: AppConfig,
    #[doc(hidden)]
    pub remote_access: Arc<std::sync::RwLock<crate::config::RemoteAccessConfig>>,
    pub remote_listener_active: Arc<std::sync::atomic::AtomicBool>,
    /// Port the dedicated remote listener bound; `0` while it is not running.
    pub remote_listener_port: Arc<std::sync::atomic::AtomicU16>,
    pub disk_usage: crate::adapters::outbound::observability::disk_usage::LocalDiskUsage,
    _temporary_database: Option<Arc<tempfile::TempDir>>,
    _agent_collaboration_builder: Arc<crate::application::agent::AgentRunContextBuilder>,
    pub readiness: AppReadiness,
    pub agui_events: Arc<AguiEventSink>,
    pub agui_replay_store: Arc<dyn AgentAguiReplayStore>,
    pub agent_presentation: AgentAguiStreamService,
    #[doc(hidden)]
    pub remote_service:
        std::sync::OnceLock<Arc<eliteforge_agent_remote_server::RemoteAccessService>>,
    pub agent_model_client: Arc<dyn ModelClient>,
    pub runtime_http_client: reqwest::Client,
    pub agent_management: AgentManagementService,
    pub agent_runs: AgentRunHistoryService,
    pub agent_tool_registry: Arc<ToolRegistry>,
    pub agent_framework: crate::application::agent::FrameworkHost,
    pub agent_terminal: Arc<dyn crate::application::agent::AgentTerminal>,
    pub active_runs: ActiveRunRegistry,
    pub apps: AppService,
    pub projects: ProjectService,
    pub directories: DirectoryService,
    pub integrations: IntegrationService,
    pub topics: TopicService,
    pub automations: AutomationService,
    pub chats: ChatService,
    pub short_term_memory: ShortTermMemoryService,
    pub event_bus: EventBusHandle,
    pub graphs: GraphService,
    pub ontologies: OntologyService,
    pub graph_explorer: GraphExplorerService,
    pub tasks: TaskService,
    pub terminal: TerminalService,
    pub variables: VariableService,
    pub resources: ResourceService,
    pub workspaces: WorkspaceService,
    pub skills: SkillRegistryService,
    pub system_config: SystemConfigService,
    pub skins: crate::application::skin::SkinService,
    pub models: ModelService,
    pub model_providers: ModelProviderService,
    pub prompts: PromptService,
    pub profiles: ProfileRegistryService,
    pub account_sync: crate::application::profile::ProfileAccountSync,
    pub llm_runtime: LlmRuntimeService,
    pub llm_router: LlmModelRouter,
    pub mcp: McpService,
    pub memory: AppMemoryService,
    pub memory_work_domains: MemoryWorkDomainService,
    pub navigation: NavigationService,
    pub rag: AppRagService,
    pub knowledge_bases: crate::application::knowledge_base::KnowledgeBaseService,
}

impl std::ops::Deref for AppState {
    type Target = AppStateInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for AppState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Host setup may replace a service before building its router. Preserve
        // value isolation for that explicit mutation; requests only share reads.
        Arc::make_mut(&mut self.0)
    }
}

impl AppState {
    /// Late-bound handle used by the remote-access capability pack. The pack is
    /// built while the framework host is assembled, before `AppState` exists, so
    /// its host adapter resolves the running state lazily.
    pub(crate) fn install_global(state: &AppState) {
        let _ = GLOBAL_APP_STATE.set(state.clone());
    }

    pub(crate) fn global() -> Option<&'static AppState> {
        GLOBAL_APP_STATE.get()
    }

    /// Whether the dedicated remote listener finished binding at startup.
    pub fn remote_access_active(&self) -> bool {
        self.0
            .remote_listener_active
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_remote_access_active(&self, active: bool) {
        self.0
            .remote_listener_active
            .store(active, std::sync::atomic::Ordering::Relaxed);
    }

    /// Port the dedicated remote listener is serving on, when it is running.
    pub fn remote_listen_port(&self) -> Option<u16> {
        match self
            .0
            .remote_listener_port
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            0 => None,
            port => Some(port),
        }
    }

    pub fn set_remote_listen_port(&self, port: Option<u16>) {
        self.0
            .remote_listener_port
            .store(port.unwrap_or(0), std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn assistant_invocation_service(&self) -> AgentInvocationService {
        AgentInvocationService::new(
            self.workspaces.clone(),
            self.apps.clone(),
            self.variables.clone(),
            self.skills.clone(),
            self.agent_management.clone(),
            self.agent_runs.clone(),
            self.mcp.clone(),
            self.event_bus.clone(),
            self.agent_model_client.clone(),
            self.agent_tool_registry.clone(),
            self.runtime_http_client.clone(),
        )
        .with_framework_host(self.agent_framework.clone())
        .with_active_runs(self.active_runs.clone())
        .with_short_term_memory(self.short_term_memory.clone())
        .with_prompt_service(self.prompts.clone())
        .with_platform_scope()
        .with_assistant_work_context(self.assistants.clone())
    }
    pub fn new(config: AppConfig) -> Result<Self, AppBootstrapError> {
        Self::new_with_id_provider(config, Arc::new(SnowflakedSnowflakeIdProvider::default()))
    }

    pub fn new_with_id_provider<I>(
        config: AppConfig,
        id_provider: Arc<I>,
    ) -> Result<Self, AppBootstrapError>
    where
        I: SnowflakeIdProvider + IdGenerator + 'static,
    {
        let profiles = if config.database_url == ":memory:" {
            ProfileRegistryService::new(":memory:")
        } else {
            ProfileRegistryService::from_environment()
        }
        .map_err(|source| AppBootstrapError::ProfileRegistryInitFailed { source })?;
        Self::new_with_profiles(config, id_provider, profiles)
    }

    /// Explicit identity registry for embedded hosts and isolated integration tests.
    pub fn new_with_profiles<I>(
        mut config: AppConfig,
        id_provider: Arc<I>,
        profiles: ProfileRegistryService,
    ) -> Result<Self, AppBootstrapError>
    where
        I: SnowflakeIdProvider + IdGenerator + 'static,
    {
        let temporary_database = if config.database_url == ":memory:" {
            let directory = Arc::new(
                tempfile::tempdir()
                    .map_err(|source| AppBootstrapError::DatabaseInitFailed { source })?,
            );
            config.database_url = directory
                .path()
                .join("tron.sqlite3")
                .to_string_lossy()
                .into_owned();
            Some(directory)
        } else {
            None
        };
        let runtime_config =
            crate::application::system_config::BackendRuntimeConfig::new(config.clone());
        let agui_events = Arc::new(AguiEventSink::new(&config.event_bus));
        let agui_replay_store = Arc::new(
            SqliteAgentEventReplayStore::new().expect("agent event replay store should initialize"),
        );
        let agent_presentation = AgentAguiStreamService::with_replay_store(
            agui_events.clone(),
            agui_replay_store.clone(),
        )
        .with_transient_protocol();
        let event_bus = EventBusRuntimeBuilder::new(config.event_bus.clone(), id_provider.clone())
            .register_sink(agui_events.clone())
            .register_sink(agui_replay_store.clone())
            .build();
        let http_client_pool = HttpClientPool::from_profiles(&config.http_clients)
            .expect("http client pool should initialize");
        let model_inference_client = http_client_pool.model_inference_client();
        let runtime_http_client = http_client_pool.runtime_control_client();
        let workspaces = build_workspace_service(&config.database_url, event_bus.clone());
        let navigation =
            build_navigation_service(&config.database_url, config.navigation.max_depth)
                .with_runtime_config(runtime_config.clone());
        let apps = build_app_service(&config.database_url, workspaces.clone());
        let projects =
            build_project_service(&config.database_url, workspaces.clone(), apps.clone());
        let directories = DirectoryService::new(
            SqliteDirectoryRepository::connect(&config.database_url)
                .expect("directory SQLite repository should initialize"),
            workspaces.clone(),
            crate::adapters::outbound::native_directory_picker::NativeDirectoryPicker::default(),
        );
        let tasks = build_task_service(
            &config.database_url,
            workspaces.clone(),
            apps.clone(),
            event_bus.clone(),
        );
        let terminal = TerminalService::new(
            apps.clone(),
            workspaces.clone(),
            config.terminal_working_directory.clone(),
        )
        .with_runtime_config(runtime_config.clone());
        let variables = build_variable_service(&config.database_url, workspaces.clone(), &config);
        let integrations = IntegrationService::new(
            SqliteConnectionRepository::connect(&config.database_url)
                .expect("connector SQLite repository should initialize"),
            variables.cipher(),
            workspaces.clone(),
            vec![
                Arc::new(eliteforge_agent_official_plugin_connector_github::GitHubProvider),
                Arc::new(eliteforge_agent_official_plugin_connector_gitlab::GitLabProvider),
                Arc::new(eliteforge_agent_official_plugin_connector_gitee::GiteeProvider),
            ],
        )
        .expect("connector providers should initialize");
        let resources = build_resource_service(
            &config.database_url,
            workspaces.clone(),
            &config,
            event_bus.clone(),
        );
        let knowledge_base_repository =
            crate::adapters::outbound::persistence::sqlite::SqliteKnowledgeBaseRepository::connect(
                &config.database_url,
            )
            .map_err(|_| AppBootstrapError::RagInitFailed {
                database_url: config.database_url.clone(),
                source: crate::domain::rag::RagError::StorageFailure,
            })?;
        let resources = resources
            .with_runtime_config(runtime_config.clone())
            .with_privacy_policy(Arc::new(knowledge_base_repository.clone()));
        let topics = TopicService::new(
            SqliteTopicRepository::connect(&config.database_url)
                .expect("topic SQLite repository should initialize"),
            projects.clone(),
            directories.clone(),
            integrations.clone(),
            resources.clone(),
            tasks.clone(),
        );
        let mcp = build_mcp_service(
            &config.database_url,
            workspaces.clone(),
            variables.clone(),
            runtime_http_client.clone(),
        );
        let skills =
            build_skill_registry_service(&config.database_url, &config.skills_registry_root);
        let agent_management = build_agent_management_service(
            &config.database_url,
            workspaces.clone(),
            &config.system_agents,
        );
        agent_management
            .initialize_default_main_agent()
            .expect("default main Agent");
        if let Err(error) = agent_management.bootstrap_registry() {
            tracing::error!("agent management bootstrap scan failed: {error:?}");
        }
        let automations =
            build_automation_service(&config.database_url, workspaces.clone(), event_bus.clone());
        let model_providers = build_model_provider_service(&config.database_url, variables.clone());
        let models = build_model_service(
            &config.database_url,
            variables.clone(),
            model_providers.clone(),
        );
        let agent_model_client = Arc::new(
            HttpModelClient::new(Arc::new(PersonAiModelProviderClient::new(
                HttpModelProviderClient::new(model_inference_client.clone()),
                model_providers.clone(),
            )))
            .with_model_executor(model_providers.model_executor())
            .with_runtime_config(runtime_config.clone()),
        );
        let prompts =
            build_prompt_service(&config.database_url, variables.clone(), id_provider.clone());
        let llm_runtime = LlmRuntimeService::new(
            Arc::new(
                ProviderAwareLlmRuntimeResolver::new(model_providers.clone())
                    .with_configured_providers(models.clone()),
            ),
            Arc::new(
                HttpLlmRuntimeClient::with_model_client(agent_model_client.clone())
                    .with_prompt_service(prompts.clone()),
            ),
        );
        let prompts = prompts.with_llm_runtime(llm_runtime.clone());
        let llm_router =
            LlmModelRouter::new(model_providers.clone()).with_configured_providers(models.clone());
        let prompts = prompts.with_llm_router(llm_router.clone());
        let graph_core = if config.graph_storage_path == ":memory:" {
            if let Some(core) = GLOBAL_GRAPH_CORE.get() {
                core.clone()
            } else {
                let core = build_graph_core(&config.graph_storage_path)?;
                let _ = GLOBAL_GRAPH_CORE.set(core.clone());
                core
            }
        } else {
            build_graph_core(&config.graph_storage_path)?
        };
        let rag = build_rag_service(
            &config.database_url,
            llm_runtime.clone(),
            llm_router.clone(),
            graph_core.clone(),
        )?
        .with_runtime_config(runtime_config.clone());
        crate::application::rag::RagConfigRepository::recover_record_tasks(
            &rag.repository_handle(),
        )
        .map_err(|source| AppBootstrapError::RagInitFailed {
            database_url: config.database_url.clone(),
            source,
        })?;
        if tokio::runtime::Handle::try_current().is_ok() {
            rag.start_graph_maintenance()
                .map_err(|_| AppBootstrapError::RagInitFailed {
                    database_url: config.database_url.clone(),
                    source: crate::domain::rag::RagError::StorageFailure,
                })?;
        }
        let memory_repository = rag
            .repository_handle()
            .lock()
            .expect("RAG repository")
            .memory_repository();
        let rag = rag.with_memory_repository(memory_repository);
        let knowledge_bases = crate::application::knowledge_base::KnowledgeBaseService::new(
            knowledge_base_repository,
            rag.clone(),
            resources.clone(),
            agent_management.clone(),
            llm_runtime.clone(),
        )
        .with_document_parser(Arc::new(
            crate::adapters::outbound::person_ai_documents::PersonAiDocumentParser::new(
                model_providers.clone(),
            ),
        ))
        .with_image_descriptions(config.mineru.image_description.clone())
        .with_visual_context(config.mineru.visual_context_enabled)
        .with_office_renderer(Arc::new(
            crate::adapters::outbound::office_visuals::LocalOfficeChartRenderer,
        ))
        .with_ingestion_config(config.mineru.ingestion.clone())
        .with_runtime_config(runtime_config.clone());
        if tokio::runtime::Handle::try_current().is_ok() {
            knowledge_bases.start_ingestion_workers().map_err(|_| {
                AppBootstrapError::RagInitFailed {
                    database_url: config.database_url.clone(),
                    source: crate::domain::rag::RagError::StorageFailure,
                }
            })?;
        }
        let memory = MemoryService::new(rag.clone());
        let memory_work_domains = build_memory_work_domain_service(&config.database_url);
        let agent_framework = crate::application::agent::FrameworkHost::for_database(
            &config.database_url,
            resources.clone(),
        )
        .expect("initialize Framework storage configuration");
        agent_framework.bind_runtime_config(runtime_config.clone());
        agent_framework.bind_memories(memory.clone());
        agent_framework
            .bind_navigation(navigation.clone())
            .expect("capability navigation catalog");
        agent_framework.bind_prompt_service(prompts.clone());
        agent_framework
            .bind_personai(model_providers.clone())
            .expect("initial PersonAI tool credentials");
        skills.bind_agents(agent_management.clone());
        knowledge_bases.bind_skills(skills.clone());
        agent_framework.bind_agent_management(agent_management.clone());
        let capability_resources=crate::application::capability::CapabilityResources::new(crate::adapters::outbound::persistence::sqlite::capability_repository::SqliteCapabilityRepository::connect(&config.database_url).expect("capability resource storage"));
        let capability_queue=crate::application::capability::CapabilityQueueService::new(crate::adapters::outbound::persistence::sqlite::capability_queue_repository::SqliteCapabilityQueueRepository::connect(&config.database_url).expect("capability queue storage"));
        agent_management.bind_capabilities(capability_resources.clone());
        mcp.bind_capabilities(capability_resources.clone());
        agent_management.bind_mcp(mcp.clone());
        skills.bind_capabilities(capability_resources.clone());
        if let Err(error) = agent_framework.bind_skills(
            skills.clone(),
            capability_resources,
            mcp.clone(),
            rag.clone(),
        ) {
            // A capability cache failure must not prevent the profile from
            // opening; the next startup or pack change retries reconciliation.
            tracing::error!(%error, "capability resource reconciliation failed");
        }
        agent_framework
            .bind_capability_queue(capability_queue)
            .expect("capability queue binding");
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let indexer =
                crate::application::skill::SkillRagIndexService::new(skills.clone(), rag.clone());
            runtime.spawn(async move {
                if let Err(error) = indexer.synchronize_all().await {
                    tracing::error!("platform skill index synchronization failed: {error}");
                }
            });
            // 中断的运行不会提交终态检查点，因而不会释放能力包版本 pin；
            // 启动后按运行状态对账，避免历史 pin 永久阻止卸载。
            let pinned = agent_framework.clone();
            runtime.spawn(async move {
                match pinned.prune_durable_pins(None).await {
                    Ok(0) => {}
                    Ok(removed) => tracing::info!(removed, "released stale capability pack pins"),
                    Err(error) => {
                        tracing::warn!(%error, "capability pack pin reconciliation failed")
                    }
                }
            });
        }
        agui_replay_store
            .bind_framework(&agent_framework)
            .expect("Framework replay storage");
        agent_framework
            .bind_knowledge_bases(knowledge_bases.clone())
            .expect("initial knowledge-base binding");
        agent_framework
            .bind_topics(topics.clone())
            .expect("initial topic binding");
        agent_framework
            .bind_integrations(integrations.clone())
            .expect("initial connector binding");
        agent_framework
            .bind_presentation(agent_presentation.clone())
            .expect("initial presentation binding");
        agent_framework
            .bind_model_client(agent_model_client.clone())
            .expect("initial model client binding");
        let short_term_memory = ShortTermMemoryService::with_host(
            agent_framework.clone(),
            config.short_term_memory.clone(),
        )
        .with_runtime_config(runtime_config.clone());
        let agent_tool_registry = Arc::new(ToolRegistry::default());
        // The application owns this bridge. The host keeps a weak reference because
        // registered product tools may themselves hold the same FrameworkHost.
        let agent_collaboration_builder = Arc::new(
            crate::application::agent::AgentRunContextBuilder::new(
                workspaces.clone(),
                apps.clone(),
                variables.clone(),
                skills.clone(),
                agent_management.clone(),
                mcp.clone(),
                event_bus.clone(),
                agent_tool_registry.clone(),
                Some(short_term_memory.clone()),
                Some(prompts.clone()),
                None,
                Some(Arc::new(
                    crate::adapters::outbound::application_directory::GitApplicationDirectoryInspector,
                )),
            ).with_framework_host(Some(agent_framework.clone())).with_model_router(crate::application::model_router::AgentModelRouter::new(models.clone(), model_providers.clone()).with_agent_management(agent_management.clone())),
        );
        agent_framework
            .bind_collaboration(&agent_collaboration_builder)
            .expect("initial Agent collaboration binding");
        let agent_terminal: Arc<dyn crate::application::agent::AgentTerminal> = Arc::new(
            crate::adapters::outbound::agent_terminal::LocalAgentTerminal::new(
                agent_framework.pty(),
            ),
        );
        register_default_agent_tools(
            agent_tool_registry.as_ref(),
            workspaces.clone(),
            apps.clone(),
            automations.clone(),
            tasks.clone(),
            skills.clone(),
            variables.clone(),
            resources.clone(),
            prompts.clone(),
            mcp.clone(),
            memory.clone(),
            short_term_memory.clone(),
        )
        .expect("default agent tools should register");
        agent_tool_registry
            .register(crate::application::agent::rag_search_tool(
                rag.clone(),
                knowledge_bases.clone(),
            ))
            .expect("RAG search tool should register");
        for tool in crate::application::agent::rag_graph_tools(rag.clone(), knowledge_bases.clone())
        {
            agent_tool_registry
                .register(tool)
                .expect("RAG graph tools should register");
        }
        let (graphs, ontologies, graph_explorer) = if config.graph_storage_path == ":memory:" {
            if let Some(services) = GLOBAL_GRAPH_SERVICES.get() {
                services.clone()
            } else {
                let services = build_graph_services(
                    graph_core.clone(),
                    workspaces.clone(),
                    llm_runtime.clone(),
                );
                let _ = GLOBAL_GRAPH_SERVICES.set(services.clone());
                services
            }
        } else {
            build_graph_services(graph_core, workspaces.clone(), llm_runtime.clone())
        };

        let agent_runs = AgentRunHistoryService::new(
            SqliteAgentRunRepository::with_host(agent_framework.clone())
                .expect("Framework run history"),
        );
        let active_runs = ActiveRunRegistry::new();
        let chats = ChatService::with_id_provider(
            SqliteChatRepository::with_host(agent_framework.clone())
                .expect("Framework chat storage"),
            id_provider,
        )
        .with_active_runs(active_runs.clone())
        .with_framework_host(agent_framework.clone())
        .with_projects(projects.clone())
        .with_directories(directories.clone())
        .with_topics(topics.clone())
        .with_execution_memory(short_term_memory.clone())
        .with_skill_registry(skills.clone())
        .with_resources(resources.clone())
        .with_prompt_service(prompts.clone())
        .with_llm_router(llm_router.clone())
        .with_event_bus(event_bus.clone());
        workspaces.bind_session_purger(Arc::new(chats.clone()));
        workspaces.bind_resource_purger(Arc::new(resources.clone()));
        automations.set_execution_runner(Arc::new(MainAgentAutomationRunner::new(
            agent_framework.clone(),
            automations.clone(),
            chats.clone(),
            agent_management.clone(),
            models.clone(),
            model_providers.clone(),
            AgentInvocationService::new(
                workspaces.clone(),
                apps.clone(),
                variables.clone(),
                skills.clone(),
                agent_management.clone(),
                agent_runs.clone(),
                mcp.clone(),
                event_bus.clone(),
                agent_model_client.clone(),
                agent_tool_registry.clone(),
                runtime_http_client.clone(),
            )
            .with_framework_host(agent_framework.clone())
            .with_directory_inspector(Some(std::sync::Arc::new(
                crate::adapters::outbound::application_directory::GitApplicationDirectoryInspector,
            )))
            .with_short_term_memory(short_term_memory.clone())
            .with_active_runs(active_runs.clone())
            .with_prompt_service(prompts.clone()),
        )));
        automations
            .bootstrap_scheduler()
            .expect("automation scheduler should initialize");
        let assistants = crate::application::assistant::AssistantService::new(
            crate::adapters::outbound::persistence::sqlite::SqliteAssistantRepository::connect(
                &config.database_url,
            )
            .expect("PersonAI storage"),
            agent_management.clone(),
        );
        assistants.bootstrap().expect("PersonAI defaults");
        agent_framework.bind_assistants(assistants.clone());
        chats.bind_assistants(assistants.clone());
        chats.bind_knowledge_quality(knowledge_bases.clone());
        knowledge_bases
            .recover_quality_optimizations()
            .map_err(|e| AppBootstrapError::DatabaseInitFailed {
                source: std::io::Error::other(e.to_string()),
            })?;
        for tool in crate::application::agent::knowledge_quality_tools(
            crate::application::knowledge_quality::KnowledgeQualityService::new(
                knowledge_bases.clone(),
            ),
            chats.clone(),
        ) {
            agent_tool_registry
                .register(tool)
                .expect("knowledge evaluation tools should register");
        }
        crate::application::assistant::register_assistant_tools(
            agent_tool_registry.as_ref(),
            assistants.clone(),
            tasks.clone(),
            agent_framework.clone(),
        )
        .expect("PersonAI tools should register");
        let assistant_memory = crate::application::assistant_memory::AssistantMemoryService::new(
            crate::adapters::outbound::persistence::sqlite::SqliteAssistantMemoryRepository::connect(
                &config.database_url,
            )
            .expect("PersonAI memory storage"),
            skills.clone(),
            std::path::Path::new(&config.skills_registry_root).join(".assistant-candidates"),
        );
        assistant_memory.set_notifications(Arc::new(assistant_runner::MemoryNotifications(
            assistants.clone(),
        )));

        let readiness = AppReadiness::validated_ready();
        let system_config = SystemConfigService::new(Arc::new(YamlSystemConfigStore::new(
            config.application_config_path.clone(),
        )))
        .with_runtime_config(runtime_config)
        .with_shortcut_defaults(Arc::new(
            crate::adapters::outbound::persistence::sqlite::SqliteShortcutDefaults::connect(
                &config.database_url,
            )
            .expect("shortcut defaults storage"),
        ));
        let skin_root = std::path::Path::new(&config.application_config_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("skins");
        let skins = crate::application::skin::SkinService::new(Arc::new(
            crate::adapters::outbound::skin::FileSkinStore::new(skin_root),
        ));
        let disk_usage =
            crate::adapters::outbound::observability::disk_usage::LocalDiskUsage::new(&config);
        let account_sync = crate::application::profile::ProfileAccountSync::new(
            profiles.clone(),
            model_providers.clone(),
            runtime_http_client.clone(),
        );
        let remote_access = Arc::new(std::sync::RwLock::new(config.remote_access.clone()));
        let state = Self(Arc::new(AppStateInner {
            assistant_memory,
            assistants,
            config,
            remote_access,
            remote_listener_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            remote_listener_port: Arc::new(std::sync::atomic::AtomicU16::new(0)),
            disk_usage,
            _temporary_database: temporary_database,
            _agent_collaboration_builder: agent_collaboration_builder,
            readiness,
            agui_events,
            agui_replay_store,
            agent_presentation,
            remote_service: std::sync::OnceLock::new(),
            agent_model_client,
            runtime_http_client,
            agent_management,
            agent_runs,
            agent_tool_registry,
            agent_framework,
            agent_terminal,
            active_runs,
            apps,
            projects,
            directories,
            integrations,
            topics,
            automations,
            chats,
            short_term_memory,
            event_bus,
            graphs,
            ontologies,
            graph_explorer,
            models,
            model_providers,
            prompts,
            profiles,
            account_sync,
            llm_runtime,
            llm_router,
            mcp,
            memory,
            memory_work_domains,
            navigation,
            rag,
            knowledge_bases,
            skills,
            system_config,
            skins,
            tasks,
            terminal,
            variables,
            resources,
            workspaces,
        }));
        Self::install_global(&state);
        Ok(state)
    }
}

pub fn build_router(config: AppConfig) -> Result<Router, AppBootstrapError> {
    build_router_with_id_provider(config, Arc::new(SnowflakedSnowflakeIdProvider::default()))
}

pub fn build_router_with_id_provider<I>(
    config: AppConfig,
    id_provider: Arc<I>,
) -> Result<Router, AppBootstrapError>
where
    I: SnowflakeIdProvider + IdGenerator + 'static,
{
    let state = AppState::new_with_id_provider(config, id_provider)?;
    build_router_with_state(state)
}

/// Assemble routes from the same state owned by the host lifecycle.
pub fn build_router_with_state(state: AppState) -> Result<Router, AppBootstrapError> {
    start_background_services(&state);
    build_router_inner(state, false)
}

/// Router for the dedicated remote listener. It exposes the mobile-facing API
/// only and requires the pairing token for every request, including requests
/// that reach it from localhost through an FRP tunnel.
pub fn build_remote_router_with_state(state: AppState) -> Result<Router, AppBootstrapError> {
    build_router_inner(state, true)
}

fn start_background_services(state: &AppState) {
    assistant_runner::start(state.clone());
    capability_queue_runner::start(state.clone());
    state.account_sync.start();
    state.model_providers.start_background_sync();
    state.model_providers.start_person_ai_heartbeat();
}

pub async fn maintain_sqlite_databases() {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        if let Err(error) = tokio::task::spawn_blocking(|| {
            crate::adapters::outbound::persistence::sqlite::framework_storage::reclaim_all_databases(1024);
        })
        .await
        {
            tracing::warn!(%error, "SQLite maintenance worker failed");
        }
    }
}

fn build_router_inner(state: AppState, remote_only: bool) -> Result<Router, AppBootstrapError> {
    let protected_routes = build_protected_routes(&state);
    if remote_only {
        return Ok(Router::new()
            .route("/api/v1/health", get(health))
            .route("/api/v1/readiness", get(readiness))
            .merge(crate::adapters::inbound::http::remote_access::identity_routes())
            .merge(protected_routes)
            .with_state(state.clone())
            .layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::adapters::inbound::http::remote_access::require_remote_token,
            ))
            .layer(axum::middleware::from_fn_with_state(
                state,
                crate::adapters::inbound::http::observability::log_application_request,
            ))
            .layer(remote_access_cors_layer()));
    }
    let profile_routes = build_profile_routes();
    let terminal_gateway = terminal_socket::TerminalSocketGateway::new(state.terminal.clone())
        .with_pty(state.agent_framework.pty());
    let (socket_layer, socket_io) = SocketIo::builder()
        .with_state(terminal_gateway)
        .max_payload(64 * 1024)
        .build_layer();
    terminal_socket::configure(&socket_io);

    Ok(Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/readiness", get(readiness))
        .merge(openapi_routes())
        .merge(crate::adapters::inbound::http::remote_access::routes())
        .merge(crate::adapters::inbound::http::remote_access::identity_routes())
        .merge(profile_routes)
        .merge(
            mcp_transport_routes().route_layer(axum::middleware::from_fn_with_state(
                state.profiles.clone(),
                require_profile,
            )),
        )
        .merge(protected_routes)
        .merge(frontend_routes())
        .with_state(state.clone())
        .layer(socket_layer)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::adapters::inbound::http::remote_access::require_remote_access,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state,
            crate::adapters::inbound::http::observability::log_application_request,
        )))
}

include!("app/builders.rs");

/// The mobile client sends bearer tokens and runs from a different origin when
/// previewed in a browser, so mirror the request origin instead of pinning a
/// host list. Credentials stay enabled for cookie-based WebView bootstrap.
fn remote_access_cors_layer() -> tower_http::cors::CorsLayer {
    use axum::http::{HeaderName, Method, header};
    tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
        .allow_methods([
            Method::GET,
            Method::HEAD,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::ACCEPT,
            header::ACCEPT_LANGUAGE,
            header::CONTENT_TYPE,
            header::RANGE,
            HeaderName::from_static("cache-control"),
            HeaderName::from_static("x-requested-with"),
            HeaderName::from_static("x-tron-access-token"),
            HeaderName::from_static("x-tron-pairing-token"),
            HeaderName::from_static("x-assistant-code"),
        ])
        .allow_credentials(true)
        .max_age(std::time::Duration::from_secs(600))
}

pub mod telemetry;
