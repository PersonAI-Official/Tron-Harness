//! Stable product-owned working directories; never borrow the backend process cwd.
use super::*;
use crate::domain::agent::{AgentExecutionScope, AgentRunContext};

fn component(value: &str) -> String {
    if !value.is_empty()
        && value != "."
        && value != ".."
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        value.to_owned()
    } else {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(value.as_bytes()))
    }
}

/// Roll back staged filesystem changes when a container transaction fails.
pub(crate) struct StagedSessionDirectories {
    paths: Vec<(PathBuf, PathBuf)>,
}

impl StagedSessionDirectories {
    pub(crate) fn commit(mut self) -> Result<(), String> {
        let mut failure = None;
        for (_, staged) in self.paths.drain(..) {
            let result = if staged.is_symlink() {
                fs::remove_file(&staged)
            } else {
                fs::remove_dir_all(&staged)
            };
            if let Err(error) = result {
                tracing::error!(path = %staged.display(), %error, "could not remove staged directory after committed deletion");
                failure = Some(format!("{}: {error}", staged.display()));
            }
        }
        failure.map_or(Ok(()), Err)
    }

    /// Remove staged directories off the request path. The rename that staged
    /// them already made the deletion atomic, so a slow `remove_dir_all` (large
    /// session workspace) must not delay the response or hold the database
    /// write transaction; a failure only leaves an unreferenced `.deleting-*`
    /// directory for later cleanup. Detaching the paths also disables the
    /// rollback in `Drop`, which would otherwise restore a deleted session.
    pub(crate) fn commit_detached(mut self) {
        let paths = std::mem::take(&mut self.paths);
        if paths.is_empty() {
            return;
        }
        if let Err(error) = std::thread::Builder::new()
            .name("session-directory-cleanup".into())
            .spawn(move || {
                for (_, staged) in paths {
                    let result = if staged.is_symlink() {
                        fs::remove_file(&staged)
                    } else {
                        fs::remove_dir_all(&staged)
                    };
                    if let Err(error) = result {
                        tracing::error!(path = %staged.display(), %error, "could not remove staged session directory after committed deletion");
                    }
                }
            })
        {
            tracing::error!(%error, "could not start session directory cleanup; staged directories remain");
        }
    }
}

impl Drop for StagedSessionDirectories {
    fn drop(&mut self) {
        for (original, staged) in self.paths.iter().rev() {
            if let Err(error) = fs::rename(staged, original) {
                tracing::error!(path = %original.display(), staged = %staged.display(), %error, "could not restore session directory after rolled back deletion");
            }
        }
    }
}

/// Profiles created before the Assistant rename keep assistant work directories
/// under the former `dudes` name; move the tree once so their files stay reachable.
fn migrate_legacy_assistant_directory(root: &std::path::Path) {
    let legacy = root.join("dudes");
    let current = root.join("assistants");
    if current.exists() || !legacy.is_dir() {
        return;
    }
    if let Err(error) = fs::rename(&legacy, &current) {
        tracing::warn!(path = %legacy.display(), %error, "could not move legacy assistant work directory");
    }
}

impl FrameworkHost {
    /// dsh's immutable session cwd identifies the shared UI group. The CLI
    /// adapter sets effective cwd and standing write root to the actual task.
    pub(crate) fn dsh_workspace_options(
        &self,
    ) -> eliteforge_agent_capability_external::DshWorkspaceOptions {
        eliteforge_agent_capability_external::DshWorkspaceOptions {
            directory: self.0.path.with_extension("workspaces").join("sessions"),
            title: "Tron harness".into(),
        }
    }

    fn managed_working_directory(&self, kind: &str, user: Option<&str>, id: &str) -> PathBuf {
        let root = self.0.path.with_extension("workspaces");
        if kind == "assistants" {
            migrate_legacy_assistant_directory(&root);
        }
        root.join(kind)
            .join(component(user.unwrap_or("system")))
            .join(component(id))
    }

    pub(crate) async fn stage_session_working_directories(
        &self,
        sessions: Vec<(String, Option<String>)>,
    ) -> Result<StagedSessionDirectories, String> {
        for (session, _) in &sessions {
            self.invalidate_file_session(session).await?;
        }
        let root = self.0.path.with_extension("workspaces");
        let directories = sessions
            .into_iter()
            .map(|(session, user)| {
                self.managed_working_directory("sessions", user.as_deref(), &session)
            })
            .collect::<Vec<_>>();
        tokio::task::spawn_blocking(move || {
            let mut staged = StagedSessionDirectories { paths: Vec::new() };
            'directory: for directory in directories {
                for parent in [
                    root.clone(),
                    root.join("sessions"),
                    directory.parent().unwrap().to_owned(),
                ] {
                    match fs::symlink_metadata(&parent) {
                        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                        }
                        Ok(_) => {
                            return Err(format!(
                                "session working directory parent is not managed: {}",
                                parent.display()
                            ));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            continue 'directory;
                        }
                        Err(error) => {
                            return Err(format!(
                                "could not inspect session working directory: {error}"
                            ));
                        }
                    }
                }
                match fs::symlink_metadata(&directory) {
                    Ok(metadata) if metadata.is_dir() || metadata.file_type().is_symlink() => {}
                    Ok(_) => {
                        return Err(format!(
                            "session working path is not a directory: {}",
                            directory.display()
                        ));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(format!(
                            "could not inspect session working directory: {error}"
                        ));
                    }
                }
                let target =
                    directory.with_file_name(format!(".deleting-{:032x}", rand::random::<u128>()));
                match fs::rename(&directory, &target) {
                    Ok(()) => staged.paths.push((directory, target)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(format!(
                            "could not stage session working directory: {error}"
                        ));
                    }
                }
            }
            Ok(staged)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub(super) fn delegated_execution_scope(
        &self,
        context: &AgentRunContext,
        raw: &str,
    ) -> Result<AgentExecutionScope, String> {
        let directory =
            crate::application::agent::business_tools::delegated_working_directory(context, raw)
                .map_err(|error| error.to_string())?;
        let mut scope = context.execution_scope.clone();
        let selected = std::path::Path::new(&directory);
        let inspector = self
            .0
            .collaboration_builder
            .get()
            .and_then(std::sync::Weak::upgrade)
            .and_then(|builder| builder.directory_inspector());
        let primary = scope
            .applications
            .iter()
            .enumerate()
            .flat_map(|(index, app)| {
                let mut roots = vec![
                    app.working_directory
                        .clone()
                        .unwrap_or_else(|| app.configured_directory.clone()),
                    app.configured_directory.clone(),
                ];
                if let Some(inspector) = &inspector {
                    roots.extend(
                        inspector
                            .list_worktrees(&app.configured_directory)
                            .into_iter()
                            .map(|tree| tree.path),
                    );
                }
                roots.into_iter().map(move |root| (index, root))
            })
            .filter_map(|(index, root)| {
                let root = Path::new(&root)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(root));
                selected.starts_with(&root).then_some((index, root))
            })
            .max_by_key(|(_, root)| root.components().count());
        if let Some((index, root)) = primary {
            if let Some(inspector) = &inspector {
                scope.applications[index] =
                    crate::application::project::inspect_current_application(
                        inspector.as_ref(),
                        &scope.applications[index],
                        root.to_str(),
                    );
            }
            for (current, app) in scope.applications.iter_mut().enumerate() {
                app.is_primary = current == index;
            }
            scope.app_code = Some(scope.applications[index].app_code.clone());
            scope.branch_name = scope.applications[index].branch_name.clone();
        }
        scope.local_directory = Some(directory);
        Ok(scope)
    }
    pub(crate) fn prepare_working_directory(
        &self,
        context: &mut AgentRunContext,
    ) -> Result<(), String> {
        if let Some(directory) = &context.execution_scope.local_directory {
            if directory.trim().is_empty() {
                return Err("configured working directory must not be empty".into());
            }
            // An explicitly selected directory is authoritative, even if missing.
            // The executor must report that error instead of silently replacing it.
            return Ok(());
        }
        if let Some(primary) = context
            .execution_scope
            .applications
            .iter()
            .find(|app| app.is_primary)
        {
            context.execution_scope.local_directory = Some(
                primary
                    .working_directory
                    .clone()
                    .unwrap_or_else(|| primary.configured_directory.clone()),
            );
            return Ok(());
        }
        let (kind, id) = match context
            .collaboration_owner
            .as_ref()
            .filter(|owner| owner.kind == "ASSISTANT")
        {
            Some(owner) => ("assistants", owner.code.as_str()),
            None => (
                "sessions",
                context
                    .conversation_code
                    .as_deref()
                    .unwrap_or(&context.run_code),
            ),
        };
        let directory = self.managed_working_directory(kind, context.user_code.as_deref(), id);
        fs::create_dir_all(&directory)
            .map_err(|error| format!("could not prepare session working directory: {error}"))?;
        let directory = directory
            .canonicalize()
            .map_err(|error| format!("could not resolve session working directory: {error}"))?;
        let directory = directory.to_string_lossy().into_owned();
        if !context.execution_scope.allowed_local_directories.is_empty() {
            context
                .execution_scope
                .allowed_local_directories
                .push(directory.clone());
        }
        context.execution_scope.local_directory = Some(directory);
        Ok(())
    }
}
