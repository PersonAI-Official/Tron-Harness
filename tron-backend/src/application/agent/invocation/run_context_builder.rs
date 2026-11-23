use crate::application::agent::tool_framework::FrameworkToolRegistryExt;
mod tool_access;
use std::{collections::HashMap, sync::Arc};
use tool_access::PreparedToolAccess;

use crate::{
    application::{
        app::AppService, mcp::McpService, prompt::PromptService,
        short_term_memory::ShortTermMemoryService, skill::SkillRegistryService,
        variable::VariableService, workspace::WorkspaceService,
    },
    domain::{
        agent::{
            AgentExecutionScope, AgentMessage, AgentModelConfig, AgentPromptPrefixSource,
            AgentRunContext, AgentRunLimits, AgentRuntimeType, AutoModelRouteContext,
            EffectiveSkill, EffectiveTool, EffectiveVariable, ToolRegistry,
        },
        prompt::PromptRuntimeProvider,
        workspace::WorkspaceStatus,
    },
    shared::{event_bus::EventBusHandle, locale::SupportedLocale},
};

use crate::application::agent::{
    AgentInvocationCommand, AgentInvocationError, AgentManagementService, ResolvedAgentRuntime,
    RuntimeAdapterSelection,
};

use super::{
    agui_warning::McpWarningPublishContext,
    attachments::user_prompt_message,
    build_system_runtime_model_config, generate_agent_run_code,
    helpers::{
        agent_kind_for_runtime, map_agent_error, normalize_optional, required_runtime_command_name,
        runtime_type_label,
    },
    short_term_memory::activate_short_term_memory,
};

const MAIN_AGENT_CODE: &str = "main-agent";

fn managed_system_prompt(
    prompt_service: Option<&PromptService>,
    catalog_key: &str,
    locale: &str,
    runtime_values: &HashMap<PromptRuntimeProvider, serde_json::Value>,
) -> Result<String, AgentInvocationError> {
    if let Some(prompt_service) = prompt_service {
        match prompt_service.runtime_prompt_text(catalog_key, locale, runtime_values) {
            Ok(Some(prompt)) => return Ok(prompt),
            Ok(None) => {}
            Err(error) => {
                return Err(AgentInvocationError::RuntimeFailed(format!(
                    "managed system prompt could not be resolved: {error}"
                )));
            }
        }
    }
    Ok(render_builtin_system_prompt(
        catalog_key,
        locale,
        runtime_values,
    )?)
}

/// Render the bundled body when no managed prompt record is available, so the
/// fallback honours the same runtime-variable contract as the database path
/// instead of leaking `{{placeholder}}` markers into the instructions.
fn render_builtin_system_prompt(
    catalog_key: &str,
    locale: &str,
    runtime_values: &HashMap<PromptRuntimeProvider, serde_json::Value>,
) -> Result<String, AgentInvocationError> {
    let definition = crate::domain::prompt::builtin_prompt(catalog_key)
        .unwrap_or_else(|| panic!("managed system prompt is missing: {catalog_key}"));
    let body = definition
        .translations
        .get(locale)
        .or_else(|| definition.translations.get("en-US"))
        .map(|translation| translation.prompt_body.as_str())
        .unwrap_or_else(|| {
            panic!("managed system prompt has no {locale} translation: {catalog_key}")
        });
    let variables = crate::domain::prompt::builtin_prompt_variables(catalog_key)
        .unwrap_or_else(|| panic!("managed system prompt is missing: {catalog_key}"));
    // Builtin system prompts declare runtime-provided variables only; a missing
    // value is a contract violation, never a silently blank instruction.
    let mut typed_values = HashMap::with_capacity(variables.len());
    for variable in &variables {
        let provider = variable.runtime_provider.unwrap_or_else(|| {
            panic!(
                "builtin system prompt variables must be runtime-provided: {catalog_key}: {}",
                variable.name
            )
        });
        let value = runtime_values.get(&provider).cloned().ok_or_else(|| {
            AgentInvocationError::RuntimeFailed(format!(
                "runtime variable is unavailable: {catalog_key}: {}",
                variable.name
            ))
        })?;
        typed_values.insert(variable.name.clone(), value);
    }
    crate::domain::prompt::render_typed_template(body, &variables, &typed_values).map_err(|error| {
        AgentInvocationError::RuntimeFailed(format!(
            "managed system prompt could not be rendered: {catalog_key}: {error}"
        ))
    })
}

fn managed_system_prompt_source(catalog_key: &str, locale: &str) -> AgentPromptPrefixSource {
    let definition = crate::domain::prompt::builtin_prompt(catalog_key)
        .unwrap_or_else(|| panic!("managed system prompt is missing: {catalog_key}"));
    AgentPromptPrefixSource {
        catalog_key: definition.catalog_key.clone(),
        prompt_code: definition.prompt_code.clone(),
        locale: locale.to_string(),
    }
}

fn invocation_locale(request_locale: Option<&str>) -> String {
    SupportedLocale::from_raw(request_locale.unwrap_or("zh-CN"))
        .as_str()
        .to_string()
}

/// The catalog records the run actually authorized. The whole workspace catalog
/// is narrowed to the run's authorized tool names so a domain the agent cannot
/// use never reaches the model, and the visible list stays stable for the run.
pub(crate) fn authorized_domain_records(
    management: &AgentManagementService,
    authorized_tools: &[EffectiveTool],
    locale: &str,
) -> Vec<crate::domain::agent::ToolCatalogRecord> {
    let mut records: HashMap<_, _> = management
        .list_tools(None, None, locale)
        .into_iter()
        .map(|record| (record.tool_name.clone(), record))
        .collect();
    authorized_tools
        .iter()
        .filter_map(|tool| {
            let code = tool
                .domain_code
                .as_deref()
                .map(str::trim)
                .filter(|code| !code.is_empty());
            let mut record = records.remove(&tool.tool_name).or_else(|| {
                code.map(|code| crate::domain::agent::ToolCatalogRecord {
                    display: None,
                    tool_code: tool.tool_name.clone(),
                    tool_name: tool.tool_name.clone(),
                    domain_code: code.into(),
                    category: None,
                    source: Default::default(),
                    name: tool.tool_name.clone(),
                    description: tool.description.clone(),
                    operation_kind: crate::domain::agent::ToolOperationKind::Unknown,
                    input_schema: tool.input_schema.clone(),
                    module_owner: "Framework".into(),
                    availability_state: crate::domain::agent::ToolAvailabilityState::Available,
                    sample_result_summary: String::new(),
                })
            })?;
            // Match binding labels: the run's explicit group wins over the catalog.
            if let Some(code) = code {
                record.domain_code = code.into();
            }
            Some(record)
        })
        .collect()
}

/// Authorized inventory is data, kept separate from stable gateway instructions.
pub(crate) fn visible_domain_context(
    domains: &[crate::domain::agent::ToolDomainRecord],
) -> AgentMessage {
    AgentMessage::new(
        "user",
        vec![crate::domain::agent::AgentMessagePart::Context {
            kind: "tool_domains".into(),
            text: serde_json::to_string(domains).expect("domain records serialize"),
            metadata: None,
        }],
    )
}

pub(crate) struct PreparedAgentRun {
    pub leases: crate::application::agent::tool_framework::FrameworkLeaseGuard,
    pub run_code: String,
    pub conversation_code: Option<String>,
    pub workspace_code: String,
    pub user_code: Option<String>,
    pub app_code: Option<String>,
    pub task_code: Option<String>,
    pub agent_code: String,
    pub objective: String,
    pub model_code: String,
    pub requested_system_agent_session_id: Option<String>,
    pub runtime_selection: RuntimeAdapterSelection,
    pub context: AgentRunContext,
}

#[derive(Clone)]
pub(crate) struct AgentRunContextBuilder {
    tool_execution_mode: crate::domain::agent::ToolExecutionMode,
    platform_scope: bool,
    framework_host: Option<crate::application::agent::FrameworkHost>,
    collaboration_owner: Option<crate::domain::agent::AgentCollaborationOwner>,
    assistant_work_context: Option<crate::application::assistant::AssistantService>,
    workspaces: WorkspaceService,
    apps: AppService,
    variables: VariableService,
    skills: SkillRegistryService,
    management: AgentManagementService,
    mcp: McpService,
    event_bus: EventBusHandle,
    tool_registry: Arc<ToolRegistry>,
    short_term_memory: Option<ShortTermMemoryService>,
    prompt_service: Option<PromptService>,
    auto_model_route: Option<AutoModelRouteContext>,
    model_router: Option<crate::application::model_router::AgentModelRouter>,
    directory_inspector:
        Option<Arc<dyn crate::application::project::ApplicationDirectoryInspector>>,
}

impl AgentRunContextBuilder {
    pub(crate) fn with_tool_execution_mode(
        mut self,
        mode: crate::domain::agent::ToolExecutionMode,
    ) -> Self {
        self.tool_execution_mode = mode;
        self
    }

    pub(crate) fn with_framework_host(
        mut self,
        host: Option<crate::application::agent::FrameworkHost>,
    ) -> Self {
        self.framework_host = host;
        self
    }

    pub(crate) fn with_assistant_work_context(
        mut self,
        assistants: Option<crate::application::assistant::AssistantService>,
    ) -> Self {
        self.assistant_work_context = assistants;
        self
    }

    pub(crate) fn with_execution_identity(
        mut self,
        platform_scope: bool,
        owner: Option<crate::domain::agent::AgentCollaborationOwner>,
    ) -> Self {
        self.platform_scope = platform_scope;
        self.collaboration_owner = owner;
        self
    }

    pub(crate) fn with_model_router(
        mut self,
        router: crate::application::model_router::AgentModelRouter,
    ) -> Self {
        self.model_router = Some(router);
        self
    }

    pub(crate) fn model_router(
        &self,
    ) -> Option<crate::application::model_router::AgentModelRouter> {
        self.model_router.clone()
    }

    pub(crate) fn directory_inspector(
        &self,
    ) -> Option<Arc<dyn crate::application::project::ApplicationDirectoryInspector>> {
        self.directory_inspector.clone()
    }

    pub(crate) fn agent_management(&self) -> &AgentManagementService {
        &self.management
    }

    pub(crate) fn new(
        workspaces: WorkspaceService,
        apps: AppService,
        variables: VariableService,
        skills: SkillRegistryService,
        management: AgentManagementService,
        mcp: McpService,
        event_bus: EventBusHandle,
        tool_registry: Arc<ToolRegistry>,
        short_term_memory: Option<ShortTermMemoryService>,
        prompt_service: Option<PromptService>,
        auto_model_route: Option<AutoModelRouteContext>,
        directory_inspector: Option<
            Arc<dyn crate::application::project::ApplicationDirectoryInspector>,
        >,
    ) -> Self {
        Self {
            platform_scope: false,
            tool_execution_mode: Default::default(),
            framework_host: None,
            collaboration_owner: None,
            assistant_work_context: None,
            workspaces,
            apps,
            variables,
            skills,
            management,
            mcp,
            event_bus,
            tool_registry,
            short_term_memory,
            prompt_service,
            auto_model_route,
            model_router: None,
            directory_inspector,
        }
    }

    pub(crate) async fn prepare(
        &self,
        command: AgentInvocationCommand,
        user_code: Option<String>,
        locale: Option<String>,
    ) -> Result<PreparedAgentRun, AgentInvocationError> {
        let AgentInvocationCommand {
            run_code: requested_run_code,
            prompt: requested_prompt,
            workspace_code: requested_workspace_code,
            conversation_code: requested_conversation_code,
            agent_code: requested_agent_code,
            system_model_id: requested_system_model_id,
            system_agent_session_id: requested_system_agent_session_id,
            app_code: requested_app_code,
            execution_scope_override,
            task_code: requested_task_code,
            model: requested_model,
            limits: requested_limits,
            tools: requested_tools,
            attachments: requested_attachments,
            page_context,
            supports_user_input_requests,
            transcript_input_kind,
            transcript_input_metadata,
        } = command;
        let prompt = requested_prompt
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                transcript_input_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("annotations"))
                    .and_then(|value| value.as_array())
                    .filter(|items| !items.is_empty())
                    .map(|_| String::new())
            })
            .ok_or(AgentInvocationError::PromptRequired)?;
        let workspace_code = requested_workspace_code
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let workspace = if let Some(code) = workspace_code.as_deref() {
            let workspace = self
                .workspaces
                .get_workspace(code)
                .map_err(|_| AgentInvocationError::WorkspaceInvalid)?;
            if workspace.status != WorkspaceStatus::Active {
                return Err(AgentInvocationError::WorkspaceInvalid);
            }
            Some(workspace)
        } else if self.platform_scope {
            None
        } else {
            return Err(AgentInvocationError::WorkspaceRequired);
        };
        // Empty internal scope denotes absence; it is never looked up as a Workspace.
        let workspace_code = workspace
            .as_ref()
            .map(|workspace| workspace.workspace_code.as_str())
            .unwrap_or("");
        let execution_workspace_code = execution_scope_override
            .as_ref()
            .and_then(|scope| scope.workspace_code.as_deref())
            .map(str::trim)
            .filter(|code| !code.is_empty())
            .unwrap_or(workspace_code);
        if !self.platform_scope && execution_workspace_code != workspace_code {
            return Err(AgentInvocationError::WorkspaceInvalid);
        }
        let execution_workspace = if execution_workspace_code.is_empty() {
            None
        } else {
            let value = self
                .workspaces
                .get_workspace(execution_workspace_code)
                .map_err(|_| AgentInvocationError::WorkspaceInvalid)?;
            if value.status != WorkspaceStatus::Active {
                return Err(AgentInvocationError::WorkspaceInvalid);
            }
            Some(value)
        };
        let app = requested_app_code
            .as_deref()
            .map(|code| self.apps.get_app(code))
            .transpose()
            .map_err(|_| AgentInvocationError::WorkspaceInvalid)?;
        if app
            .as_ref()
            .is_some_and(|app| app.workspace_code != execution_workspace_code)
        {
            return Err(AgentInvocationError::WorkspaceInvalid);
        }
        let resolved_agent = if workspace.is_some() && !self.platform_scope {
            resolve_invocation_agent(&self.management, workspace_code, requested_agent_code)?
        } else {
            self.management
                .resolve_platform_agent_runtime(requested_agent_code.as_deref())
                .map_err(|error| {
                    map_agent_error(requested_agent_code.as_deref().unwrap_or(""), error)
                })?
        };
        let model =
            resolve_invocation_model(&resolved_agent, requested_model, requested_system_model_id)?;
        let model_code = model.model_code.clone();
        let agent_kind = agent_kind_for_runtime(resolved_agent.runtime_type);
        let locale = invocation_locale(locale.as_deref());
        let run_code = requested_run_code.unwrap_or_else(generate_agent_run_code);
        eliteforge_agent_core::RunId::parse_str(&run_code)
            .map_err(|_| AgentInvocationError::RuntimeFailed("runCode must be a UUID".into()))?;
        let leases = self.tool_registry.framework_lease_guard(&run_code);
        let mut tool_access = self.resolve_tool_access(
            workspace_code,
            &resolved_agent.agent_code,
            &locale,
            requested_tools,
        )?;

        let limits = requested_limits.unwrap_or_else(|| {
            tool_access
                .policy
                .as_ref()
                .map(|policy| AgentRunLimits {
                    max_steps: policy.max_steps,
                    max_tool_calls: policy.max_tool_calls,
                    max_parallel_tool_calls: policy.max_parallel_tool_calls,
                })
                .unwrap_or_default()
        });
        let variables = if execution_workspace.is_some() {
            self.variables.runtime_variables(execution_workspace_code)
        } else {
            self.variables.platform_runtime_variables()
        }
        .map_err(AgentInvocationError::RuntimeVariableResolutionFailed)?
        .into_iter()
        .map(|variable| EffectiveVariable {
            key: variable.key,
            value: variable.value,
            source: variable.source,
            sensitive: variable.is_sensitive,
        })
        .collect::<Vec<_>>();
        let variable_values = variables
            .iter()
            .map(|variable| (variable.key.clone(), variable.value.clone()))
            .collect::<HashMap<_, _>>();
        let skills = if matches!(resolved_agent.runtime_type, AgentRuntimeType::React) {
            self.skills
                .effective_agent_skills(&resolved_agent.agent_code)
                .map_err(|error| AgentInvocationError::RuntimeFailed(error.to_string()))?
                .into_iter()
                .filter(|skill| skill.assigned && skill.available)
                .map(|skill| EffectiveSkill {
                    skill_name: skill.skill_name,
                    description: skill.description.unwrap_or_default(),
                    instruction: None,
                    tool_names: Vec::new(),
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        self.materialize_mcp_tools(
            &mut tool_access,
            &variable_values,
            &McpWarningPublishContext {
                run_code: &run_code,
                conversation_code: requested_conversation_code.as_deref(),
                workspace_code: workspace_code,
                user_code: user_code.as_deref(),
                agent_kind,
                model: &model,
                app_code: requested_app_code.as_deref(),
                task_code: requested_task_code.as_deref(),
            },
        )
        .await?;
        let tool_assignment =
            tool_access
                .policy
                .as_ref()
                .map(|policy| crate::domain::agent::AgentToolAssignment {
                    mode: policy.tool_assignment_mode,
                    selected_category_codes: policy.selected_category_codes.clone(),
                    selected_tool_codes: policy.selected_tool_codes.clone(),
                    requested_tool_names: tool_access
                        .requested_tool_names
                        .iter()
                        .cloned()
                        .collect(),
                });
        let PreparedToolAccess {
            mut tools,
            mut effective_tools,
            ..
        } = tool_access;

        // Standalone registries create a default host with tools enabled.
        let gateway_enabled = matches!(resolved_agent.runtime_type, AgentRuntimeType::React)
            && self
                .framework_host
                .as_ref()
                .is_none_or(|host| host.gateway_enabled());
        if gateway_enabled
            && requested_conversation_code.is_some()
            && supports_user_input_requests
            && tools
                .iter()
                .all(|tool| tool.tool_name != crate::application::agent::CHAT_ASK_USER_TOOL)
        {
            let ask_user = crate::application::agent::ask_user_definition(&locale);
            tools.push(ask_user.clone());
            effective_tools.push(ask_user);
        }

        let mut messages = Vec::new();
        let mut runtime_context_messages = Vec::new();
        let mut prompt_prefix_sources = Vec::new();
        let empty_runtime_prompt_values = HashMap::new();
        if matches!(resolved_agent.runtime_type, AgentRuntimeType::React) {
            let guidance = if workspace.is_some() && !self.platform_scope {
                self.management
                    .current_internal_agent_guidance(workspace_code, &resolved_agent.agent_code)
            } else {
                self.management
                    .platform_internal_agent_policy(&resolved_agent.agent_code)
                    .map(|policy| {
                        (!policy.role_prompt.trim().is_empty()).then_some(policy.role_prompt)
                    })
            }
            .map_err(|_| {
                AgentInvocationError::RuntimeFailed("internal agent config load failed".into())
            })?;
            if let Some(guidance) = guidance {
                messages.push(
                    AgentMessage::system(guidance)
                        .with_instruction_origin(eliteforge_agent_core::InstructionOrigin::Agent),
                );
                prompt_prefix_sources.push(None);
            }
        }
        if gateway_enabled && requested_conversation_code.is_some() && supports_user_input_requests
        {
            messages.push(
                AgentMessage::system(managed_system_prompt(
                    self.prompt_service.as_ref(),
                    "chat.ask-user",
                    &locale,
                    &empty_runtime_prompt_values,
                )?)
                .with_instruction_origin(eliteforge_agent_core::InstructionOrigin::Plugin),
            );
            prompt_prefix_sources
                .push(Some(managed_system_prompt_source("chat.ask-user", &locale)));
        }
        if gateway_enabled {
            // The gateway hides every wrapped tool behind discovery, so the
            // run instructions must carry the authorized domain inventory:
            // otherwise the model cannot see the scope of its own abilities.
            // Deriving it from the run's authorized tools (not the whole
            // catalog) keeps unauthorized domains invisible.
            let domain_records =
                authorized_domain_records(&self.management, &effective_tools, &locale);
            // Resolve fallback labels once, then give both prompt projection
            // and leased bindings the same run-owned taxonomy.
            let domain_labels: HashMap<_, _> = domain_records
                .iter()
                .map(|record| (record.tool_name.as_str(), record.domain_code.as_str()))
                .collect();
            for tool in tools.iter_mut().chain(effective_tools.iter_mut()) {
                if tool
                    .domain_code
                    .as_deref()
                    .is_none_or(|code| code.trim().is_empty())
                {
                    tool.domain_code = domain_labels
                        .get(tool.tool_name.as_str())
                        .map(|code| (*code).to_owned());
                }
            }
            let visible_domains =
                crate::application::agent::BusinessToolCatalog::domains_for_records(
                    &domain_records,
                    &locale,
                );
            runtime_context_messages.push(visible_domain_context(&visible_domains));
            messages.push(
                AgentMessage::system(managed_system_prompt(
                    self.prompt_service.as_ref(),
                    "agent.domain-discovery",
                    &locale,
                    &empty_runtime_prompt_values,
                )?)
                .with_instruction_origin(eliteforge_agent_core::InstructionOrigin::Plugin),
            );
            prompt_prefix_sources.push(Some(managed_system_prompt_source(
                "agent.domain-discovery",
                &locale,
            )));
        }
        // Keep all trusted system instructions contiguous before user-role data
        // blocks, so HostRunState retains identity and role across model steps.
        for message in runtime_context_messages {
            messages.push(message);
            prompt_prefix_sources.push(None);
        }
        let mut prompt_prefix_messages = messages.clone();
        let mut user_prompt_message = user_prompt_message(prompt.clone(), &requested_attachments);
        crate::application::agent::page_context::record_snapshot(
            &mut user_prompt_message,
            page_context.as_ref(),
            &locale,
        );
        messages.push(user_prompt_message.clone());

        let local_directory = execution_scope_override
            .as_ref()
            .and_then(|scope| scope.local_directory.clone())
            .or_else(|| app.as_ref().map(|app| app.local_directory.clone()));
        let branch_name = execution_scope_override
            .as_ref()
            .and_then(|scope| scope.branch_name.clone());
        let repository_url = execution_scope_override
            .as_ref()
            .and_then(|scope| scope.repository_url.clone())
            .or_else(|| app.as_ref().and_then(|app| app.repository_url.clone()));
        let app_code = execution_scope_override
            .as_ref()
            .and_then(|scope| scope.app_code.clone())
            .or_else(|| app.as_ref().map(|app| app.app_code.clone()));
        let task_code = requested_task_code;
        let requested_system_agent_session_id =
            normalize_optional(requested_system_agent_session_id);
        let mut context = AgentRunContext {
            tool_execution_mode: self.tool_execution_mode,
            system_agent_fast_mode: resolved_agent.system_fast_mode,
            collaboration_owner: self.collaboration_owner.clone(),
            agent_code: Some(resolved_agent.agent_code.clone()),
            auto_model_route: self.auto_model_route.clone(),
            conversation_code: requested_conversation_code.clone(),
            run_code: run_code.clone(),
            workspace_code: workspace_code.to_owned(),
            user_code: user_code.clone(),
            locale,
            request_id: None,
            stream_key: requested_conversation_code
                .clone()
                .or_else(|| Some(run_code.clone())),
            agent_kind,
            system_agent_session_id: None,
            model,
            variables,
            skills,
            tools,
            effective_tools,
            tool_assignment,
            execution_scope: execution_scope_override.unwrap_or(AgentExecutionScope {
                applications: Vec::new(),
                project_code: None,
                app_code: app_code.clone(),
                repository_url,
                local_directory,
                branch_name,
                task_code: task_code.clone(),
                allowed_local_directories: Vec::new(),
                ..Default::default()
            }),
            external_topic_snapshot: None,
            limits,
            messages: messages.into(),
            observations: Vec::new(),
            short_term_memory: None,
            started_at: chrono::Utc::now().to_rfc3339(),
        };
        // Revalidate inherited snapshots as well as fresh user submissions. Names
        // come from the workspace service, never from the browser or agent config.
        context.execution_scope.workspace_code = execution_workspace
            .as_ref()
            .map(|value| value.workspace_code.clone());
        context.execution_scope.workspace_name =
            execution_workspace.as_ref().map(|value| value.name.clone());
        if page_context.is_some() {
            context.execution_scope.page_context =
                crate::application::agent::page_context::normalized(page_context.as_ref());
        }
        if let Some(host) = &self.framework_host {
            host.prepare_working_directory(&mut context)
                .map_err(AgentInvocationError::RuntimeNotStarted)?;
            if context.agent_kind == crate::domain::agent::AgentKind::SystemCli {
                if let (Some(service), Some(project), Some(user)) = (
                    host.topics(),
                    context.execution_scope.project_code.as_deref(),
                    context.user_code.as_deref(),
                ) {
                    context.external_topic_snapshot = Some(match service.get(user, project) {
                        Ok(memo) => crate::application::topic::external_snapshot(&memo),
                        Err(_) => "[主题信息不可用，请勿假定已读取主题资料。]".into(),
                    });
                }
            }
        }
        let runtime_selection = runtime_selection_for_agent(
            &resolved_agent,
            context.execution_scope.local_directory.clone(),
        )?;
        if let Some(message) =
            crate::application::agent::execution_context_message(&context.execution_scope)
        {
            prompt_prefix_messages.push(message.clone());
            prompt_prefix_sources.push(None);
            let index = context.messages.len().saturating_sub(1);
            std::sync::Arc::make_mut(&mut context.messages).insert(index, message);
        }
        if context.agent_kind == crate::domain::agent::AgentKind::React {
            if let Some(inspector) = &self.directory_inspector {
                if let Some(message) = crate::application::agent::project_instructions_message(
                    &context.execution_scope,
                    inspector.as_ref(),
                )
                .map_err(AgentInvocationError::RuntimeFailed)?
                {
                    prompt_prefix_messages.push(message.clone());
                    prompt_prefix_sources.push(None);
                    let index = context.messages.len().saturating_sub(1);
                    std::sync::Arc::make_mut(&mut context.messages).insert(index, message);
                }
            }
        }
        activate_short_term_memory(
            self.short_term_memory.as_ref(),
            &mut context,
            &runtime_selection,
            requested_conversation_code.as_deref(),
            &resolved_agent.agent_code,
            &prompt,
            prompt_prefix_messages,
            prompt_prefix_sources,
            Some(user_prompt_message),
            transcript_input_kind.as_deref(),
            transcript_input_metadata,
        )?;

        Ok(PreparedAgentRun {
            leases,
            run_code,
            conversation_code: context.conversation_code.clone(),
            workspace_code: context.workspace_code.clone(),
            user_code,
            app_code,
            task_code,
            agent_code: resolved_agent.agent_code,
            objective: prompt,
            model_code,
            requested_system_agent_session_id,
            runtime_selection,
            context,
        })
    }
}

fn resolve_invocation_agent(
    management: &AgentManagementService,
    workspace_code: &str,
    requested_agent_code: Option<String>,
) -> Result<ResolvedAgentRuntime, AgentInvocationError> {
    let normalized = normalize_optional(requested_agent_code);
    let requested_agent_code = normalized.as_deref();
    management
        .resolve_workspace_agent_runtime(workspace_code, requested_agent_code)
        .map_err(|error| map_agent_error(requested_agent_code.unwrap_or(MAIN_AGENT_CODE), error))
}

fn resolve_invocation_model(
    resolved_agent: &ResolvedAgentRuntime,
    requested_model: Option<AgentModelConfig>,
    requested_system_model_id: Option<String>,
) -> Result<AgentModelConfig, AgentInvocationError> {
    match resolved_agent.runtime_type {
        AgentRuntimeType::React => requested_model.ok_or(AgentInvocationError::ModelRequired),
        AgentRuntimeType::CodexExec
        | AgentRuntimeType::CodexAppServer
        | AgentRuntimeType::DshHeadless
        | AgentRuntimeType::OpenCodeRun
        | AgentRuntimeType::OpenCodeServe
        | AgentRuntimeType::OpenCodeAcp => {
            let model_name = resolved_agent
                .system_model_id
                .clone()
                .or_else(|| normalize_optional(requested_system_model_id))
                .ok_or(AgentInvocationError::ModelRequired)?;
            let mut config = build_system_runtime_model_config(&model_name, None);
            config.reasoning_effort = resolved_agent.system_reasoning_effort;
            Ok(config)
        }
    }
}

fn runtime_selection_for_agent(
    resolved_agent: &ResolvedAgentRuntime,
    local_directory: Option<String>,
) -> Result<RuntimeAdapterSelection, AgentInvocationError> {
    match resolved_agent.runtime_type {
        AgentRuntimeType::React => Ok(RuntimeAdapterSelection::React),
        AgentRuntimeType::CodexExec => Ok(RuntimeAdapterSelection::CodexExec {
            command_name: required_runtime_command_name(resolved_agent)?,
            working_directory: local_directory,
        }),
        AgentRuntimeType::DshHeadless => Ok(RuntimeAdapterSelection::DshHeadless {
            command_name: required_runtime_command_name(resolved_agent)?,
            working_directory: local_directory,
        }),
        AgentRuntimeType::OpenCodeRun => Ok(RuntimeAdapterSelection::OpenCodeRun {
            command_name: required_runtime_command_name(resolved_agent)?,
            working_directory: local_directory,
        }),
        AgentRuntimeType::CodexAppServer
        | AgentRuntimeType::OpenCodeServe
        | AgentRuntimeType::OpenCodeAcp => Err(AgentInvocationError::RuntimeFailed(format!(
            "agent `{}` runtime type `{}` is not configured for invocation",
            resolved_agent.agent_code,
            runtime_type_label(resolved_agent.runtime_type)
        ))),
    }
}
