impl ChatService {
    /// Trusted host entrypoint. Platform PersonAI ownership is persisted by AssistantService;
    /// public user session creation continues to require an actual workspace.
    pub fn ensure_assistant_session(&self, assistant: &crate::domain::assistant::AssistantSession, user_code: Option<String>) -> Result<ChatSession, ChatError> {
        if let Ok(existing)=self.repository().get_session(&assistant.session_code) { if existing.user_code!=user_code {return Err(ChatError::WorkspaceForbidden);} return Ok(existing); }
        let now=current_timestamp();
        let session=ChatSession {
            execution_mode: Default::default(),
            id:self.id_provider.next_id().map_err(|_|ChatError::StorageFailure)?.value(),
            session_code:assistant.session_code.clone(), workspace_code:assistant.workspace_code.clone().unwrap_or_default(),
            project_code:None,directory_code:None,working_directory_code:None,user_code,app_code:None,task_code:(assistant.purpose == crate::domain::assistant::AssistantSessionPurpose::Task).then(|| assistant.correlation_key.clone()),source_type:ChatSessionSourceType::Manual,
            source_code:Some(format!("__assistant:{}",assistant.assistant_code)),agent_code:Some(assistant.assistant_code.clone()),
            system_agent_session_id:None,title:assistant.title.clone(),status:ChatSessionStatus::Active,
            last_run_code:None,last_run_status:None,created_at:now.clone(),updated_at:now,
        };
        let mut repository=self.repository();
        if let Ok(existing)=repository.get_session(&assistant.session_code){if existing.user_code!=session.user_code {return Err(ChatError::WorkspaceForbidden);}return Ok(existing)}
        repository.create_session(&session)?;Ok(session)
    }

    pub fn create_session(
        &self,
        command: CreateChatSessionCommand,
        user_code: Option<String>,
    ) -> Result<ChatSession, ChatError> {
        if command.source_code.as_deref().is_some_and(|source| source.starts_with("__assistant:") || source.starts_with("__knowledge:")) {
            return Err(ChatError::WorkspaceForbidden);
        }
        let workspace_code =
            normalized_required(command.workspace_code).ok_or(ChatError::WorkspaceRequired)?;
        let id = self
            .id_provider
            .next_id()
            .map_err(|_| ChatError::StorageFailure)?
            .value();
        let now = current_timestamp();
        let task_code = normalized_optional(command.task_code);
        let source_type = command.source_type.unwrap_or(if task_code.is_some() {
            ChatSessionSourceType::Task
        } else {
            ChatSessionSourceType::Manual
        });
        let project_code = normalized_optional(command.project_code);
        let directory_code = normalized_optional(command.directory_code);
        if directory_code.is_some() && (project_code.is_some() || source_type != ChatSessionSourceType::Manual) {
            return Err(ChatError::DirectoryInvalid);
        }
        if let Some(code) = directory_code.as_deref() {
            self.directories.as_ref().ok_or(ChatError::DirectoryInvalid)?
                .resolve(&workspace_code, code).map_err(|_| ChatError::DirectoryInvalid)?;
        }
        let requested_working = normalized_optional(command.working_directory_code);
        if directory_code.as_ref().zip(requested_working.as_ref()).is_some_and(|(directory, working)| directory != working) {
            return Err(ChatError::DirectoryFixed);
        }
        if let Some(code) = requested_working.as_deref() {
            self.directories.as_ref().ok_or(ChatError::DirectoryInvalid)?
                .resolve(&workspace_code, code).map_err(|_| ChatError::DirectoryInvalid)?;
        }
        let working_directory_code = if let Some(project) = project_code.as_deref() {
            if let Some(topics) = self.topics.as_ref() {
                let memo = topics.get(user_code.as_deref().unwrap_or(""), project).map_err(|_| ChatError::ProjectInvalid)?;
                let directories = memo.references.iter().filter(|reference| reference.kind == crate::domain::topic::TopicReferenceKind::Directory).collect::<Vec<_>>();
                requested_working.or_else(|| directories.first().map(|reference| reference.target_code.clone()))
            } else if requested_working.is_some() {
                return Err(ChatError::DirectoryInvalid);
            } else {
                None
            }
        } else if let Some(code) = directory_code.as_ref() {
            requested_working.or_else(|| Some(code.clone()))
        } else {
            requested_working
        };
        let app_code = if source_type == ChatSessionSourceType::Manual
            && self.projects.is_some()
            && project_code.is_some()
        {
            let projects = self.projects.as_ref().ok_or(ChatError::ProjectInvalid)?;
            let project = projects
                .resolve_execution_context(
                    &workspace_code,
                    project_code.as_deref().expect("project code is checked"),
                )
                .map_err(|_| ChatError::ProjectInvalid)?;
            project.primary_app.map(|app| app.app_code)
        } else {
            normalized_optional(command.app_code)
        };
        let session = ChatSession {
            execution_mode: command.execution_mode,
            id,
            session_code: eliteforge_agent_core::SessionRecord::new("").id,
            workspace_code,
            project_code,
            directory_code,
            working_directory_code,
            user_code,
            app_code,
            task_code,
            source_type,
            source_code: normalized_optional(command.source_code),
            agent_code: normalized_optional(command.agent_code),
            system_agent_session_id: None,
            title: normalized_optional(command.title)
                .unwrap_or_else(|| DEFAULT_MANUAL_SESSION_TITLE.into()),
            status: ChatSessionStatus::Active,
            last_run_code: None,
            last_run_status: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let mut repository = self.repository();
        if session.source_type == ChatSessionSourceType::Manual
            && session.project_code.is_some()
            && let Some(existing) = repository.list_sessions()?.into_iter().find(|existing| {
                existing.workspace_code == session.workspace_code
                    && existing.project_code == session.project_code
                    && existing.working_directory_code == session.working_directory_code
                    && existing.user_code == session.user_code
                    && existing.source_type == ChatSessionSourceType::Manual
                    && existing.execution_mode == session.execution_mode
                    && existing.status == ChatSessionStatus::Active
                    && existing.last_run_code.is_none()
            })
        {
            return Ok(existing);
        }
        repository.create_session(&session)?;
        Ok(session)
    }

    pub fn list_session_groups(
        &self,
        query: ChatSessionQuery,
    ) -> Result<Vec<ChatSessionGroup>, ChatError> {
        let workspace_code =
            normalized_required(query.workspace_code).ok_or(ChatError::WorkspaceRequired)?;
        let mut sessions = self.repository().list_sessions_scoped(Some(&workspace_code), query.source_type)?;
        sessions.retain(|session| {
            session.workspace_code == workspace_code
                && !session.source_code.as_deref().is_some_and(|s|s.starts_with("__assistant:") || s.starts_with("__knowledge:"))
                && session.status == ChatSessionStatus::Active
                && session.source_type != ChatSessionSourceType::Terminal
        });

        let codes = sessions
            .iter()
            .map(|session| session.session_code.clone())
            .collect::<Vec<_>>();
        let pending_counts = self.repository().pending_input_counts(&codes)?;
        let mut grouped = BTreeMap::<String, ChatSessionGroup>::new();
        for session in sessions {
            let task_title = match session.source_type {
                ChatSessionSourceType::Automation => "Automation".into(),
                ChatSessionSourceType::Task => {
                    session.task_code.clone().unwrap_or_else(|| "Task".into())
                }
                ChatSessionSourceType::Manual => "Chat".into(),
                ChatSessionSourceType::Terminal => "Terminal".into(),
            };
            let group_key = format!(
                "{:?}:{}:{}",
                session.source_type,
                session.source_code.as_deref().unwrap_or(""),
                session.task_code.as_deref().unwrap_or("")
            );
            let group = grouped
                .entry(group_key)
                .or_insert_with(|| ChatSessionGroup {
                    task_code: session.task_code.clone(),
                    task_title,
                    source_type: session.source_type,
                    source_code: session.source_code.clone(),
                    pending_user_input_count: 0,
                    pending_user_input_session_codes: Vec::new(),
                    sessions: Vec::new(),
                });
            group.sessions.push(session);
        }
        for group in grouped.values_mut() {
            for session in &group.sessions {
                let pending = pending_counts
                    .get(&session.session_code)
                    .copied()
                    .unwrap_or(0);
                if pending > 0 {
                    group.pending_user_input_count += pending;
                    group
                        .pending_user_input_session_codes
                        .push(session.session_code.clone());
                }
            }
        }
        Ok(grouped.into_values().collect())
    }

    pub fn get_session(&self, session_code: &str) -> Result<ChatSession, ChatError> {
        self.repository().get_session(session_code)
    }

    pub fn sessions_by_codes(&self, codes: &[String]) -> Result<Vec<ChatSession>, ChatError> {
        self.repository().sessions_by_codes(codes)
    }

    pub fn session_titles(
        &self,
        codes: &[String],
    ) -> Result<Vec<crate::application::chat::ChatSessionTitle>, ChatError> {
        let codes = codes
            .iter()
            .filter(|code| !code.is_empty())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if codes.is_empty() {
            return Ok(Vec::new());
        }
        self.repository().session_titles(&codes)
    }

    pub fn ensure_session_writable(&self, session_code: &str) -> Result<(), ChatError> {
        if self.active_runs.snapshot(session_code).map_err(|_| ChatError::StorageFailure)?
            .is_some_and(|run| run.operation_kind.as_deref() == Some("ContextCompaction")) {
            return Err(ChatError::SessionRunActive);
        }
        let session = self.repository().get_session(session_code)?;
        if session.status != ChatSessionStatus::Active
            || self
                .active_runs
                .is_retired(session_code)
                .map_err(|_| ChatError::StorageFailure)?
        {
            return Err(ChatError::SessionArchived);
        }
        Ok(())
    }

    pub fn archive_session(&self, session_code: &str) -> Result<ChatSession, ChatError> {
        let admission = self.active_runs.lock_admission().map_err(|_| ChatError::StorageFailure)?;
        let session = self.repository().get_session(session_code)?;
        if session.status == ChatSessionStatus::Archived {
            admission.retire(&[session_code.to_owned()]).map_err(|_| ChatError::StorageFailure)?;
            return Ok(session);
        }
        if self
            .execution_memory
            .as_ref()
            .map(|memory| memory.get_active_goal(session_code))
            .transpose()
            .map_err(|_| ChatError::StorageFailure)?
            .flatten()
            .is_some_and(|goal| goal.goal.status == "ACTIVE")
        {
            return Err(ChatError::SessionRunActive);
        }
        admission
            .ensure_idle(session_code)
            .map_err(|error| {
                if matches!(
                    error,
                    crate::application::agent::ActiveRunError::AlreadyActive
                        | crate::application::agent::ActiveRunError::SessionArchived
                ) {
                    ChatError::SessionRunActive
                } else {
                    ChatError::StorageFailure
                }
            })?;
        let updated_at = current_timestamp();
        self.repository().update_session_status(
            session_code,
            ChatSessionStatus::Archived,
            &updated_at,
        )?;
        admission.retire(&[session_code.to_owned()]).map_err(|_| ChatError::StorageFailure)?;
        self.repository().get_session(session_code)
    }

    pub fn archive_directory(&self, workspace_code: &str, directory_code: &str) -> Result<Vec<String>, ChatError> {
        self.mutate_container(crate::application::chat::ChatContainer::Directory {
            workspace_code: workspace_code.into(), directory_code: directory_code.into(),
        }, false)
    }

    pub fn delete_directory(&self, workspace_code: &str, directory_code: &str) -> Result<Vec<String>, ChatError> {
        self.mutate_container(crate::application::chat::ChatContainer::Directory {
            workspace_code: workspace_code.into(), directory_code: directory_code.into(),
        }, true)
    }

    pub fn mutate_project(&self, workspace_code: &str, project_code: &str, delete: bool) -> Result<Vec<String>, ChatError> {
        self.mutate_container(crate::application::chat::ChatContainer::Project {
            workspace_code: workspace_code.into(), project_code: project_code.into(),
        }, delete)
    }

    fn mutate_container(&self, container: crate::application::chat::ChatContainer, delete: bool) -> Result<Vec<String>, ChatError> {
        // Keep run admission and chat placement locked until the entire batch is committed.
        let admission = self.active_runs.lock_admission().map_err(|_| ChatError::StorageFailure)?;
        let mut repository = self.repository();
        let snapshot = repository.container_snapshot(&container)?;
        let mut attachments = Vec::new();
        for code in &snapshot.session_codes {
            let session = repository.get_session(code)?;
            if session.status == ChatSessionStatus::Active {
                admission.ensure_idle(code).map_err(|error| match error {
                    crate::application::agent::ActiveRunError::AlreadyActive
                    | crate::application::agent::ActiveRunError::SessionArchived => ChatError::SessionRunActive,
                    _ => ChatError::StorageFailure,
                })?;
            }
            if delete && let Some(resources) = self.resources.as_ref() {
                attachments.extend(resources.chat_session_attachment_records(code).map_err(chat_error_from_resource)?);
            }
        }
        repository.mutate_container(&container, &snapshot, delete)?;
        admission.retire(&snapshot.session_codes).map_err(|_| ChatError::StorageFailure)?;
        drop(repository);
        drop(admission);
        if delete && let Some(resources) = self.resources.as_ref() {
            resources.cleanup_chat_session_attachment_files(&attachments);
        }
        Ok(snapshot.session_codes)
    }

    pub fn restore_session(&self, session_code: &str) -> Result<ChatSession, ChatError> {
        let admission = self.active_runs.lock_admission().map_err(|_| ChatError::StorageFailure)?;
        let session = self.repository().get_session(session_code)?;
        if session.status == ChatSessionStatus::Active {
            admission
                .reactivate(session_code)
                .map_err(|_| ChatError::StorageFailure)?;
            return Ok(session);
        }
        let updated_at = current_timestamp();
        self.repository().update_session_status(
            session_code,
            ChatSessionStatus::Active,
            &updated_at,
        )?;
        admission
            .reactivate(session_code)
            .map_err(|_| ChatError::StorageFailure)?;
        self.repository().get_session(session_code)
    }

    pub fn prepare_session_deletion(&self, session_code: &str) -> Result<(), ChatError> {
        self.repository().get_session(session_code)?;
        self.ensure_session_inactive(session_code)
    }

    pub fn delete_session(&self, session_code: &str) -> Result<ChatSession, ChatError> {
        let session = self.repository().get_session(session_code)?;
        self.delete_session_inner(session, false)
    }

    pub fn delete_archived_session(&self, session_code: &str) -> Result<ChatSession, ChatError> {
        let session = self.repository().get_session(session_code)?;
        if session.status != ChatSessionStatus::Archived {
            return Err(ChatError::SessionMustBeArchived);
        }
        self.delete_session_inner(session, false)
    }

    pub(crate) fn delete_expired_heartbeat_session(&self, session_code: &str) -> Result<ChatSession, ChatError> {
        let session = self.repository().get_session(session_code)?;
        if session.status != ChatSessionStatus::Archived {
            return Err(ChatError::SessionMustBeArchived);
        }
        self.delete_session_inner(session, true)
    }

    fn delete_session_inner(&self, session: ChatSession, heartbeat_retention: bool) -> Result<ChatSession, ChatError> {
        let started = std::time::Instant::now();
        let session_code = session.session_code.as_str();
        let attachments = self
            .resources
            .as_ref()
            .map(|resources| resources.chat_session_attachment_records(session_code))
            .transpose()
            .map_err(chat_error_from_resource)?
            .unwrap_or_default();
        // Block new runs during deletion without changing the persisted status.
        self.ensure_session_inactive(session_code)?;
        let deleted = if heartbeat_retention {
            self.repository().delete_expired_heartbeat_session(session_code)
        } else {
            self.repository().delete_session(session_code)
        };
        if let Err(error) = deleted {
            // A failed delete must not leave an active conversation read-only.
            if session.status == ChatSessionStatus::Active
                && self.repository().get_session(session_code)
                    .is_ok_and(|current| current.status == ChatSessionStatus::Active)
            {
                let _ = self.active_runs.reactivate_session(session_code);
            }
            return Err(error);
        }
        // The rows are gone once the repository call returns; attachment files
        // are best-effort and must not keep the response waiting for disk I/O.
        let attachment_count = attachments.len();
        if let Some(resources) = self.resources.clone()
            && attachment_count > 0
        {
            let _ = std::thread::Builder::new()
                .name("session-attachment-cleanup".into())
                .spawn(move || resources.cleanup_chat_session_attachment_files(&attachments));
        }
        if !heartbeat_retention {
            tracing::info!(
                session_code,
                elapsed_ms = started.elapsed().as_millis() as u64,
                attachments = attachment_count,
                "chat session deleted"
            );
        }
        Ok(session)
    }

    fn ensure_session_inactive(&self, session_code: &str) -> Result<(), ChatError> {
        if self
            .execution_memory
            .as_ref()
            .map(|memory| memory.get_active_goal(session_code))
            .transpose()
            .map_err(|_| ChatError::StorageFailure)?
            .flatten()
            .is_some_and(|goal| goal.goal.status == "ACTIVE")
        {
            return Err(ChatError::SessionRunActive);
        }
        self.active_runs
            .retire_session(session_code)
            .map_err(|error| {
                if matches!(
                    error,
                    crate::application::agent::ActiveRunError::AlreadyActive
                ) {
                    ChatError::SessionRunActive
                } else {
                    ChatError::StorageFailure
                }
            })?;
        Ok(())
    }

    pub fn store_session_attachment_from_reader(
        &self,
        session_code: &str,
        command: ChatSessionAttachmentUploadCommand,
        source: &mut (dyn std::io::Read + Send),
    ) -> Result<ChatMessageAttachment, ChatError> {
        let session = self.repository().get_session(session_code)?;
        if session.status != ChatSessionStatus::Active {
            return Err(ChatError::SessionArchived);
        }
        let resources = self.resources.as_ref().ok_or(ChatError::StorageFailure)?;
        // PersonAI/assistant sessions have no workspace. Resource storage still
        // needs a stable namespace so pasted files can be persisted and read.
        let attachment_workspace = if session.workspace_code.trim().is_empty() {
            "default".to_string()
        } else {
            session.workspace_code.clone()
        };
        let id = self
            .id_provider
            .next_id()
            .map_err(|_| ChatError::StorageFailure)?
            .value();
        let attachment_code = format!("CHAT-ATTACH-{id}");
        let summary =
            normalized_optional(command.summary).unwrap_or_else(|| command.file_name.clone());
        let attachment = ChatMessageAttachment {
            id: attachment_code.clone(),
            kind: command.kind,
            title: Some(command.file_name.clone()),
            summary,
            image_url: None,
            resource_code: Some(attachment_code.clone()),
            file_name: Some(command.file_name.clone()),
            content_type: Some(command.content_type.clone()),
            byte_size: None,
            metadata: command.metadata,
        };
        let mut attachment = normalize_chat_attachments(vec![attachment])?
            .pop()
            .expect("a single normalized attachment is retained");
        let memory = self
            .execution_memory
            .as_ref()
            .ok_or(ChatError::StorageFailure)?;
        let mut stored = None;
        let result = memory.with_active_session_write(session_code, || {
            let record = resources
                .store_chat_session_attachment(
                    ChatSessionAttachmentCreateCommand {
                        workspace_code: attachment_workspace,
                        session_code: session.session_code,
                        attachment_code,
                        file_name: command.file_name,
                        content_type: command.content_type,
                    },
                    source,
                )
                .map_err(chat_error_from_resource)?;
            stored = Some(record.clone());
            Ok(record)
        });
        let record = match result {
            Ok(record) => record,
            Err(error) => {
                if let Some(record) = stored {
                    resources.cleanup_chat_session_attachment_files(&[record]);
                }
                return Err(error);
            }
        };
        apply_attachment_record(&mut attachment, &record);
        Ok(attachment)
    }

    pub fn read_session_attachment(
        &self,
        session_code: &str,
        attachment_code: &str,
    ) -> Result<(ChatSessionAttachmentRecord, Vec<u8>), ChatError> {
        let session = self.repository().get_session(session_code)?;
        let resources = self.resources.as_ref().ok_or(ChatError::StorageFailure)?;
        let attachment_workspace = if session.workspace_code.trim().is_empty() {
            "default".to_string()
        } else {
            session.workspace_code.clone()
        };
        resources
            .read_chat_session_attachment(
                &attachment_workspace,
                &session.session_code,
                attachment_code,
            )
            .map_err(chat_error_from_resource)
    }

    pub fn update_session(
        &self,
        session_code: &str,
        command: UpdateChatSessionCommand,
    ) -> Result<ChatSession, ChatError> {
        let session = self.repository().get_session(session_code)?;
        if session.status != ChatSessionStatus::Active {
            return Err(ChatError::SessionArchived);
        }
        self.ensure_session_writable(session_code)?;
        let title = command.title.map(|title| {
            normalized_required(Some(title)).ok_or(ChatError::SessionTitleRequired)
        }).transpose()?;
        if title.is_none() && command.execution_mode.is_none() {
            return Err(ChatError::SessionTitleRequired);
        }
        let updated_at = current_timestamp();
        self.repository()
            .update_session_settings(session_code, title.as_deref(), command.execution_mode, &updated_at)?;
        self.repository().get_session(session_code)
    }

    /// Move a manual chat to a topic or quick chats without changing its cwd.
    pub fn move_session(&self, session_code: &str, project_code: Option<String>) -> Result<ChatSession, ChatError> {
        self.change_session_location(session_code, Some(normalized_optional(project_code)), None)
    }

    /// Set the cwd independently of the chat's placement.
    pub fn change_session_directory(&self, session_code: &str, directory_code: Option<String>) -> Result<ChatSession, ChatError> {
        self.change_session_location(session_code, None, Some(normalized_optional(directory_code)))
    }

    fn change_session_location(
        &self,
        session_code: &str,
        placement: Option<Option<String>>,
        working: Option<Option<String>>,
    ) -> Result<ChatSession, ChatError> {
        let _guard = self.active_runs.idle_session_guard(session_code).map_err(|error| match error {
            crate::application::agent::ActiveRunError::AlreadyActive => ChatError::SessionRunActive,
            crate::application::agent::ActiveRunError::SessionArchived => ChatError::SessionArchived,
            _ => ChatError::StorageFailure,
        })?;
        if self.execution_memory.as_ref().map(|memory| memory.get_active_goal(session_code)).transpose()
            .map_err(|_| ChatError::StorageFailure)?.flatten().is_some_and(|goal| goal.goal.status == "ACTIVE") {
            return Err(ChatError::SessionRunActive);
        }
        let session = self.repository().get_session(session_code)?;
        if session.status != ChatSessionStatus::Active { return Err(ChatError::SessionArchived); }
        if session.source_type != ChatSessionSourceType::Manual
            || session.source_code.as_deref().is_some_and(|source| source.starts_with("__assistant:") || source.starts_with("__knowledge:")) {
            return Err(ChatError::WorkspaceForbidden);
        }
        if session.directory_code.is_some() && working.as_ref().is_some_and(|value| value != &session.directory_code) {
            return Err(ChatError::DirectoryFixed);
        }
        let project = placement.clone().unwrap_or_else(|| session.project_code.clone());
        let directory = if placement.is_some() { None } else { session.directory_code.clone() };
        let mut working_directory = working.unwrap_or_else(|| session.working_directory_code.clone().or(session.directory_code.clone()));
        if working_directory.is_none() && directory.is_some() { working_directory = directory.clone(); }
        if let Some(code) = project.as_deref() {
            self.projects.as_ref().ok_or(ChatError::ProjectInvalid)?.resolve_execution_context(&session.workspace_code, code).map_err(|_| ChatError::ProjectInvalid)?;
        }
        if let Some(code) = working_directory.as_deref() {
            self.directories.as_ref().ok_or(ChatError::DirectoryInvalid)?.resolve(&session.workspace_code, code).map_err(|_| ChatError::DirectoryInvalid)?;
        }
        let reference = format!("TOPIC-REF-{}", self.id_provider.next_id().map_err(|_| ChatError::StorageFailure)?.value());
        self.repository().update_session_location(session_code, project.as_deref(), directory.as_deref(), working_directory.as_deref(), &reference)?;
        self.repository().get_session(session_code)
    }

    pub fn list_messages(&self, session_code: &str) -> Result<Vec<ChatMessage>, ChatError> {
        self.list_messages_with_actions(session_code, None)
    }

    /// Public message list with delivery actions intersected with what the
    /// requesting client can render. `None` means the portable set.
    pub fn list_messages_with_actions(
        &self,
        session_code: &str,
        actions: Option<&[crate::domain::chat::ArtifactAction]>,
    ) -> Result<Vec<ChatMessage>, ChatError> {
        self.repository().get_session(session_code)?;
        let mut messages = self.repository().list_messages(session_code)?;
        // Delivery records are stored privately per run; the public list is a
        // projection and never reads paths from the persisted message.
        let mut artifacts = self.repository().artifacts_by_session(session_code)?;
        let effective = actions.unwrap_or(&crate::domain::chat::ArtifactAction::PORTABLE);
        for message in &mut messages {
            let Some(run_code) = message.run_code.as_deref() else {
                continue;
            };
            if message.role != ChatMessageRole::Assistant {
                continue;
            }
            if let Some(records) = artifacts.remove(run_code) {
                message.artifacts =
                    crate::domain::chat::ChatArtifacts::from_records_with_actions(&records, effective);
            }
        }
        Ok(messages)
    }

    /// Private delivery records of one session, keyed by run code.
    pub fn artifacts_by_session(
        &self,
        session_code: &str,
    ) -> Result<
        std::collections::BTreeMap<String, Vec<crate::domain::chat::ArtifactRecord>>,
        ChatError,
    > {
        self.repository().get_session(session_code)?;
        self.repository().artifacts_by_session(session_code)
    }

    pub fn list_user_input_requests(
        &self,
        session_code: &str,
    ) -> Result<Vec<ChatUserInputRequest>, ChatError> {
        self.repository().get_session(session_code)?;
        self.repository().list_user_input_requests(session_code)
    }

    /// Product session/model mapping only; native schema and answer CAS live in the SDK.
    pub fn native_resume_command(
        &self,
        session_code: &str,
        entries: Vec<crate::application::chat::ChatResumeEntry>,
        model_code: Option<String>,
        reasoning_effort: Option<crate::domain::reasoning::ReasoningEffort>,
    ) -> Result<SendChatMessageCommand, ChatError> {
        let session = self.repository().get_session(session_code)?;
        Ok(SendChatMessageCommand {
            annotations: Vec::new(),
            requested_skills: Vec::new(),
            a2ui_action_id: None, a2ui_metadata: None,
            run_code: None,
            content: None,
            workspace_code: Some(session.workspace_code),
            app_code: session.app_code,
            task_code: session.task_code,
            agent_code: session.agent_code,
            model_selection: Self::continuation_model_selection(model_code.as_deref()),
            model_code,
            system_model_id: None,
            reasoning_effort,
            page_context: None,
            limits: None,
            tools: Vec::new(),
            attachments: Vec::new(),
            client_capabilities: Some(crate::domain::chat::ChatClientCapabilities {
                user_input_requests: true,
            a2ui: None,
            artifacts: None,
        }),
            resume: entries,
        })
    }

    fn continuation_model_selection(model_code: Option<&str>) -> Option<ModelSelection> {
        let model_code = model_code?;
        if model_code == "PERSON_AI:ROUTE:ChatAuto" {
            return Some(ModelSelection::auto());
        }
        let fixed = model_code.strip_prefix("PERSON_AI:FIXED:")?;
        let (quality, catalog_model_code) = fixed.split_once(':')?;
        Some(ModelSelection::person_ai_fixed(
            catalog_model_code,
            ProviderModelQuality::from_storage(quality)?,
        ))
    }

    pub fn steer_message(
        &self,
        command: SteerChatMessageCommand,
    ) -> Result<ChatSteerResult, ChatSteerError> {
        let session_code =
            normalized_required(command.session_code).ok_or(ChatSteerError::SessionCodeRequired)?;
        let expected_run_code = normalized_required(command.expected_run_code)
            .ok_or(ChatSteerError::ExpectedRunCodeRequired)?;
        eliteforge_agent_core::validate_annotations(&command.annotations).map_err(|_| ChatSteerError::Chat(ChatError::AnnotationsInvalid))?;
        let annotations = command.annotations;
        let content = normalized_optional(command.content).unwrap_or_default();
        let attachments =
            normalize_chat_attachments(command.attachments).map_err(ChatSteerError::Chat)?;
        if content.is_empty() && attachments.is_empty() && annotations.is_empty() {
            return Err(ChatSteerError::Chat(ChatError::MessageRequired));
        }
        let session = self
            .repository()
            .get_session(&session_code)
            .map_err(ChatSteerError::Chat)?;
        if session.status != ChatSessionStatus::Active { return Err(ChatSteerError::Chat(ChatError::SessionArchived)); }
        self.repository().validate_annotation_sources(&session_code, &annotations).map_err(ChatSteerError::Chat)?;
        let agent_code = session.agent_code.as_deref().unwrap_or(MAIN_AGENT_CODE);
        let requested_skills = self.authorize_requested_skills(agent_code, &command.requested_skills).map_err(ChatSteerError::Chat)?;
        let goal = super::composer_input::goal_objective(&content).map_err(ChatSteerError::Chat)?;
        if goal.is_some() { self.ensure_goal_agent(agent_code).map_err(ChatSteerError::Chat)?; }
        let (attachments, invocation_attachments) = self
            .prepare_session_attachments(&session, attachments)
            .map_err(ChatSteerError::Chat)?;
        let prompt = if !annotations.is_empty() { content.clone() } else { agent_prompt_for_content(&content) };
        let channel = if session.task_code.is_some() {
            ChatMessageChannel::TaskAgent
        } else {
            ChatMessageChannel::MainAgent
        };

        match self
            .active_runs
            .accept_steer(&session_code, &expected_run_code, || {
                // Admission is fenced by the active-run lock before command side effects.
                if let Some(objective) = goal { self.apply_goal_command(&session_code, objective, crate::shared::auth_holder::AuthHolder::get().map(|auth| auth.user_code().to_string())).map_err(ChatSendError::Chat)?; }
                let message = self.create_message_with_skills(
                    &session_code,
                    Some(expected_run_code.clone()),
                    ChatMessageRole::User,
                    channel,
                    "User",
                    &content,
                    attachments.clone(),
                    None,
                    requested_skills.clone(),
                    annotations.clone(),
                )?;
                let input = AgentSteerInput {
                    page_context: crate::application::agent::page_context::normalized(command.page_context.as_ref()),
                    annotations: annotations.clone(),
                    steer_code: message.message_code.clone(),
                    requested_skills: requested_skills.clone(),
                    content: prompt.clone(),
                    attachments: invocation_attachments.clone(),
                };
                Ok::<_, ChatSendError>((
                    ChatSteerResult {
                        session_code: session_code.clone(),
                        run_code: expected_run_code.clone(),
                        message,
                    },
                    input,
                ))
            }) {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(ChatSendError::Chat(error))) => Err(ChatSteerError::Chat(error)),
            Ok(Err(ChatSendError::Agent(_) | ChatSendError::RunAlreadyActive { .. })) => {
                Err(ChatSteerError::RegistryUnavailable)
            }
            Err(error) => Err(ChatSteerError::from(error)),
        }
    }
}
