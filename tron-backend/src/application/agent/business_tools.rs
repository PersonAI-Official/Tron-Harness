mod ask_user;
mod coding_tools;
mod file_access;
mod graph_tools;
mod memory_tools;
mod rag_tools;
pub use graph_tools::rag_graph_tools;
mod knowledge_quality_tools;
pub use knowledge_quality_tools::knowledge_quality_tools;
pub use rag_tools::rag_search_tool;
mod platform_tools;
mod productivity_tools;
mod short_term_memory_tools;

use crate::domain::agent::{EffectiveTool, Tool, ToolError, ToolRegistry};
use crate::{
    application::{
        app::AppService,
        automation::AutomationService,
        mcp::McpService,
        memory::MemoryService,
        prompt::PromptService,
        rag::{
            RagDiagnosticsRepository, RagEmbeddingRuntime, RagIngestRepository,
            RagSearchRepository, RagSearchRuntime,
        },
        resource::ResourceService,
        short_term_memory::ShortTermMemoryService,
        skill::SkillRegistryService,
        task::TaskService,
        variable::VariableService,
        workspace::WorkspaceService,
    },
    domain::agent::ToolCatalogRecord,
};

pub use ask_user::{CHAT_ASK_USER_TOOL, ask_user_definition};

pub fn tool_record_to_effective_tool(record: ToolCatalogRecord) -> EffectiveTool {
    EffectiveTool {
        tool_name: record.tool_name,
        domain_code: Some(record.domain_code),
        description: record.description,
        input_schema: record.input_schema,
        skill_name: None,
        required_variable_keys: Vec::new(),
    }
}

pub fn register_default_agent_tools<R, E>(
    registry: &ToolRegistry,
    workspaces: WorkspaceService,
    apps: AppService,
    automations: AutomationService,
    tasks: TaskService,
    skills: SkillRegistryService,
    variables: VariableService,
    resources: ResourceService,
    prompts: PromptService,
    mcp: McpService,
    memory: MemoryService<R, E>,
    short_term_memory: ShortTermMemoryService,
) -> Result<(), ToolError>
where
    R: RagIngestRepository
        + RagSearchRepository
        + RagDiagnosticsRepository
        + Clone
        + Send
        + Sync
        + 'static,
    E: RagEmbeddingRuntime
        + crate::application::rag::RagGraphRuntime
        + RagSearchRuntime
        + Send
        + Sync
        + 'static,
{
    let mut tools: Vec<std::sync::Arc<dyn Tool>> = platform_tools::platform_business_tools(
        workspaces,
        apps,
        automations,
        tasks,
        skills,
        variables,
        resources.clone(),
        prompts,
        mcp,
    );
    tools.extend(coding_tools::coding_domain_tools(resources.clone()));
    tools.extend(productivity_tools::tools(resources.clone()));
    tools.extend(memory_tools::memory_business_tools(memory));
    tools.extend(short_term_memory_tools::short_term_memory_tools(
        short_term_memory,
    ));

    for tool in tools {
        registry.register(tool)?;
    }
    Ok(())
}

pub(crate) fn framework_local_scope(
    context: &crate::domain::agent::AgentRunContext,
) -> Result<
    eliteforge_agent_official_plugin_tools_local::FileSystemToolsConfig,
    crate::domain::agent::ToolError,
> {
    let cwd = coding_tools::default_cwd_for_context(context);
    // An unlinked legacy host session explicitly had machine-wide path authority.
    // Linked sessions retain exactly their resolved application/worktree grants.
    let roots = if context.execution_scope.allowed_local_directories.is_empty() {
        vec![
            cwd.ancestors()
                .last()
                .ok_or_else(|| {
                    crate::domain::agent::ToolError::ExecutionFailed(
                        "working directory is not absolute".into(),
                    )
                })?
                .to_path_buf(),
        ]
    } else {
        context
            .execution_scope
            .allowed_local_directories
            .iter()
            .map(|root| coding_tools::expand_tilde(root))
            .collect()
    };
    let mut config = eliteforge_agent_official_plugin_tools_local::FileSystemToolsConfig::new(cwd)
        .with_authorized_roots(roots.clone())
        .with_authorized_symlinks(true)
        .with_max_edit_bytes(32 * 1024 * 1024);
    // Local mounts use their canonical root as workspace identity. A future
    // distributed pack must replace these with deployment-stable mount IDs.
    for root in roots {
        let canonical = std::fs::canonicalize(&root).map_err(|error| crate::domain::agent::ToolError::ExecutionFailed(error.to_string()))?;
        config = config.with_coordination_root(&canonical, "tron.local", canonical.to_string_lossy().into_owned());
    }
    Ok(config)
}

pub(crate) fn framework_file_reference(
    reference: serde_json::Value,
    context: &crate::domain::agent::AgentRunContext,
    resources: &crate::application::resource::ResourceService,
    max_bytes: usize,
) -> Result<
    eliteforge_agent_official_plugin_tools_local::ReferencedFile,
    crate::domain::agent::ToolError,
> {
    let source = file_access::resolve_input_file_bounded(
        &serde_json::json!({"fileRef":reference}),
        context,
        resources,
        max_bytes as u64,
    )?;
    if source.bytes.len() > max_bytes {
        return Err(crate::domain::agent::ToolError::ExecutionFailed(
            "attachment exceeds input limit".into(),
        ));
    }
    Ok(
        eliteforge_agent_official_plugin_tools_local::ReferencedFile {
            name: source.file_name,
            bytes: source.bytes,
            reference: source.reference,
        },
    )
}

/// Resolve a delegated task directory using exactly the caller's path authority.
pub(crate) fn delegated_working_directory(
    context: &crate::domain::agent::AgentRunContext,
    raw: &str,
) -> Result<String, crate::domain::agent::ToolError> {
    let path = coding_tools::resolve_path(context, raw)?;
    std::fs::create_dir_all(&path).map_err(|error| {
        crate::domain::agent::ToolError::ExecutionFailed(format!(
            "could not prepare delegated working directory: {error}"
        ))
    })?;
    let canonical = path
        .canonicalize()
        .map_err(|error| crate::domain::agent::ToolError::ExecutionFailed(error.to_string()))?;
    // Check resolved symlinks as well as lexical relative paths.
    coding_tools::resolve_path(context, &canonical.to_string_lossy())?;
    Ok(canonical.to_string_lossy().into_owned())
}
