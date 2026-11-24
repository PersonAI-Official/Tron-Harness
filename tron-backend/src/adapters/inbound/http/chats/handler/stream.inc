pub async fn send_message(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(session_code): Path<String>,
    Json(request): Json<SendChatMessageRequest>,
) -> Response {
    let mut request = SendChatMessageCommand::from(request);
    let Ok(session) = state.chats.get_session(&session_code) else {
        return chat_error_response(ChatError::SessionNotFound);
    };
    if let Err(error) = state.chats.ensure_session_writable(&session_code) {
        return chat_error_response(error);
    }
    if let Err(error) = state.chats.ensure_session_directory_available(&session_code) {
        return chat_error_response(error);
    }
    if let Err(response) = ensure_no_active_run(&state, &session_code) {
        return response;
    }
    if let Err(response) = ensure_native_resume(&state, &session_code, &request.resume).await {
        return response;
    }
    let resolved_runtime = match resolve_chat_agent_runtime(&state, &session, &request) {
        Ok(runtime) => runtime,
        Err(error) => return chat_send_error_response(error),
    };
    if let Err(error) = state.chats.validate_composer_input(&shared::effective_agent_code(&session, &request), request.content.as_deref().unwrap_or_default(), &request.requested_skills) {
        return chat_error_response(error);
    }
    let run_code = request.run_code.clone().unwrap_or_else(generate_agent_run_code);
    request.run_code = Some(run_code.clone());
    let prepared_model = match prepare_send_model(&state, &session, &request, &resolved_runtime) {
        Ok(model) => model,
        Err(error) => return chat_send_error_response(error),
    };
    let auto_model_route = prepared_model.auto_route.clone();
    let auto_model_router = prepared_model.auto_router.clone();
    let resolved_model = prepared_model.config;
    if let Err(error) = state.model_providers.record_requested_runtime_snapshot(
        Some(&run_code),
        &session.session_code,
        &session.workspace_code,
        &resolved_model,
    ) {
        return chat_send_error_response(ChatSendError::Agent(
            crate::application::agent::AgentInvocationError::RuntimeFailed(error.to_string()),
        ));
    }
    let resolved_model = crate::adapters::outbound::model::with_person_ai_conversation(
        resolved_model,
        &session.session_code,
        Some(&run_code),
    );

    let agent_service = AgentInvocationService::new(
        state.workspaces.clone(),
        state.apps.clone(),
        state.variables.clone(),
        state.skills.clone(),
        state.agent_management.clone(),
        state.agent_runs.clone(),
        state.mcp.clone(),
        state.event_bus.clone(),
        state.agent_model_client.clone(),
        state.agent_tool_registry.clone(),
        state.runtime_http_client.clone(),
    )
    .with_framework_host(state.agent_framework.clone())
    .with_assistant_work_context(state.assistants.clone())
    .with_directory_inspector(Some(std::sync::Arc::new(
        crate::adapters::outbound::application_directory::GitApplicationDirectoryInspector,
    )))
    .with_short_term_memory(state.short_term_memory.clone())
    .with_active_runs(state.active_runs.clone())
    .with_prompt_service(state.prompts.clone())
    .with_auto_model_route(auto_model_route, auto_model_router);
    match state
        .chats
        .send_message(
            &session_code,
            request,
            resolved_model,
            Some(auth.user().user_code.clone()),
            Some(auth.locale.clone()),
            Some(state.llm_runtime.clone()),
            agent_service,
        )
        .await
    {
        Ok(result) => {
            if result.session.source_code.as_deref().is_some_and(|source| source.starts_with("__assistant:"))
                && result.run.status == "COMPLETED"
            {
                if let Err(error) = state.assistants.resolve_awaiting_input(&session_code, &result.run.final_output) {
                    tracing::error!("PersonAI human response resolution: {error}");
                    return chat_send_error_response(ChatSendError::Agent(crate::application::agent::AgentInvocationError::RuntimeFailed(error.to_string())));
                }
            }
            Json(ApiResponse::success(result)).into_response()
        },
        Err(error) => chat_send_error_response(error),
    }
}

pub(crate) fn ensure_no_active_run(state: &AppState, session_code: &str) -> Result<(), Response> {
    match state.active_runs.snapshot(session_code) {
        Ok(None) => Ok(()),
        Ok(Some(active_run)) => Err(chat_send_error_response(ChatSendError::RunAlreadyActive {
            active_run_code: Some(active_run.run_code),
        })),
        Err(error) => Err(chat_send_error_response(ChatSendError::Agent(
            crate::application::agent::AgentInvocationError::RuntimeFailed(error.to_string()),
        ))),
    }
}

pub async fn steer_message(
    State(state): State<AppState>,
    _auth: RequestAuthContext,
    request: Result<Json<SteerChatMessageRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => return steer_chat_message_request_rejection_response(error),
    };
    match state.chats.steer_message(SteerChatMessageCommand {
        page_context: request.page_context,
        annotations: request.annotations,
        session_code: request.session_code,
        expected_run_code: request.expected_run_code,
        content: request.content,
        attachments: request.attachments,
        requested_skills: request.requested_skills,
    }) {
        Ok(result) => {
            Json(ApiResponse::success(ChatSteerResultResponse::from(result))).into_response()
        }
        Err(error) => chat_steer_error_response(error),
    }
}

pub async fn stream_message(
    State(state): State<AppState>,
    auth: RequestAuthContext,
    Path(session_code): Path<String>,
    Json(request): Json<StreamChatMessageRequest>,
) -> Response {
    let task_execution = request.task_execution;
    stream_message_command(state, auth, session_code, request.into(), task_execution).await
}

pub(crate) async fn stream_message_command(
    state: AppState,
    auth: RequestAuthContext,
    session_code: String,
    mut request: SendChatMessageCommand,
    task_execution: bool,
) -> Response {
    let stream_locale = SupportedLocale::from_raw(&auth.locale);
    let Ok(session) = state.chats.get_session(&session_code) else {
        return chat_error_response(ChatError::SessionNotFound);
    };
    if let Err(error) = state.chats.ensure_session_writable(&session_code) {
        return chat_error_response(error);
    }
    if let Err(error) = state.chats.ensure_session_directory_available(&session_code) {
        return chat_error_response(error);
    }
    if let Err(response) = ensure_no_active_run(&state, &session_code) {
        return response;
    }
    if let Err(response) = ensure_native_resume(&state, &session_code, &request.resume).await {
        return response;
    }
    let resolved_runtime = match resolve_chat_agent_runtime(&state, &session, &request) {
        Ok(runtime) => runtime,
        Err(error) => return chat_send_error_response(error),
    };
    let supports_steer = matches!(resolved_runtime.runtime_type, AgentRuntimeType::React);
    if let Err(error) = state.chats.validate_composer_input(&shared::effective_agent_code(&session, &request), request.content.as_deref().unwrap_or_default(), &request.requested_skills) {
        return chat_error_response(error);
    }
    let run_code = request.run_code.clone().unwrap_or_else(generate_agent_run_code);
    request.run_code = Some(run_code.clone());
    let prepared_model = match prepare_send_model(&state, &session, &request, &resolved_runtime) {
        Ok(model) => model,
        Err(error) => return chat_send_error_response(error),
    };
    let auto_model_route = prepared_model.auto_route.clone();
    let auto_model_router = prepared_model.auto_router.clone();
    let resolved_model = prepared_model.config;
    if let Err(error) = state.model_providers.record_requested_runtime_snapshot(
        Some(&run_code),
        &session.session_code,
        &session.workspace_code,
        &resolved_model,
    ) {
        return chat_send_error_response(ChatSendError::Agent(
            crate::application::agent::AgentInvocationError::RuntimeFailed(error.to_string()),
        ));
    }
    let resolved_model = crate::adapters::outbound::model::with_person_ai_conversation(
        resolved_model,
        &session.session_code,
        Some(&run_code),
    );

    if task_execution {
        let Some(task_code) = session.task_code.as_deref() else {
            return crate::adapters::inbound::http::tasks::task_error_response(
                crate::domain::task::TaskError::InvalidRequest,
            );
        };
        if session.source_type != crate::domain::chat::ChatSessionSourceType::Task
            || session.last_run_code.is_some()
            || request.task_code.as_deref() != Some(task_code)
        {
            return crate::adapters::inbound::http::tasks::task_error_response(
                crate::domain::task::TaskError::InvalidRequest,
            );
        }
        if let Err(error) = state
            .tasks
            .begin_task_execution(task_code, &session_code, &run_code)
        {
            return crate::adapters::inbound::http::tasks::task_error_response(error);
        }
    }

    let agui_stream = state.agent_presentation.clone();
    let _ = agui_stream.activate(&session_code);

    let (completion_sender, mut completion_receiver) =
        tokio::sync::mpsc::channel::<ChatSsePayload>(1);
    let chats = state.chats.clone();
    let session_code_for_task = session_code.clone();
    let run_code_for_task = run_code.clone();
    let task_execution_record = task_execution
        .then(|| session.task_code.clone().map(|task_code| (task_code, run_code.clone())))
        .flatten();
    let tasks = state.tasks.clone();
    let user_code = Some(auth.user().user_code.clone());
    let agent_service = AgentInvocationService::new(
        state.workspaces.clone(),
        state.apps.clone(),
        state.variables.clone(),
        state.skills.clone(),
        state.agent_management.clone(),
        state.agent_runs.clone(),
        state.mcp.clone(),
        state.event_bus.clone(),
        state.agent_model_client.clone(),
        state.agent_tool_registry.clone(),
        state.runtime_http_client.clone(),
    )
    .with_framework_host(state.agent_framework.clone())
    .with_assistant_work_context(state.assistants.clone())
    .with_directory_inspector(Some(std::sync::Arc::new(
        crate::adapters::outbound::application_directory::GitApplicationDirectoryInspector,
    )))
    .with_short_term_memory(state.short_term_memory.clone())
    .with_active_runs(state.active_runs.clone())
    .with_prompt_service(state.prompts.clone())
    .with_auto_model_route(auto_model_route, auto_model_router);
    let forwarding_gate = ChatStreamForwardingGate::new();
    let sink = Arc::new(|_event: ModelStreamEvent| {});

    let llm_runtime = state.llm_runtime.clone();
    let a2ui_action_id = request.a2ui_action_id.clone();
    let a2ui_host = state.agent_framework.clone();
    let assistants = state.assistants.clone();
    let run_task = AuthHolder::spawn(async move {
        let result = chats
            .send_message_stream(
                &session_code_for_task,
                request,
                resolved_model,
                user_code,
                Some(auth.locale.clone()),
                Some(llm_runtime),
                agent_service,
                sink,
            )
            .await;
        if let Some((task_code, execution_code)) = task_execution_record {
            let completion = match &result {
                Ok(result) => tasks.finish_task_execution(
                    &task_code,
                    &execution_code,
                    crate::domain::task::TaskExecutionStatus::Success,
                    Some(result.run.final_output.clone()),
                ),
                Err(_) => tasks.finish_task_execution(
                    &task_code,
                    &execution_code,
                    crate::domain::task::TaskExecutionStatus::Failed,
                    None,
                ),
            };
            if let Err(error) = completion {
                tracing::error!("task execution {task_code} result persistence failed: {error:?}");
            }
        }
        let completion = match result {
            Ok(result) => {
                if result.session.source_code.as_deref().is_some_and(|source| source.starts_with("__assistant:"))
                    && result.run.status == "COMPLETED"
                {
                    match assistants.resolve_awaiting_input(&session_code_for_task, &result.run.final_output) {
                        Ok(()) => ChatStreamCompletion::Success(result),
                        Err(error) => ChatStreamCompletion::Error { code: "101031004".into(), message: super::error_messages::message("101031004", &error.to_string(), stream_locale) },
                    }
                } else {
                    ChatStreamCompletion::Success(result)
                }
            },
            Err(error) => {
                let code = chat_send_error_code(&error).to_string();
                let message = chat_send_error_message(error, stream_locale);
                ChatStreamCompletion::Error { code, message }
            }
        };
        let mut done_payload = completion.payload(&session_code_for_task, &run_code_for_task, stream_locale);
        if let Some(id)=a2ui_action_id {
            done_payload.data["productCompletion"]=json!(true);
            if let Ok(framework)=a2ui_host.storage_framework().await {
                if let Ok(services)=framework.services() {
                    if let Some(service)=services.get::<eliteforge_agent_official_plugin_a2ui::A2uiService>() {
                        if let Err(error)=service.complete_action(&session_code_for_task,&id,done_payload.data.clone()).await {
                            done_payload.data["productCompletionError"]=json!({"code":"101031004","message":super::error_messages::message("101031004", "", stream_locale)});
                            let _ = error;
                        }
                    }
                }
            }
        }
        let _ = completion_sender.send(done_payload).await;
    });

    let stream_key = session_code.clone();
    let stream_run_code = run_code.clone();
    let stream_service = agui_stream.clone();
    let lifecycle_guard = ChatStreamLifecycleGuard::new(
        run_task,
        forwarding_gate,
        stream_service.clone(),
        stream_key.clone(),
        stream_run_code.clone(),
    );
    let stream = async_stream::stream! {
        let mut lifecycle_guard = lifecycle_guard;
        let mut seen = BTreeSet::<String>::new();
        let mut completion: Option<ChatSsePayload> = None;
        let mut interval = tokio::time::interval(Duration::from_millis(CHAT_STREAM_POLL_INTERVAL_MS));
        let mut native_terminals: std::collections::BTreeMap<String,crate::domain::agent::AgentAguiProtocolEvent> = std::collections::BTreeMap::new();
        let mut routed_child_runs = std::collections::BTreeSet::new();
        let mut continuation_seen = false;
        let mut presentation_incomplete = false;
        let mut replay_read_stalled = false;
        // Incremental reads clone only frames appended since the last tick.
        // None keeps the durable full-snapshot fallback.
        let mut replay_cursor: Option<u64> = None;
        loop {
            interval.tick().await;
            if completion.is_none() {
                let (received, unavailable)=poll_product_completion(&mut completion_receiver,&stream_key,&stream_run_code,stream_locale);
                completion=received;
                presentation_incomplete |= unavailable;
                if unavailable { stream_service.mark_incomplete(&stream_key,"product completion acknowledgment is unavailable"); }
            }
            let replay = if replay_read_stalled {
                None
            } else {
                let (events, cursor, stalled) = read_protocol_with_deadline(stream_service.clone(), stream_key.clone(), replay_cursor, Duration::from_secs(5)).await;
                if let Some(next) = cursor { replay_cursor = Some(next); }
                replay_read_stalled |= stalled;
                Some(events)
            };
            match replay {
                Some(Ok(events)) => {
                    // A child can persist before its parent's relation frame. Resolve
                    // native relations across the same snapshot before streaming it.
                    resolve_native_child_routes(&events, &stream_run_code, &mut routed_child_runs);
                    let has_continuation = events.iter().any(|event| event.data.pointer("/metadata/tron/parentProductRunCode").and_then(Value::as_str)==Some(stream_run_code.as_str()));
                    if has_continuation && !continuation_seen {
                        continuation_seen = true;
                        if let Some(terminal) = native_terminals.get(&stream_run_code) {
                            yield Ok::<Event, Infallible>(ChatSsePayload::agui(terminal.sse_event.clone(), terminal.data.clone(), stream_locale).event());
                        }
                    }
                    for event in events {
                        if !protocol_event_belongs_to_invocation_tree(&event.data, &stream_run_code, &routed_child_runs) { continue; }
                        let Some(key) = crate::application::agent::protocol_event_identity(&event) else {
                            presentation_incomplete = true;
                            continue;
                        };
                        if !seen.insert(key) { continue; }
                        if event.sse_event == "RUN_ERROR" && !native_protocol_execution_terminal(&event) {
                            presentation_incomplete = true;
                            yield Ok::<Event, Infallible>(ChatSsePayload::agui(event.sse_event, event.data, stream_locale).event());
                            continue;
                        }
                        if native_protocol_execution_terminal(&event) {
                            let product_run = event.data.get("runId").and_then(Value::as_str).unwrap_or(&stream_run_code).to_owned();
                            if continuation_seen || product_run != stream_run_code {
                                yield Ok::<Event, Infallible>(ChatSsePayload::agui(event.sse_event.clone(),event.data.clone(), stream_locale).event());
                            }
                            native_terminals.insert(product_run,event);
                            continue;
                        }
                        let data = run_started_with_supports_steer(&event.sse_event, event.data, supports_steer);
                        yield Ok::<Event, Infallible>(ChatSsePayload::agui(event.sse_event, data, stream_locale).event());
                    }
                }
                Some(Err(_)) if !presentation_incomplete => {
                    presentation_incomplete = true;
                    yield Ok::<Event, Infallible>(ChatSsePayload::agui("CUSTOM", json!({
                        "type": "CUSTOM", "name": "tron.presentation.incomplete", "value": {"code": "AGENT_AGUI_REPLAY_INCOMPLETE"},
                        "threadId": stream_key, "runId": stream_run_code,
                    }), stream_locale).event());
                }
                Some(Err(_)) | None => {}
            }
            if let Some(done) = completion.take() {
                // Native terminal identity/outcome are authoritative. Attach the product
                // result required by the existing HTTP contract after its transaction finishes.
                let actual_run = done.data.get("runId").and_then(Value::as_str).unwrap_or(&stream_run_code);
                let native_terminal = native_terminals.remove(actual_run).or_else(|| {
                    if done.data.get("code").and_then(Value::as_str)==Some("CHAT_STREAM_COMPLETION_UNAVAILABLE") {
                        native_terminals.into_values().max_by_key(|event|event.data.get("timestamp").and_then(Value::as_i64).unwrap_or(0))
                    } else { None }
                });
                let payload = product_completion_payload(native_terminal, done, presentation_incomplete, stream_locale);
                lifecycle_guard.complete();
                yield Ok::<Event, Infallible>(payload.event());
                break;
            }
        }

    };

    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub(crate) fn run_started_with_supports_steer(
    sse_event: &str,
    mut data: Value,
    supports_steer: bool,
) -> Value {
    if sse_event == "RUN_STARTED"
        && let Some(object) = data.as_object_mut()
    {
        object.insert("supportsSteer".to_string(), Value::Bool(supports_steer));
    }
    data
}

/// Product result decoration, not a protocol lifecycle generator. When presentation
/// terminated early, the independently completed execution still reports its real outcome.
// Output failures do not replace the last authoritative native execution outcome.
fn native_protocol_execution_terminal(
    event: &crate::domain::agent::AgentAguiProtocolEvent,
) -> bool {
    match event.sse_event.as_str() {
        "RUN_FINISHED" => matches!(
            event.data.pointer("/outcome/type").and_then(Value::as_str),
            Some("success" | "interrupt")
        ),
        "RUN_ERROR" => !event
            .data
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| {
                code.starts_with("AGUI_") || code == "CHAT_STREAM_COMPLETION_UNAVAILABLE"
            }),
        _ => false,
    }
}

fn product_completion_payload(
    native: Option<crate::domain::agent::AgentAguiProtocolEvent>,
    mut completion: ChatSsePayload,
    incomplete: bool,
    locale: SupportedLocale,
) -> ChatSsePayload {
    if let Some(object) = completion.data.as_object_mut() {
        object.insert("productCompletion".into(), json!(true));
    }
    if let Some(native) = native {
        if completion.data.get("code").and_then(Value::as_str)
            == Some("CHAT_STREAM_COMPLETION_UNAVAILABLE")
        {
            let mut data = native.data;
            if let Some(object) = data.as_object_mut() {
                object.insert("productCompletion".into(), json!(true));
                object.insert("presentationIncomplete".into(), json!(true));
                object.insert("productCompletionError".into(), json!({"code":"CHAT_STREAM_COMPLETION_UNAVAILABLE","message":t!("error.chat.product_completion_unavailable", locale = locale)}));
            }
            return ChatSsePayload::agui(native.sse_event, data, locale);
        }
        let output_failed = native
            .data
            .get("code")
            .and_then(Value::as_str)
            .is_some_and(|code| code.starts_with("AGUI_"));
        if !output_failed
            && (completion.event_name == native.sse_event || native.sse_event == "RUN_ERROR")
        {
            let mut data = native.data;
            if let Some(object) = data.as_object_mut() {
                if let Some(result) = completion.data.get("result") {
                    object.insert("result".into(), result.clone());
                }
                // Native failures carry the same diagnostic as persisted history.
                // Product completion only settles the HTTP request; its localized
                // generic error must not erase the native failure code/message.
                // Enrich interrupt metadata with product question IDs, retaining native
                // interaction records in the result rather than reconstructing execution.
                if native.sse_event == "RUN_FINISHED" {
                    if let Some(outcome) = completion.data.get("outcome").filter(|outcome| {
                        outcome.get("type").and_then(Value::as_str) == Some("interrupt")
                    }) {
                        object.insert("outcome".into(), outcome.clone());
                    }
                }
                object.insert("productCompletion".into(), json!(true));
                object.insert("presentationIncomplete".into(), json!(incomplete));
            }
            return ChatSsePayload::agui(native.sse_event, data, locale);
        }
        if let Some(object) = completion.data.as_object_mut() {
            object.insert("metadata".into(), json!({"nativeTerminal": native.data}));
        }
    }
    if let Some(object) = completion.data.as_object_mut() {
        object.insert("presentationIncomplete".into(), json!(incomplete));
    }
    completion
}

async fn ensure_native_resume(
    state: &AppState,
    session_code: &str,
    resume: &[crate::application::chat::ChatResumeEntry],
) -> Result<(), Response> {
    use crate::domain::agent::AgentError;
    match state
        .agent_framework
        .validate_native_resume(session_code, resume)
        .await
    {
        Ok(_) => Ok(()),
        Err(AgentError::AwaitingUserInput) => Err(chat_error_response(ChatError::UserInputPending)),
        Err(AgentError::InvalidConfiguration(_)) => Err(chat_error_response(
            ChatError::UserInputAnswerInvalid("invalid native interaction response".into()),
        )),
        Err(AgentError::SessionBusy(session)) => {
            Err(chat_send_error_response(ChatSendError::Agent(
                crate::application::agent::AgentInvocationError::SessionBusy(session),
            )))
        }
        Err(AgentError::Cancelled) => Err(chat_send_error_response(ChatSendError::Agent(
            crate::application::agent::AgentInvocationError::RunStopped,
        ))),
        Err(_) => Err(chat_send_error_response(ChatSendError::Agent(
            crate::application::agent::AgentInvocationError::RuntimeFailed(
                "native interaction validation failed".into(),
            ),
        ))),
    }
}

fn completion_channel_closed_payload(
    session_code: &str,
    run_code: &str,
    locale: SupportedLocale,
) -> ChatSsePayload {
    let message = t!("error.chat.product_completion_unavailable", locale = locale).into_owned();
    ChatSsePayload::agui(
        "RUN_ERROR",
        json!({
            "type":"RUN_ERROR","threadId":session_code,"runId":run_code,
            "code":"CHAT_STREAM_COMPLETION_UNAVAILABLE","message":message,
            "presentationIncomplete":true,
            "productCompletionError":{"code":"CHAT_STREAM_COMPLETION_UNAVAILABLE","message":message},
        }),
        locale,
    )
}



fn poll_product_completion(
    receiver: &mut tokio::sync::mpsc::Receiver<ChatSsePayload>,
    session_code: &str,
    run_code: &str,
    locale: SupportedLocale,
) -> (Option<ChatSsePayload>, bool) {
    match receiver.try_recv() {
        Ok(completion) => (Some(completion), false),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => (None, false),
        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => (
            Some(completion_channel_closed_payload(session_code, run_code, locale)),
            true,
        ),
    }
}

/// One blocked SQLite read may outlive its acknowledgment deadline. Never start
/// another reader behind it; the caller continues polling native execution completion.
async fn read_protocol_with_deadline(
    service: crate::application::agent::AgentAguiStreamService,
    session: String,
    cursor: Option<u64>,
    deadline: Duration,
) -> (
    Result<
        Vec<crate::domain::agent::AgentAguiProtocolEvent>,
        crate::application::agent::AgentAguiStreamError,
    >,
    Option<u64>,
    bool,
) {
    let reader = service.clone();
    let stream_key = session.clone();
    match tokio::time::timeout(
        deadline,
        tokio::task::spawn_blocking(move || reader.protocol_events_since(&stream_key, cursor)),
    )
    .await
    {
        Ok(Ok(Ok((events, cursor)))) => (Ok(events), cursor, false),
        Ok(Ok(Err(error))) => (Err(error), None, false),
        _ => {
            service.mark_incomplete(
                &session,
                "presentation replay read acknowledgment unavailable",
            );
            (
                Err(
                    crate::application::agent::AgentAguiStreamError::ReplayReadFailed(
                        "presentation replay read acknowledgment unavailable".into(),
                    ),
                ),
                None,
                true,
            )
        }
    }
}

fn protocol_event_belongs_to_invocation_tree(
    data: &Value,
    product_run: &str,
    children: &std::collections::BTreeSet<String>,
) -> bool {
    protocol_event_belongs_to_run(data, product_run)
        || data
            .pointer("/metadata/tron/parentProductRunCode")
            .and_then(Value::as_str)
            == Some(product_run)
        || data
            .pointer("/metadata/framework/runId")
            .and_then(Value::as_str)
            .is_some_and(|run| children.contains(run))
}
fn resolve_native_child_routes(
    events: &[crate::domain::agent::AgentAguiProtocolEvent],
    product_run: &str,
    children: &mut std::collections::BTreeSet<String>,
) {
    loop {
        let before = children.len();
        for event in events {
            if protocol_event_belongs_to_invocation_tree(&event.data, product_run, children)
                && matches!(
                    event.sse_event.as_str(),
                    "SUBAGENT_STARTED" | "SUBAGENT_FINISHED" | "SUBAGENT_ERROR"
                )
                && let Some(child) = event.data.get("subagentRunId").and_then(Value::as_str)
            {
                children.insert(child.to_owned());
            }
        }
        if children.len() == before {
            break;
        }
    }
}
