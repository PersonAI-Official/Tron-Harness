//! Product ACL and registration selection over original SDK tool bindings.
//! Admission changes the catalog, never the executor or resource owner.
use super::super::model::HostExecution;
use crate::application::agent::tool_framework::FrameworkToolRegistryExt;
use crate::domain::agent::{AgentRunContext, EffectiveTool, ToolRegistry, ToolRequest};
use async_trait::async_trait;
use eliteforge_agent_core as fw;
use tron_i18n::t;

pub(super) fn has_product_implementation(
    registry: &ToolRegistry,
    context: &AgentRunContext,
    name: &str,
) -> bool {
    registry
        .resolve_allowed(
            &ToolRequest {
                tool_name: name.into(),
                input: serde_json::Value::Null,
                invocation_id: None,
            },
            context,
        )
        .is_ok()
}

fn framework_artifact_binding(
    extensions: &fw::Extensions,
    descriptor: &fw::ToolDescriptor,
) -> bool {
    extensions
        .get::<tron_harness_capability::ArtifactsRunOptions>()
        .is_some()
        && matches!(
            (descriptor.id.as_str(), descriptor.call_name.as_str()),
            ("artifacts.present", "artifacts_present")
        )
}

fn framework_a2ui_binding(extensions: &fw::Extensions, descriptor: &fw::ToolDescriptor) -> bool {
    eliteforge_agent_official_plugin_a2ui::negotiated(extensions)
        && (eliteforge_agent_official_plugin_a2ui::is_a2ui_tool(descriptor)
            || native_skill_descriptor(descriptor))
}

pub(crate) fn map_authorized_descriptor(
    context: &AgentRunContext,
    descriptor: &fw::ToolDescriptor,
    extensions: &fw::Extensions,
) -> Result<EffectiveTool, fw::AgentError> {
    // The stable gateway trio is host infrastructure, not a business tool:
    // it is always visible and carries no product ACL entry.
    if eliteforge_agent_core::is_gateway_descriptor(descriptor) {
        return Ok(EffectiveTool {
            tool_name: descriptor.call_name.clone(),
            domain_code: None,
            description: descriptor.description.clone(),
            input_schema: descriptor.input_schema.clone(),
            skill_name: None,
            required_variable_keys: vec![],
        });
    }
    if framework_a2ui_binding(extensions, descriptor)
        || framework_artifact_binding(extensions, descriptor)
        || native_skill_descriptor(descriptor)
    {
        return Ok(EffectiveTool {
            tool_name: descriptor.call_name.clone(),
            domain_code: Some(
                if native_skill_descriptor(descriptor) {
                    "skill"
                } else if framework_artifact_binding(extensions, descriptor) {
                    "artifacts"
                } else {
                    "a2ui"
                }
                .into(),
            ),
            description: descriptor.description.clone(),
            input_schema: descriptor.input_schema.clone(),
            skill_name: if native_skill_descriptor(descriptor) {
                None
            } else {
                Some(eliteforge_agent_official_plugin_a2ui::A2UI_SKILL_NAME.into())
            },
            required_variable_keys: vec![],
        });
    }
    if !context.is_tool_authorized(&descriptor.call_name) {
        return Err(fw::AgentError::ToolSetup(t!(
            "agent_runtime.tool.authority.unauthorized_tool",
            locale = context.locale.as_str(),
            tool = descriptor.call_name
        )));
    }
    let mut tool = context
        .authorized_tools()
        .iter()
        .find(|tool| tool.tool_name == descriptor.call_name)
        .cloned()
        .ok_or_else(|| {
            fw::AgentError::ToolSetup(t!(
                "agent_runtime.tool.authority.missing_acl",
                locale = context.locale.as_str(),
                tool = descriptor.call_name
            ))
        })?;
    tool.description = descriptor.description.clone();
    tool.input_schema = descriptor.input_schema.clone();
    Ok(tool)
}

fn overridden_native_skill(
    host: &HostExecution,
    state: &super::super::model::HostRunState,
    descriptor: &fw::ToolDescriptor,
) -> bool {
    if !native_skill_descriptor(descriptor) {
        return false;
    }
    let registry = host.harness.framework_registry();
    has_product_implementation(&registry, &state.context, &descriptor.call_name)
        || registry.has_framework_binding(&state.context.run_code, &descriptor.call_name)
        || state
            .tool_catalog
            .get(&descriptor.call_name)
            .is_some_and(|id| id != &descriptor.id)
}

fn native_skill_descriptor(descriptor: &fw::ToolDescriptor) -> bool {
    matches!(
        (descriptor.id.as_str(), descriptor.call_name.as_str()),
        ("skills.load", "skill_load")
            | ("skills.read_resource", "skill_read_resource")
            | ("skills.diagnostics", "skill_diagnostics")
    )
}

pub(crate) struct ToolCatalogPolicy {
    tools_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    management: Option<crate::application::agent::AgentManagementService>,
    skills: Option<crate::application::skill::SkillRegistryService>,
}
impl ToolCatalogPolicy {
    pub(crate) fn shared(
        tools_enabled: std::sync::Arc<std::sync::atomic::AtomicBool>,
        management: Option<crate::application::agent::AgentManagementService>,
        skills: Option<crate::application::skill::SkillRegistryService>,
    ) -> Self {
        Self {
            tools_enabled,
            management,
            skills,
        }
    }
}
#[async_trait]
impl fw::ToolPolicy for ToolCatalogPolicy {
    async fn admit_binding(
        &self,
        binding: &fw::ToolBinding,
        run: &fw::RunContext,
    ) -> Result<fw::ToolBindingAdmission, fw::AgentError> {
        if !self
            .tools_enabled
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Ok(fw::ToolBindingAdmission::Exclude);
        }
        // The stable gateway trio is host infrastructure: assignment modes and
        // the persisted platform tool table never scope or revoke it. Inner
        // tools are still fully checked below after envelope unwrapping.
        if eliteforge_agent_core::is_gateway_descriptor(&binding.descriptor) {
            return Ok(fw::ToolBindingAdmission::Allow);
        }
        let caller_scope = run.extensions.get::<fw::CallerInteractionScope>().is_some();
        if (binding.descriptor.id == "agent.ask_caller" && !caller_scope)
            || (caller_scope
                && matches!(
                    binding.descriptor.call_name.as_str(),
                    "chat_ask_user" | "ask_user" | "a2ui_request_input"
                ))
        {
            return Ok(fw::ToolBindingAdmission::Exclude);
        }
        let host = HostExecution::from_extensions(&run.extensions)?;
        let mut state = host.state.lock().await;
        let name = &binding.descriptor.call_name;
        if !crate::domain::computer_use::allows(state.context.agent_code.as_deref().unwrap_or(""))
            && crate::domain::computer_use::is_tool(
                &binding.descriptor.id,
                name,
                binding.domain_code.as_deref(),
                binding.source.pack_id.as_deref(),
            )
        {
            state.product_tool_authority.remove(name);
            state
                .context
                .effective_tools
                .retain(|tool| &tool.tool_name != name);
            state.context.tools.retain(|tool| &tool.tool_name != name);
            return Ok(fw::ToolBindingAdmission::Exclude);
        }
        if self.management.is_some() {
            state.pending_tool_bindings.push(binding.clone());
        }
        if let (Some(management), Some(selection)) =
            (&self.management, state.context.tool_assignment.as_ref())
        {
            let record = management
                .describe_tool_binding(binding)
                .map_err(fw::AgentError::ToolSetup)?;
            if state.context.is_owning_assistant()
                && selection.mode != crate::domain::agent::ToolAssignmentMode::AllPlatformTools
                && crate::application::agent::tool_catalog::assistant_workspace_tool(&record)
            {
                state.product_tool_authority.remove(name);
                return Ok(fw::ToolBindingAdmission::Exclude);
            }
            let selected = selection.selects(&record)
                || (state.context.is_owning_assistant()
                    && record
                        .category
                        .as_ref()
                        .is_some_and(|category| category.code == "tron.assistant")
                    && state.product_tool_authority.contains(name))
                || (caller_scope && binding.descriptor.id == "agent.ask_caller");
            let restrict_selection = matches!(
                selection.mode,
                crate::domain::agent::ToolAssignmentMode::AllReadOnlyTools
                    | crate::domain::agent::ToolAssignmentMode::SelectedCategories
            );
            if restrict_selection && !selected {
                state.product_tool_authority.remove(name);
                return Ok(fw::ToolBindingAdmission::Exclude);
            }
            if !management
                .platform_tool_record_allowed(&record)
                .map_err(|_| {
                    fw::AgentError::ToolSetup("Could not read platform tool permission".into())
                })?
            {
                return Ok(fw::ToolBindingAdmission::Exclude);
            }
            // Only leased native bindings can extend a dynamic rule; request schemas
            // and the persisted discovery cache never create executable authority.
            if selected
                && !native_skill_descriptor(&binding.descriptor)
                && !state.product_tool_authority.contains(name)
            {
                let tool = crate::application::agent::tool_record_to_effective_tool(record);
                state.product_tool_authority.insert(name.clone());
                state.context.effective_tools.push(tool.clone());
                state.context.tools.push(tool);
            }
        }
        if (!state.product_tool_authority.contains(name)
            && !framework_a2ui_binding(&run.extensions, &binding.descriptor)
            && !framework_artifact_binding(&run.extensions, &binding.descriptor)
            && !(self.skills.is_some() && native_skill_descriptor(&binding.descriptor)))
            || overridden_native_skill(&host, &state, &binding.descriptor)
        {
            return Ok(fw::ToolBindingAdmission::Exclude);
        }
        if (self.skills.is_some() && native_skill_descriptor(&binding.descriptor))
            || framework_a2ui_binding(&run.extensions, &binding.descriptor)
            || framework_artifact_binding(&run.extensions, &binding.descriptor)
        {
            return Ok(fw::ToolBindingAdmission::Allow);
        }
        Ok(
            if state
                .context
                .tools
                .iter()
                .any(|tool| &tool.tool_name == name)
            {
                fw::ToolBindingAdmission::Allow
            } else {
                fw::ToolBindingAdmission::Hidden
            },
        )
    }

    async fn catalog_ready(
        &self,
        bindings: &[fw::ToolBinding],
        run: &fw::RunContext,
    ) -> Result<(), fw::AgentError> {
        let host = HostExecution::from_extensions(&run.extensions)?;
        let mut state = host.state.lock().await;
        if let Some(management) = &self.management {
            management
                .register_tool_bindings(&state.pending_tool_bindings)
                .map_err(fw::AgentError::ToolSetup)?;
            state.pending_tool_bindings.clear();
        }
        // Legacy contexts use `tools` for both ACL and model exposure until an
        // effective catalog is supplied. Materialize that original ACL before
        // context/model preparation narrows `tools` to actual native bindings.
        // An authorized but unregistered tool remains missing, not unauthorized;
        // copying its DTO does not admit an executor or expose its schema.
        if state.context.effective_tools.is_empty() {
            state.context.effective_tools = state.context.tools.clone();
        }
        if state.context.tool_assignment.as_ref().is_some_and(|s| {
            matches!(
                s.mode,
                crate::domain::agent::ToolAssignmentMode::AllReadOnlyTools
                    | crate::domain::agent::ToolAssignmentMode::SelectedCategories
            )
        }) {
            let admitted = bindings
                .iter()
                .map(|b| b.descriptor.call_name.as_str())
                .collect::<std::collections::HashSet<_>>();
            state
                .context
                .effective_tools
                .retain(|t| admitted.contains(t.tool_name.as_str()));
            state
                .context
                .tools
                .retain(|t| admitted.contains(t.tool_name.as_str()));
            state
                .product_tool_authority
                .retain(|name| admitted.contains(name.as_str()));
        }
        // Remove globally denied tools from discovery while preserving the
        // existing distinction between authorized and registered executors.
        if let Some(management) = &self.management {
            let mut denied = std::collections::HashSet::new();
            for tool in state.context.authorized_tools() {
                if !management
                    .platform_tool_allowed(&tool.tool_name)
                    .map_err(|_| {
                        fw::AgentError::ToolSetup("Could not read platform tool permission".into())
                    })?
                {
                    denied.insert(tool.tool_name.clone());
                }
            }
            state
                .context
                .effective_tools
                .retain(|t| !denied.contains(&t.tool_name));
            state
                .product_tool_authority
                .retain(|name| !denied.contains(name));
            state
                .context
                .tools
                .retain(|t| !denied.contains(&t.tool_name));
        }
        // The initial inventory is assembled before provider leases. Replace it
        // with the same admitted, exposed bindings used by gateway discovery.
        if let Some(management) = &self.management {
            let admitted = bindings
                .iter()
                .filter(|b| b.exposed)
                .map(|b| b.descriptor.call_name.as_str())
                .collect::<std::collections::HashSet<_>>();
            let tools = state
                .context
                .authorized_tools()
                .iter()
                .filter(|tool| admitted.contains(tool.tool_name.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            let records = crate::application::agent::invocation::authorized_domain_records(
                management,
                &tools,
                &state.context.locale,
            );
            let domains = crate::application::agent::BusinessToolCatalog::domains_for_records(
                &records,
                &state.context.locale,
            );
            for message in &mut state.trusted_prefix {
                if message.parts.iter().any(|part| matches!(part,
                    crate::domain::agent::AgentMessagePart::Context { kind, text, .. }
                    if kind == "tool_domains" && serde_json::from_str::<Vec<crate::domain::agent::ToolDomainRecord>>(text).is_ok())) {
                    *message = crate::application::agent::invocation::visible_domain_context(&domains);
                }
            }
        }
        state.tool_catalog = bindings
            .iter()
            .map(|binding| {
                (
                    binding.descriptor.call_name.clone(),
                    binding.descriptor.id.clone(),
                )
            })
            .collect();
        if state.onboarding {
            state.onboarding_tools =
                super::super::onboarding::tool_definitions(bindings, &state.context);
        }
        // A new invocation or resume has a fresh saved-policy snapshot. Tell
        // the model exactly what changed instead of leaving stale tool names in
        // prior conversation turns as its only evidence. In-flight tools keep
        // the assignment they were admitted under.
        if let Some(notice) = host
            .harness
            .framework_tool_catalog_update(&state.context, bindings)
            .await
            .map_err(|error| fw::AgentError::ToolSetup(error.to_string()))?
        {
            state
                .trusted_prefix
                .push(crate::domain::agent::AgentMessage::new(
                    "user",
                    vec![crate::domain::agent::AgentMessagePart::Context {
                        kind: "tool_domains".into(),
                        text: notice.to_string(),
                        metadata: None,
                    }],
                ));
        }
        Ok(())
    }

    async fn evaluate(
        &self,
        invocation: &fw::ToolInvocationContext<'_>,
    ) -> Result<fw::ToolPolicyDecision, fw::AgentError> {
        if !self
            .tools_enabled
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Ok(fw::ToolPolicyDecision::Deny(
                t!(
                    "agent_runtime.tool.authority.extension_disabled",
                    locale = tron_harness_capability::run_locale(invocation.run)
                )
                .into_owned(),
            ));
        }
        // The stable gateway trio is host infrastructure: executing discovery
        // (or the defensive execute executor) never requires product tool
        // authority, mirroring the admission-side exemption.
        if eliteforge_agent_core::is_gateway_descriptor(invocation.descriptor) {
            return Ok(fw::ToolPolicyDecision::Allow);
        }
        let host = HostExecution::from_extensions(&invocation.run.extensions)?;
        let state = host.state.lock().await;
        if !crate::domain::computer_use::allows(state.context.agent_code.as_deref().unwrap_or(""))
            && crate::domain::computer_use::is_tool(
                &invocation.descriptor.id,
                &invocation.descriptor.call_name,
                None,
                None,
            )
        {
            return Ok(fw::ToolPolicyDecision::Deny(
                t!(
                    "agent_runtime.tool.authority.computer_use_restricted",
                    locale = tron_harness_capability::run_locale(invocation.run)
                )
                .into_owned(),
            ));
        }
        if let Some(management) = &self.management {
            if !management
                .platform_tool_allowed(&invocation.descriptor.call_name)
                .map_err(|_| {
                    fw::AgentError::Run("Could not read platform tool permission".into())
                })?
            {
                return Ok(fw::ToolPolicyDecision::Deny(
                    tron_harness_capability::permission_denied_message(
                        tron_harness_capability::run_locale(invocation.run),
                    )
                    .into(),
                ));
            }
        }
        if self.skills.is_some() && native_skill_descriptor(invocation.descriptor) {
            if let Some(skills) = self
                .skills
                .as_ref()
                .filter(|_| invocation.descriptor.call_name != "skill_diagnostics")
            {
                let name = invocation
                    .call
                    .arguments
                    .get("name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("");
                let agent = state.context.agent_code.as_deref().unwrap_or("main-agent");
                if !skills
                    .effective_agent_skill_names(agent)
                    .map_err(|e| fw::AgentError::Run(e.to_string()))?
                    .contains(name)
                {
                    return Ok(fw::ToolPolicyDecision::Deny(
                        t!(
                            "agent_runtime.tool.authority.skill_unavailable",
                            locale = tron_harness_capability::run_locale(invocation.run)
                        )
                        .into_owned(),
                    ));
                }
            }
            return Ok(fw::ToolPolicyDecision::Allow);
        }
        if (!state
            .product_tool_authority
            .contains(&invocation.descriptor.call_name)
            && !framework_artifact_binding(&invocation.run.extensions, invocation.descriptor)
            && !(framework_a2ui_binding(&invocation.run.extensions, invocation.descriptor)
                && (!native_skill_descriptor(invocation.descriptor)
                    || invocation
                        .call
                        .arguments
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        == Some(eliteforge_agent_official_plugin_a2ui::A2UI_SKILL_NAME))))
            || overridden_native_skill(&host, &state, invocation.descriptor)
        {
            return Ok(fw::ToolPolicyDecision::Deny(
                t!(
                    "agent_runtime.tool.authority.changed_after_admission",
                    locale = tron_harness_capability::run_locale(invocation.run)
                )
                .into_owned(),
            ));
        }
        Ok(fw::ToolPolicyDecision::Allow)
    }
}

/// Hosts without management (isolated tests) preserve the previous run behavior.
pub(crate) struct PlatformPermissionSource(
    pub Option<crate::application::agent::AgentManagementService>,
);
impl tron_harness_capability::ToolPermissionSource for PlatformPermissionSource {
    fn setting(
        &self,
        descriptor: &fw::ToolDescriptor,
    ) -> Result<tron_harness_capability::ToolPermissionSetting, String> {
        match &self.0 {
            Some(source) => {
                tron_harness_capability::ToolPermissionSource::setting(source, descriptor)
            }
            None => Ok(tron_harness_capability::ToolPermissionSetting::default_for(
                &descriptor.id,
            )),
        }
    }
    fn operation(&self, descriptor: &fw::ToolDescriptor) -> Option<bool> {
        self.0
            .as_ref()
            .and_then(|s| tron_harness_capability::ToolPermissionSource::operation(s, descriptor))
    }
}
