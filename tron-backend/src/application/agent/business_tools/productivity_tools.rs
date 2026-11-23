mod office;
mod worktree;

use super::coding_tools::{json_observation, resolve_optional_directory, resolve_path};
use crate::{
    application::resource::ResourceService,
    domain::agent::{
        AgentRunContext, Tool, ToolError, ToolExecutionControl, ToolExecutionFuture,
        ToolExecutionResult, ToolRequest,
    },
};
use serde_json::{Value, json};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tron_i18n::t;

const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 65536;

pub(super) fn tools(resources: ResourceService) -> Vec<Arc<dyn Tool>> {
    [
        "worktree_list",
        "worktree_create",
        "worktree_select",
        "word_read",
        "word_create",
        "word_replace_text",
        "excel_read",
        "excel_write",
    ]
    .into_iter()
    .map(|name| {
        Arc::new(ProductivityTool {
            name,
            resources: resources.clone(),
        }) as Arc<dyn Tool>
    })
    .collect()
}
struct ProductivityTool {
    name: &'static str,
    resources: ResourceService,
}
impl Tool for ProductivityTool {
    fn name(&self) -> &str {
        self.name
    }
    fn exec(
        &self,
        request: ToolRequest,
        context: &AgentRunContext,
    ) -> Result<ToolExecutionResult, ToolError> {
        validate(self.name, &request.input, context)?;
        let result = if self.name.starts_with("word_") || self.name.starts_with("excel_") {
            office::execute(
                self.name,
                &request.input,
                context,
                &self.resources.for_agent_in_workspace(
                    context.agent_code.as_deref(),
                    context.execution_workspace_code(),
                ),
            )?
        } else {
            let name = self.name;
            let input = request.input;
            let context = context.clone();
            eliteforge_agent_official_plugin_tools_local::capture::host_operation_blocking(
                async move {
                    worktree::execute(name, &input, &context, None)
                        .await
                        .map_err(|error| {
                            eliteforge_agent_core::ToolError::Failed(error.to_string())
                        })
                },
            )
            .map_err(error)?
        };
        finish(self.name, result, context.locale.as_str())
    }
    fn exec_async<'a>(
        &'a self,
        request: ToolRequest,
        context: &'a AgentRunContext,
        control: &'a dyn ToolExecutionControl,
    ) -> Option<ToolExecutionFuture<'a>> {
        if self.name.starts_with("word_") || self.name.starts_with("excel_") {
            return None;
        }
        Some(Box::pin(async move {
            control.check_active()?;
            validate(self.name, &request.input, context)?;
            let result =
                worktree::execute(self.name, &request.input, context, Some(control)).await?;
            control.check_active()?;
            finish(self.name, result, context.locale.as_str())
        }))
    }
}
fn validate(name: &str, input: &Value, context: &AgentRunContext) -> Result<(), ToolError> {
    let locale = context.locale.as_str();
    if input.to_string().len() > 4 * 1024 * 1024 {
        return Err(error(
            t!(
                "agent_runtime.tool.productivity.input_too_large",
                locale = locale
            )
            .into_owned(),
        ));
    }
    let definition = crate::application::agent::BusinessToolCatalog::tool(
        &name.replacen('_', ".", 1),
        &context.locale,
    )
    .ok_or_else(|| {
        error(
            t!(
                "agent_runtime.tool.productivity.definition_unavailable",
                locale = locale
            )
            .into_owned(),
        )
    })?;
    let validator = jsonschema::validator_for(&definition.input_schema).map_err(error)?;
    if let Err(failure) = validator.validate(&input) {
        return Err(error(t!(
            "agent_runtime.tool.productivity.invalid_input",
            locale = locale,
            detail = failure
        )));
    }
    if ["worktree_create", "worktree_select"].contains(&name)
        && context.execution_scope.applications.is_empty()
    {
        return Err(error(
            t!(
                "agent_runtime.tool.worktree.project_application_required",
                locale = locale
            )
            .into_owned(),
        ));
    }
    Ok(())
}
fn finish(name: &str, result: Value, locale: &str) -> Result<ToolExecutionResult, ToolError> {
    if (name.starts_with("word_") || name.starts_with("excel_") || name == "worktree_list")
        && result.to_string().len() > MAX_OUTPUT_BYTES
    {
        return Err(error(
            t!(
                "agent_runtime.tool.productivity.result_too_large",
                locale = locale
            )
            .into_owned(),
        ));
    }
    json_observation(name, &result)
}

fn error(message: impl ToString) -> ToolError {
    ToolError::ExecutionFailed(message.to_string())
}
fn text<'a>(input: &'a Value, key: &str, locale: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            error(t!(
                "agent_runtime.tool.productivity.parameter_required",
                locale = locale,
                parameter = key
            ))
        })
}
fn path(input: &Value, key: &str, context: &AgentRunContext) -> Result<PathBuf, ToolError> {
    resolve_path(context, text(input, key, context.locale.as_str())?)
}
fn cwd(input: &Value, context: &AgentRunContext) -> Result<PathBuf, ToolError> {
    resolve_optional_directory(context, input.get("cwd").and_then(Value::as_str))?.ok_or_else(
        || {
            error(
                t!(
                    "agent_runtime.tool.productivity.working_directory_required",
                    locale = context.locale.as_str()
                )
                .into_owned(),
            )
        },
    )
}
fn read_file(path: &Path, locale: &str) -> Result<Vec<u8>, ToolError> {
    let file = std::fs::File::open(path).map_err(error)?;
    if file.metadata().map_err(error)?.len() > MAX_FILE_BYTES {
        return Err(error(
            t!(
                "agent_runtime.tool.productivity.file_too_large",
                locale = locale
            )
            .into_owned(),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(error(
            t!(
                "agent_runtime.tool.productivity.file_too_large",
                locale = locale
            )
            .into_owned(),
        ));
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8], locale: &str) -> Result<(), ToolError> {
    let parent = path.parent().ok_or_else(|| {
        error(
            t!(
                "agent_runtime.tool.productivity.output_parent_required",
                locale = locale
            )
            .into_owned(),
        )
    })?;
    if !parent.is_dir() {
        return Err(error(
            t!(
                "agent_runtime.tool.productivity.output_parent_missing",
                locale = locale
            )
            .into_owned(),
        ));
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
    std::io::Write::write_all(&mut temporary, bytes).map_err(error)?;
    temporary.persist_noclobber(path).map_err(error)?;
    Ok(())
}
async fn run(
    command: &str,
    args: &[String],
    directory: &Path,
    timeout: u64,
    control: Option<&dyn ToolExecutionControl>,
) -> Result<Value, ToolError> {
    use eliteforge_agent_official_plugin_tools_local::capture::{
        CapturedCommand, capture_command, capture_command_with_cancel,
    };
    let mut env = std::collections::BTreeMap::new();
    let mut env_remove = Vec::new();
    if command == "git" {
        env_remove = std::env::vars_os()
            .filter_map(|(key, _)| key.into_string().ok())
            .filter(|key| key.starts_with("GIT_"))
            .collect();
        env.insert("GIT_OPTIONAL_LOCKS".into(), "0".into());
        env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    }
    if let Some(control) = control {
        control.check_active()?;
    }
    let spec = CapturedCommand {
        program: command.into(),
        args: args.to_vec(),
        cwd: directory.into(),
        env,
        env_remove,
        timeout: Duration::from_secs(timeout),
        max_output_bytes: MAX_OUTPUT_BYTES,
    };
    let output = if let Some(control) = control {
        capture_command_with_cancel(
            spec,
            control.cancelled(),
            tokio::time::Instant::now() + control.remaining(),
        )
        .await
    } else {
        capture_command(spec).await
    }
    .map_err(error)?;
    if matches!(
        output["status"].as_str(),
        Some("timed_out" | "failed" | "cancelled" | "running")
    ) {
        return Err(error(
            output["error"]
                .as_str()
                .unwrap_or("command did not complete"),
        ));
    }
    Ok(output)
}
