use super::*;
use tron_i18n::t;
async fn git(
    directory: &Path,
    args: &[&str],
    control: Option<&dyn ToolExecutionControl>,
) -> Result<Value, ToolError> {
    run(
        "git",
        &args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>(),
        directory,
        30,
        control,
    )
    .await
}
async fn checked(
    directory: &Path,
    args: &[&str],
    control: Option<&dyn ToolExecutionControl>,
    locale: &str,
) -> Result<String, ToolError> {
    let result = git(directory, args, control).await?;
    if result["exitCode"] != 0 {
        return Err(error(
            result["stderr"]["content"].as_str().unwrap_or("git failed"),
        ));
    }
    if ["stdout", "stderr"].iter().any(|stream| {
        result[*stream]["hasMore"].as_bool() != Some(false)
            || result[*stream]["droppedBytes"].as_u64() != Some(0)
            || result[*stream]["captureStatus"].as_str() != Some("eof")
    }) {
        return Err(error(
            t!(
                "agent_runtime.tool.worktree.git_output_too_large",
                locale = locale
            )
            .into_owned(),
        ));
    }
    Ok(result["stdout"]["content"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches(['\n', '\r'])
        .to_string())
}
async fn worktrees(
    directory: &Path,
    control: Option<&dyn ToolExecutionControl>,
    locale: &str,
) -> Result<Value, ToolError> {
    let output = checked(
        directory,
        &["worktree", "list", "--porcelain", "-z"],
        control,
        locale,
    )
    .await?;
    let mut items = Vec::new();
    let mut current = json!({});
    for field in output.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            if current.get("path").is_some() {
                items.push(current);
            }
            current = json!({"path":path,"branch":null,"head":null});
        } else if let Some(branch) = field.strip_prefix("branch refs/heads/") {
            current["branch"] = json!(branch);
        } else if let Some(head) = field.strip_prefix("HEAD ") {
            current["head"] = json!(head);
        }
    }
    if current.get("path").is_some() {
        items.push(current);
    }
    Ok(json!(items))
}
pub(super) async fn execute(
    name: &str,
    input: &Value,
    context: &AgentRunContext,
    control: Option<&dyn ToolExecutionControl>,
) -> Result<Value, ToolError> {
    let directory = cwd(input, context)?;
    let locale = context.locale.as_str();
    checked(
        &directory,
        &["rev-parse", "--show-toplevel"],
        control,
        locale,
    )
    .await?;
    match name {
        "worktree_list" => Ok(json!({"worktrees":worktrees(&directory, control, locale).await?})),
        "worktree_select" => {
            let selected = PathBuf::from(text(input, "path", locale)?)
                .canonicalize()
                .map_err(error)?;
            let items = worktrees(&directory, control, locale).await?;
            let registered = items.as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["path"]
                        .as_str()
                        .and_then(|path| Path::new(path).canonicalize().ok())
                        .as_ref()
                        == Some(&selected)
                })
            });
            if !registered {
                return Err(error(
                    t!(
                        "agent_runtime.tool.worktree.not_registered",
                        locale = locale
                    )
                    .into_owned(),
                ));
            }
            Ok(json!({"cwd":selected,"selected":true}))
        }
        "worktree_create" => {
            let name = text(input, "name", locale)?;
            if name.len() > 64
                || name.starts_with('.')
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(error(
                    t!("agent_runtime.tool.worktree.invalid_name", locale = locale).into_owned(),
                ));
            }
            let branch = text(input, "branch", locale)?;
            if branch.starts_with('-') {
                return Err(error(
                    t!(
                        "agent_runtime.tool.worktree.invalid_branch",
                        locale = locale
                    )
                    .into_owned(),
                ));
            }
            checked(
                &directory,
                &["check-ref-format", "--branch", branch],
                control,
                locale,
            )
            .await?;
            let reference = input
                .get("startPoint")
                .and_then(Value::as_str)
                .unwrap_or("HEAD");
            let commit = checked(
                &directory,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{reference}^{{commit}}"),
                ],
                control,
                locale,
            )
            .await?;
            let items = worktrees(&directory, control, locale).await?;
            let root = PathBuf::from(
                items
                    .as_array()
                    .and_then(|items| items.first())
                    .and_then(|item| item["path"].as_str())
                    .ok_or_else(|| {
                        error(
                            t!(
                                "agent_runtime.tool.worktree.main_worktree_missing",
                                locale = locale
                            )
                            .into_owned(),
                        )
                    })?,
            );
            let parent = root
                .parent()
                .ok_or_else(|| {
                    error(
                        t!(
                            "agent_runtime.tool.worktree.repository_parent_missing",
                            locale = locale
                        )
                        .into_owned(),
                    )
                })?
                .canonicalize()
                .map_err(error)?;
            let base = parent.join(".tron-worktrees");
            if base.exists() && base.canonicalize().map_err(error)? != base {
                return Err(error(
                    t!("agent_runtime.tool.worktree.root_symlink", locale = locale).into_owned(),
                ));
            }
            if let Some(control) = control {
                control.check_active()?;
            }
            std::fs::create_dir_all(&base).map_err(error)?;
            let destination = base.join(name);
            if destination.exists() {
                return Err(error(
                    t!(
                        "agent_runtime.tool.worktree.destination_exists",
                        locale = locale
                    )
                    .into_owned(),
                ));
            }
            checked(
                &directory,
                &[
                    "worktree",
                    "add",
                    "-b",
                    branch,
                    "--",
                    destination.to_str().ok_or_else(|| error("invalid path"))?,
                    &commit,
                ],
                control,
                locale,
            )
            .await?;
            // Creation is explicit and activates the new checkout for the next model boundary.
            Ok(json!({"cwd":destination,"branch":branch,"created":true,"selected":true}))
        }
        _ => Err(error(
            t!("agent_runtime.tool.worktree.unknown_tool", locale = locale).into_owned(),
        )),
    }
}
