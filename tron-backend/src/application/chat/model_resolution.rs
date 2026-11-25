use crate::{
    application::{
        agent::{
            AgentInvocationError, AgentManagementService, ResolvedAgentRuntime,
            build_system_runtime_model_config,
        },
        model::ModelService,
        model_provider::ModelProviderService,
    },
    domain::{
        agent::{AgentModelConfig, AgentRuntimeType, ManagedSystemAgentRecord},
        chat::{ChatError, ChatSession},
        model_provider::{
            ModelProviderCode, ModelSelection, ModelSelectionKind, ProviderRouteCode,
        },
    },
};

use super::{ChatSendError, SendChatMessageCommand};

pub(crate) fn prepare_send_model(
    models: &ModelService,
    providers: &ModelProviderService,
    agent_management: &AgentManagementService,
    session: &ChatSession,
    request: &SendChatMessageCommand,
    runtime: &ResolvedAgentRuntime,
) -> Result<PreparedChatModel, ChatSendError> {
    if let Some(selection) = request.model_selection.as_ref() {
        validate_chat_model_selection(selection)?;
    }
    if matches!(runtime.runtime_type, AgentRuntimeType::React) {
        let is_auto = request.model_selection.as_ref().is_some_and(|selection| {
            selection.kind == crate::domain::model_provider::ModelSelectionKind::Auto
                || (selection.kind
                    == crate::domain::model_provider::ModelSelectionKind::ProviderRoute
                    && selection.route_code
                        == Some(crate::domain::model_provider::ProviderRouteCode::ChatAuto))
        });
        if is_auto
            || (session.workspace_code.is_empty()
                && request.model_selection.is_none()
                && request
                    .model_code
                    .as_deref()
                    .is_none_or(|code| code.trim().is_empty()))
        {
            let router = crate::application::model_router::AgentModelRouter::new(
                models.clone(),
                providers.clone(),
            )
            .with_agent_management(agent_management.clone());
            let (model, auto_route) = router
                .resolve_initial(
                    &session.workspace_code,
                    &session.session_code,
                    &runtime.agent_code,
                    request.reasoning_effort,
                )
                .map_err(|message| {
                    ChatSendError::Agent(AgentInvocationError::RuntimeFailed(message))
                })?;
            return Ok(PreparedChatModel {
                config: model,
                auto_route: Some(auto_route),
                auto_router: Some(router),
            });
        }
        let fixed_model_code = request.model_selection.as_ref().and_then(|selection| {
            (selection.kind == crate::domain::model_provider::ModelSelectionKind::Fixed
                && selection.provider_code.is_none())
            .then(|| {
                selection
                    .model_code
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
            })
            .flatten()
        });
        match providers.resolve_chat_selection(
            &session.workspace_code,
            request.model_selection.as_ref(),
            request.model_code.as_deref(),
            request.reasoning_effort,
        ) {
            Ok(Some(config)) => {
                if config.model_code == "PERSON_AI:ROUTE:ChatAuto" {
                    let router = crate::application::model_router::AgentModelRouter::new(
                        models.clone(),
                        providers.clone(),
                    )
                    .with_agent_management(agent_management.clone());
                    let (model, auto_route) = router
                        .resolve_initial(
                            &session.workspace_code,
                            &session.session_code,
                            &runtime.agent_code,
                            request.reasoning_effort,
                        )
                        .map_err(|message| {
                            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(message))
                        })?;
                    return Ok(PreparedChatModel {
                        config: model,
                        auto_route: Some(auto_route),
                        auto_router: Some(router),
                    });
                }
                return Ok(PreparedChatModel::fixed(config));
            }
            Ok(None) => {}
            Err(error) => {
                return Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
                    error.to_string(),
                )));
            }
        }
        let config = models
            .text_agent_model_config_with_reasoning_effort(
                &session.workspace_code,
                fixed_model_code.or(request.model_code.as_deref()),
                request.reasoning_effort,
            )
            .map_err(|error| match error {
                crate::domain::model::ModelError::ReasoningEffortUnsupported => {
                    ChatSendError::Agent(AgentInvocationError::RuntimeFailed(error.to_string()))
                }
                _ => ChatSendError::Chat(ChatError::StorageFailure),
            })?
            .ok_or(ChatSendError::Agent(AgentInvocationError::ModelRequired))?;
        return Ok(PreparedChatModel::fixed(config));
    }

    let model_name = runtime
        .system_model_id
        .as_deref()
        .or(request.system_model_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(ChatSendError::Agent(AgentInvocationError::ModelRequired))?;

    let system_agent = agent_management
        .list_workspace_system_agents(&session.workspace_code)
        .map_err(|_| {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
                "workspace system agent policy load failed".into(),
            ))
        })?
        .into_iter()
        .find(|record| record.agent_code.eq_ignore_ascii_case(&runtime.agent_code))
        .ok_or_else(|| {
            ChatSendError::Agent(AgentInvocationError::RuntimeFailed(format!(
                "agent `{}` was not found",
                runtime.agent_code
            )))
        })?;

    let matched_model = validate_system_agent_model(&system_agent, &model_name)?;
    Ok(PreparedChatModel::fixed(build_system_runtime_model_config(
        &model_name,
        matched_model.and_then(|item| item.context_window_tokens),
    )))
}

pub(crate) struct PreparedChatModel {
    pub config: AgentModelConfig,
    pub auto_route: Option<crate::domain::agent::AutoModelRouteContext>,
    pub auto_router: Option<crate::application::model_router::AgentModelRouter>,
}

impl PreparedChatModel {
    fn fixed(config: AgentModelConfig) -> Self {
        Self {
            config,
            auto_route: None,
            auto_router: None,
        }
    }
}

fn validate_chat_model_selection(selection: &ModelSelection) -> Result<(), ChatSendError> {
    let invalid = |message: &str| {
        ChatSendError::Agent(AgentInvocationError::RuntimeFailed(message.to_owned()))
    };
    match selection.kind {
        ModelSelectionKind::Auto => {
            if selection.provider_code.is_some()
                || selection.model_code.is_some()
                || selection.quality_type.is_some()
                || selection.route_code.is_some()
            {
                return Err(invalid(
                    "Auto modelSelection cannot include a model or provider",
                ));
            }
            Ok(())
        }
        ModelSelectionKind::Fixed => {
            if selection.route_code.is_some() {
                return Err(invalid("Fixed modelSelection cannot include routeCode"));
            }
            match selection.provider_code {
                Some(ModelProviderCode::PersonAi) if selection.quality_type.is_none() => {
                    return Err(invalid(
                        "Person-AI Fixed modelSelection requires qualityType",
                    ));
                }
                None if selection.quality_type.is_some() => {
                    return Err(invalid(
                        "platform Fixed modelSelection cannot include qualityType",
                    ));
                }
                _ => {}
            }
            if selection
                .model_code
                .as_deref()
                .is_none_or(|model_code| model_code.trim().is_empty())
            {
                return Err(invalid("Fixed modelSelection requires modelCode"));
            }
            Ok(())
        }
        ModelSelectionKind::ProviderRoute => {
            if selection.provider_code != Some(ModelProviderCode::PersonAi)
                || selection.route_code != Some(ProviderRouteCode::ChatAuto)
                || selection.model_code.is_some()
                || selection.quality_type.is_some()
            {
                return Err(invalid(
                    "chat ProviderRoute modelSelection must be PersonAi ChatAuto without modelCode",
                ));
            }
            Ok(())
        }
    }
}

fn validate_system_agent_model(
    system_agent: &ManagedSystemAgentRecord,
    model_name: &str,
) -> Result<Option<crate::domain::agent::SystemAgentModelRecord>, ChatSendError> {
    if !system_agent.enabled || !system_agent.available {
        return Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
            format!(
                "agent `{}` is unavailable or disabled for this workspace",
                system_agent.agent_code
            ),
        )));
    }

    if system_agent.models.is_empty() {
        return Ok(None);
    }

    let model_matched = system_agent
        .models
        .iter()
        .find(|item| item.model_id.eq_ignore_ascii_case(model_name));
    if let Some(model) = model_matched {
        return Ok(Some(model.clone()));
    }

    Err(ChatSendError::Agent(AgentInvocationError::RuntimeFailed(
        format!(
            "model `{model_name}` is not available for agent `{}`",
            system_agent.agent_code
        ),
    )))
}
