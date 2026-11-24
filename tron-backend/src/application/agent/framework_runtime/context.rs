//! Tron maps authorized product prompt sources and attachments; Framework memory
//! owns assembly and compaction before the final controlled model admission.
use super::model::HostExecution;
use async_trait::async_trait;
use eliteforge_agent_core as fw;

pub(super) struct TronContext;
#[async_trait]
impl fw::ContextPreparationPolicy for TronContext {
    async fn prepare(
        &self,
        input: &fw::ContextAssemblyInput<'_>,
        reductions: usize,
    ) -> Result<fw::ContextPreparation, fw::AgentError> {
        let host = HostExecution::from_extensions(&input.run.extensions)?;
        let mut state = host.state.lock().await;
        super::model::synchronize_selected_model(
            &mut state,
            input.model,
            &input.run.extensions,
            input.options,
        )?;
        if state
            .context_auxiliary_calls
            .as_ref()
            .is_none_or(|(step, _)| *step != input.step)
        {
            state.context_auxiliary_calls = Some((
                input.step,
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ));
        }
        if reductions == 0 && input.attempt > 1 {
            if let Some(message) = state.last_model_failure.take() {
                state.error = None;
                match host.harness.framework_model_error(&state.context, &message) {
                    fw::ModelError::ContextOverflow(_) => {
                        state.context_force_compression = true;
                        // Recovery goes through the same configured global coordinator.
                        // Keep strategies and the additional auxiliary ceiling remain binding.
                    }
                    // ModelInvoker already admitted this recovery. Structured provider
                    // authentication/rate-limit errors must not veto its next dispatch.
                    _ => {}
                }
            }
        }
        state.context.messages = input
            .history
            .messages
            .iter()
            .map(super::messages::decode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(fw::AgentError::InvalidConfiguration)?
            .into();
        // Instructions belong to the current authorized host scope. Persisted
        // conversation history cannot freeze an earlier workspace/agent prefix.
        let old_prefix_len = state
            .context
            .messages
            .iter()
            .take_while(|message| super::messages::is_prompt_prefix(message))
            .count();
        let prefix = state.trusted_prefix.clone();
        std::sync::Arc::make_mut(&mut state.context.messages).splice(..old_prefix_len, prefix);
        state.context.tools = input
            .tools
            .iter()
            .map(|descriptor| {
                super::tools::map_authorized_descriptor(
                    &state.context,
                    descriptor,
                    &input.run.extensions,
                )
            })
            .collect::<Result<Vec<_>, fw::AgentError>>()?;
        let super::model::HostRunState { context, turn, .. } = &mut *state;
        let projection = match host.harness.prepare_framework_context(context, turn).await {
            Ok(projection) => projection,
            Err(error) => {
                let message = error.to_string();
                state.error = Some(error);
                return Err(fw::AgentError::Run(message));
            }
        };
        let context = context.clone();
        if state
            .context_auxiliary_calls
            .as_ref()
            .is_none_or(|(step, _)| *step != input.step)
        {
            state.context_auxiliary_calls = Some((
                input.step,
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            ));
        }
        let auxiliary_calls = state.context_auxiliary_calls.as_ref().unwrap().1.clone();
        let manual = input
            .run
            .extensions
            .get::<fw::ContextCompactionRequest>()
            .is_some();
        let force = state.context_force_compression || manual;
        let onboarding = state.onboarding;
        let previous_messages = state
            .prepared
            .as_ref()
            .map(|prepared| prepared.messages.clone());
        // No host mutex may be held while a reducer invokes an auxiliary model.
        drop(state);
        let assembled = super::context_providers::assemble(
            input,
            host.clone(),
            &context,
            &projection,
            force,
            auxiliary_calls,
            onboarding,
        )
        .await;
        let window = match assembled {
            Ok(window) => window,
            Err(error) => match error.primary() {
                fw::AgentError::Context(fw::ContextError::RefreshRequired { source_revision }) => {
                    return Ok(fw::ContextPreparation::Refresh {
                        source_revision: source_revision.clone(),
                    });
                }
                _ => return Err(identify_failed_budget_block(error)),
            },
        };
        let mut projection = projection;
        projection.messages = window
            .messages
            .iter()
            .map(super::messages::decode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(fw::AgentError::InvalidConfiguration)?
            .into();
        if force && !manual && previous_messages.as_ref() == Some(&projection.messages) {
            return Err(fw::ModelError::ContextOverflow(
                "configured context strategies reproduced the failed model input".into(),
            )
            .into());
        }
        projection.message_token_estimate = window
            .report
            .estimated_input_tokens
            .saturating_sub(window.report.protocol_tokens)
            .min(u32::MAX as usize) as u32;
        projection.estimated_tokens =
            window.report.estimated_input_tokens.min(u32::MAX as usize) as u32;
        projection.over_budget = false;
        projection.over_budget_reason = None;
        projection.compaction_required = false;
        super::context_projection::update(
            &mut projection,
            &window,
            input,
            &host.harness.framework_context_config(),
        )?;
        let snapshot_sequence = host
            .harness
            .framework_record_context(&context, &projection, &window.report)
            .map_err(|error| fw::AgentError::Run(error.to_string()))?;
        // Detail does not complete the lifecycle: Framework emits completion
        // only after admitting this context. Ordinary assemblies are ignored.
        input.run.emit_output(fw::OutputEvent::Custom {
            name: "agent.memory.compaction.details".into(),
            value: serde_json::json!({
                "blocks": crate::application::short_term_memory::compaction_blocks(&window.report),
            }),
        });
        // Inspector metadata follows the native output/replay channel. Do not
        // duplicate full history over SSE or fit a report into bus attribute caps.
        input.run.emit_output(fw::OutputEvent::Custom {
            name: "tron.short_term_memory.context.assembled".into(),
            value: serde_json::json!({
                "contextReport": window.report,
                "budgetBreakdown": projection.budget_breakdown,
                "snapshotSequence": snapshot_sequence.map(|value| value.to_string()),
            }),
        });
        let mut state = host.state.lock().await;
        state.context.messages = projection.messages.clone();
        state.context_force_compression = false;
        state.prepared = Some(projection);
        Ok(fw::ContextPreparation::Ready(window))
    }
}

fn identify_failed_budget_block(error: fw::AgentError) -> fw::AgentError {
    let fw::AgentError::ContextAssembly { report, source } = error else {
        return error;
    };
    if matches!(
        source.primary(),
        fw::AgentError::Context(fw::ContextError::BudgetExceeded(_))
    ) {
        if let Some(block) = report
            .blocks
            .iter()
            .find(|block| matches!(block.outcome, fw::ContextBlockOutcome::Failed { .. }))
        {
            tracing::warn!(block_id = %block.id, error = %source, "context block exceeded its budget");
            let source = Box::new(fw::AgentError::Context(fw::ContextError::BudgetExceeded(
                format!("tron_context_block:{}", block.id),
            )));
            return fw::AgentError::ContextAssembly { report, source };
        }
    }
    fw::AgentError::ContextAssembly { report, source }
}
