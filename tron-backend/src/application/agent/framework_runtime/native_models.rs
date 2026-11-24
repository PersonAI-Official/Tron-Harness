//! Run-scoped authorization and native SDK model bindings. No transport retry loop.
use super::*;
use crate::domain::agent::{AgentError, AgentModelConfig, AgentRunContext};
use eliteforge_agent_framework as sdk;
use std::collections::BTreeMap;

#[derive(Clone)]
struct Binding {
    config: AgentModelConfig,
}
#[derive(Clone)]
struct Bindings {
    models: BTreeMap<fw::ModelRef, Binding>,
    summaries: BTreeMap<fw::ModelRef, fw::ModelRef>,
}
pub(crate) struct NativeModelScope {
    execution: sdk::ModelExecutionScope,
    bindings: Bindings,
    primary: Option<fw::ModelRef>,
    retry: Arc<super::retry::SessionModelRetry>,
}
pub(super) fn protocol(config: &AgentModelConfig) -> &'static str {
    match config.protocol {
        crate::domain::agent::AgentModelProtocol::Responses => "openai.responses",
        crate::domain::agent::AgentModelProtocol::ChatCompletions => "openai.chat-completions",
        crate::domain::agent::AgentModelProtocol::Embeddings => "openai.embeddings",
        crate::domain::agent::AgentModelProtocol::Anthropic => "anthropic.messages",
        crate::domain::agent::AgentModelProtocol::Gemini => "gemini.generate-content",
    }
}
fn reference(
    context: &AgentRunContext,
    config: &AgentModelConfig,
    scope: Option<&str>,
    auxiliary: bool,
) -> fw::ModelRef {
    use sha2::{Digest, Sha256};
    // Product credentials and run/conversation IDs do not become public identities.
    let endpoint_path = if config.model_code.starts_with("PERSON_AI:") {
        None
    } else {
        config.api_path.as_deref()
    };
    let identity = serde_json::json!([
        context.user_code,
        context.workspace_code,
        config.base_url,
        endpoint_path,
        protocol(config),
        config.model_code,
        scope,
        auxiliary
    ]);
    fw::ModelRef::new(
        format!(
            "product-{:x}",
            Sha256::digest(identity.to_string().as_bytes())
        ),
        &config.model_code,
    )
}
impl NativeModelScope {
    pub(crate) fn new(harness: &ReactAgent, context: &AgentRunContext) -> Result<Self, AgentError> {
        let configs = harness.framework_authorized_models(context)?;
        let runtime = sdk::ModelRuntime::new(sdk::ModelRuntimeUpdate::new("", 0, vec![]))
            .map_err(|e| AgentError::InvalidConfiguration(e.to_string()))?;
        let mut bindings = Bindings {
            models: BTreeMap::new(),
            summaries: BTreeMap::new(),
        };
        let mut order = vec![];
        let mut refreshers: Vec<Arc<dyn sdk::ModelCatalogRefresh>> = vec![];
        let fallback_context_window = harness
            .framework_context_config()
            .context
            .default_context_window_tokens;
        let binding_configs = configs
            .iter()
            .flat_map(|config| {
                [
                    config.clone(),
                    harness.framework_compaction_model_config(config),
                ]
            })
            .collect::<Vec<_>>();
        let mut prepared = harness.framework_bind_models(&binding_configs)?.into_iter();
        for config in configs {
            let (main_scope, main_client) = prepared.next().expect("main model binding");
            let (summary_scope, summary_client) = prepared.next().expect("summary model binding");
            let main = add_binding(
                &runtime,
                &mut bindings.models,
                main_scope.clone(),
                main_client,
                context,
                config.clone(),
                false,
                fallback_context_window,
            )?;
            let summary_config = harness.framework_compaction_model_config(&config);
            let summary = add_binding(
                &runtime,
                &mut bindings.models,
                summary_scope,
                summary_client,
                context,
                summary_config,
                true,
                fallback_context_window,
            )?;
            bindings.summaries.insert(main.clone(), summary);
            refreshers.push(Arc::new(AuthorizedRefresh {
                reference: main.clone(),
                harness: harness.clone(),
                context: {
                    let mut snapshot = context.clone();
                    snapshot.model = config.clone();
                    snapshot
                },
                scope: main_scope,
            }));
            order.push(main);
        }
        let primary = order.first().cloned().ok_or_else(|| {
            AgentError::InvalidConfiguration("no authorized model bindings".into())
        })?;
        let retry = Arc::new(super::retry::SessionModelRetry::default());
        let router = super::retry::SessionAutoRouter::new(retry.clone());
        let mut execution = sdk::ModelExecutionScope::new(
            runtime,
            Arc::new(fw::ChatAutoRoutingStrategy {
                routers: fw::AutoRouters {
                    source: Some(Arc::new(
                        crate::application::model_router::TronAutoRouteDataSource(
                            order
                                .iter()
                                .cloned()
                                .map(fw::AutoRouteCandidate::new)
                                .collect(),
                        ),
                    )),
                    chat: Some(Arc::new(router)),
                    ..Default::default()
                },
                routing: Default::default(),
            }),
        )
        .map_err(|e| AgentError::InvalidConfiguration(e.to_string()))?;
        execution.refreshers = refreshers;
        if context.auto_model_route.is_some() {
            execution.recovery = sdk::ModelRecoveryPolicy {
                candidates: order,
                allow_explicit_model_replacement: false,
            };
        }
        Ok(Self {
            execution,
            bindings,
            retry,
            primary: if context.auto_model_route.is_some() {
                None
            } else {
                Some(primary)
            },
        })
    }
    pub(crate) fn attach(self, request: &mut fw::RunRequest) {
        self.retry.attach(request);
        request.model = self.primary;
        self.execution.attach(request);
        request.extensions.insert(self.bindings);
    }
}
fn add_binding(
    runtime: &sdk::ModelRuntime,
    bindings: &mut BTreeMap<fw::ModelRef, Binding>,
    scope: Option<String>,
    client: Arc<dyn crate::application::agent::ModelClient>,
    context: &AgentRunContext,
    config: AgentModelConfig,
    auxiliary: bool,
    fallback_context_window: u32,
) -> Result<fw::ModelRef, AgentError> {
    if config.model_code.starts_with("PERSON_AI:")
        && let Some(expected) = context
            .auto_model_route
            .as_ref()
            .and_then(|route| route.authorization_scope.as_ref())
        && scope.as_ref() != Some(expected)
    {
        return Err(AgentError::InvalidConfiguration(
            "Auto authorization changed before model binding".into(),
        ));
    }
    let model = reference(context, &config, scope.as_deref(), auxiliary);
    if bindings.contains_key(&model) {
        return Ok(model);
    }
    let wire = crate::adapters::outbound::model::http_model_client::sdk_config(&config)
        .map_err(AgentError::InvalidConfiguration)?;
    let mut descriptor = eliteforge_agent_official_plugin_model_http::model_descriptor(&wire);
    let window = config
        .context_window_tokens
        .unwrap_or(fallback_context_window);
    if descriptor.context_window_tokens.is_none() {
        descriptor.context_window_tokens = Some(window as usize);
    }
    if descriptor.max_output_tokens.is_none() {
        // An unspecified output limit must leave room for instructions and tools,
        // including when the model catalog has no context-window metadata.
        descriptor.max_output_tokens = Some((window / 4).max(1));
    }
    let connection = fw::ModelConnection::new(
        &model.connection,
        Arc::new(TronChatModel {
            reference: model.clone(),
            config: config.clone(),
            client: client.clone(),
            auxiliary,
        }),
        vec![descriptor],
    )
    .with_protocol(protocol(&config))
    .with_token_estimator(Arc::new(
        super::super::token_estimation::TronTokenEstimator::new(&config),
    ));
    runtime
        .replace_connection(
            &model.connection,
            sdk::ModelRuntimeUpdate::new(
                scope.unwrap_or_else(|| model.connection.clone()),
                0,
                vec![connection],
            ),
        )
        .map_err(|e| AgentError::InvalidConfiguration(e.to_string()))?;
    bindings.insert(model.clone(), Binding { config });
    Ok(model)
}
pub(crate) fn synchronize_selected_model(
    state: &mut HostRunState,
    reference: &fw::ModelRef,
    extensions: &fw::Extensions,
    options: &fw::GenerationOptions,
) -> Result<(), fw::AgentError> {
    let bindings = extensions.get::<Bindings>().ok_or_else(|| {
        fw::AgentError::InvalidConfiguration("native model bindings missing".into())
    })?;
    let binding = bindings.models.get(reference).ok_or_else(|| {
        fw::AgentError::InvalidConfiguration("selected model is outside authorized bindings".into())
    })?;
    if state.context.model.identity() != binding.config.identity() {
        state.turn = TurnStateAdapter::new(
            binding.config.protocol,
            binding.config.responses_stateful_enabled,
        );
        state.prepared = None;
    }
    state.context.model = binding.config.clone();
    state.context.model.max_tokens = options.max_output_tokens.or(state.context.model.max_tokens);
    state.context.model.temperature = options.temperature.or(state.context.model.temperature);
    state.context.model.top_p = options.top_p.or(state.context.model.top_p);
    Ok(())
}
pub(crate) fn compaction_model_reference(
    extensions: &fw::Extensions,
    main: &fw::ModelRef,
) -> Result<fw::ModelRef, fw::AgentError> {
    extensions
        .get::<Bindings>()
        .and_then(|b| b.summaries.get(main).cloned())
        .ok_or_else(|| {
            fw::AgentError::InvalidConfiguration("authorized summary model binding missing".into())
        })
}

/// Reuse the auxiliary binding paired with the model selected by the existing router.
pub(crate) fn risk_review_model(
    extensions: &fw::Extensions,
    config: &AgentModelConfig,
) -> Result<fw::ModelRef, fw::AgentError> {
    let bindings = extensions
        .get::<Bindings>()
        .ok_or(fw::AgentError::MissingService("authorized model bindings"))?;
    bindings
        .summaries
        .iter()
        .find_map(|(main, auxiliary)| {
            bindings
                .models
                .get(main)
                .filter(|binding| binding.config.identity() == config.identity())
                .map(|_| auxiliary.clone())
        })
        .ok_or(fw::AgentError::MissingService(
            "authorized risk review model",
        ))
}

/// Re-read host authorization only. The SDK owns refresh fencing and recovery.
/// No inference is sent by discovery, and a switched account cannot publish here.
struct AuthorizedRefresh {
    reference: fw::ModelRef,
    harness: ReactAgent,
    context: AgentRunContext,
    scope: Option<String>,
}
#[async_trait]
impl sdk::ModelCatalogRefresh for AuthorizedRefresh {
    fn connection_id(&self) -> &str {
        &self.reference.connection
    }
    fn account_identity(&self) -> Option<sdk::ModelAccountIdentity> {
        self.harness
            .framework_model_authorization_scope(&self.context.model)
            .ok()
            .map(|scope| sdk::ModelAccountIdentity {
                scope: scope.unwrap_or_else(|| self.reference.connection.clone()),
                revision: 0,
            })
    }
    async fn refresh(&self) -> Result<sdk::ModelRuntimeUpdate, fw::ModelError> {
        let expected = sdk::ModelAccountIdentity {
            scope: self
                .scope
                .clone()
                .unwrap_or_else(|| self.reference.connection.clone()),
            revision: 0,
        };
        if self.account_identity() != Some(expected.clone()) {
            return Err(account_changed());
        }
        let config = self
            .harness
            .framework_refreshed_models(&self.context)
            .map_err(|e| fw::ModelError::Fatal(e.to_string()))?
            .into_iter()
            .find(|config| config.identity() == self.context.model.identity());
        let mut chat = vec![];
        if let Some(config) = config {
            if config != self.context.model {
                return Err(fw::ModelError::Unavailable(
                    "authorized model configuration changed; start a new model execution scope"
                        .into(),
                ));
            }
            if reference(&self.context, &config, self.scope.as_deref(), false) != self.reference {
                return Err(account_changed());
            }
            let client = self
                .harness
                .framework_bound_model_client(&config)
                .map_err(|e| fw::ModelError::Fatal(e.to_string()))?;
            let wire = crate::adapters::outbound::model::http_model_client::sdk_config(&config)
                .map_err(fw::ModelError::Fatal)?;
            chat.push(
                fw::ModelConnection::new(
                    &self.reference.connection,
                    Arc::new(TronChatModel {
                        reference: self.reference.clone(),
                        config: config.clone(),
                        client,
                        auxiliary: false,
                    }),
                    vec![eliteforge_agent_official_plugin_model_http::model_descriptor(&wire)],
                )
                .with_protocol(protocol(&config))
                .with_token_estimator(Arc::new(
                    super::super::token_estimation::TronTokenEstimator::new(&config),
                )),
            );
        }
        if self.account_identity() != Some(expected.clone()) {
            return Err(account_changed());
        }
        Ok(sdk::ModelRuntimeUpdate::new(
            expected.scope,
            expected.revision,
            chat,
        ))
    }
}
fn account_changed() -> fw::ModelError {
    fw::ModelError::Provider(Box::new(fw::ProviderModelError {
        authorization_attempt: None,
        provider: "authorized-model-runtime".into(),
        category: fw::ProviderErrorCategory::AccountChanged,
        http_status: None,
        business_code: None,
        error_type: None,
        model: None,
        message: "model authorization changed during discovery".into(),
    }))
}
