mod settings;
pub use settings::KnowledgeLibrarySettings;
mod agent_grants;
pub(crate) mod chunk_context;
pub mod deletion;
mod document_configuration;
mod document_intake;
pub mod fidelity; // Legacy finding types and read guards for persisted documents.
pub(crate) mod image_index;
pub mod ingestion;
pub(crate) mod ingestion_worker;
pub mod local_documents;
mod managed_retrieval;
pub mod office_visuals;
pub mod parsing;
mod parsing_cache;
pub use crate::application::document::preflight::{DocumentParsePlan, DocumentParsePreference};
pub mod pdf_visual_source;
mod platform_catalog;
mod publication_resources;
mod source_locations;
mod visual_evidence;
pub use visual_evidence::{KnowledgeBaseVisualContext, KnowledgeBaseVisualEvidence};
pub mod image_descriptions;
use crate::shared::i18n_message::{MessageText, StoredMessage};
use crate::{
    adapters::outbound::persistence::sqlite::SqliteKnowledgeBaseRepository,
    app::AppRagService,
    application::{
        agent::AgentManagementService,
        llm::LlmRuntimeService,
        rag::*,
        resource::{
            ResourceBinaryDownloadCommand, ResourceCommitBinary, ResourceCreateCommand,
            ResourceService,
        },
    },
    domain::{rag::*, resource::ResourceKind},
    shared::sqlite_extensions::TRON_VECTOR_INDEX_DIMENSION,
};
use local_documents::{ChunkConfig, ImageAnalysisSummary, LocatedChunk, parse_local_document};
use parsing::{
    KnowledgeDocumentParser, OcrMode, ParsingArtifact, ParsingCapabilities, ParsingTier,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::{collections::BTreeMap, io::Write};
#[derive(Debug, Clone)]
pub enum KnowledgeBaseError {
    NotFound,
    Forbidden,
    Conflict,
    NotEmpty,
    /// User-facing validation failure. Carries a locale-independent message;
    /// display layers call [`KnowledgeBaseError::localized`].
    Invalid(StoredMessage),
    /// Storage/provider failure. Carries a locale-independent message; the
    /// plain variant is reserved for technical detail that has no stable key.
    Storage(StoredMessage),
}
impl KnowledgeBaseError {
    pub fn invalid(key: &'static str) -> Self {
        Self::Invalid(StoredMessage::message(MessageText::new(key)))
    }
    pub fn invalid_message(message: MessageText) -> Self {
        Self::Invalid(StoredMessage::message(message))
    }
    pub fn storage(key: &'static str) -> Self {
        Self::Storage(StoredMessage::message(MessageText::new(key)))
    }
    pub fn storage_message(message: MessageText) -> Self {
        Self::Storage(StoredMessage::message(message))
    }
    /// Technical detail (I/O, SQL, provider failures) that has no stable key.
    pub fn storage_detail(detail: impl std::fmt::Display) -> Self {
        Self::Storage(StoredMessage::plain(detail.to_string()))
    }
    /// Convert an error/detail that may already carry an encoded structured
    /// message; plain values become technical detail for the display layer.
    pub fn from_detail(error: impl std::fmt::Display) -> Self {
        let text = error.to_string();
        match MessageText::decode(&text) {
            Some(message) => Self::invalid_message(message),
            None => Self::Invalid(StoredMessage::plain(text)),
        }
    }
    /// Locale-independent payload for persistence (key + arguments only).
    pub fn persisted_message(&self) -> Option<&StoredMessage> {
        match self {
            Self::Invalid(message) | Self::Storage(message) => Some(message),
            _ => None,
        }
    }
    /// Stable translation key, when the error carries a structured message.
    pub fn message_key(&self) -> Option<&str> {
        match self {
            Self::Invalid(message) | Self::Storage(message) => message.message_key(),
            _ => None,
        }
    }
    /// Locale-independent persisted form for stored/background diagnostics.
    pub fn persisted(&self) -> String {
        match self.persisted_message() {
            Some(message) => message.encode(),
            None => {
                let key = match self {
                    Self::NotFound => "knowledge.base.document_not_found",
                    Self::Forbidden => "tron.knowledge_base.forbidden",
                    Self::Conflict => "knowledge.base.document_conflict",
                    Self::NotEmpty => "tron.knowledge_base.not_empty",
                    Self::Invalid(_) | Self::Storage(_) => unreachable!(),
                };
                MessageText::new(key).encode()
            }
        }
    }
    /// Display-layer rendering. The request/session locale is always explicit.
    pub fn localized(&self, locale: &str) -> String {
        use tron_i18n::t;
        match self {
            Self::NotFound => t!("knowledge.base.document_not_found", locale = locale).into_owned(),
            Self::Forbidden => t!("tron.knowledge_base.forbidden", locale = locale).into_owned(),
            Self::Conflict => t!("knowledge.base.document_conflict", locale = locale).into_owned(),
            Self::NotEmpty => t!("tron.knowledge_base.not_empty", locale = locale).into_owned(),
            Self::Invalid(message) => message.render(locale, Some("knowledge.base.invalid_detail")),
            Self::Storage(message) => {
                message.render(locale, Some("knowledge.base.storage_failure"))
            }
        }
    }
}
impl std::fmt::Display for KnowledgeBaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Diagnostics only; user-facing output must call `localized(locale)`.
        match self {
            Self::NotFound => f.write_str("knowledge.base.document_not_found"),
            Self::Forbidden => f.write_str("tron.knowledge_base.forbidden"),
            Self::Conflict => f.write_str("knowledge.base.document_conflict"),
            Self::NotEmpty => f.write_str("tron.knowledge_base.not_empty"),
            Self::Invalid(message) | Self::Storage(message) => {
                f.write_str(&message.render_diagnostic())
            }
        }
    }
}
impl std::error::Error for KnowledgeBaseError {}
impl From<crate::application::document::LocalDocumentError> for KnowledgeBaseError {
    fn from(error: crate::application::document::LocalDocumentError) -> Self {
        Self::invalid_message(error.0)
    }
}
impl From<RagError> for KnowledgeBaseError {
    fn from(e: RagError) -> Self {
        Self::Storage(StoredMessage::message(MessageText::new(rag_error_key(&e))))
    }
}
fn rag_error_key(error: &RagError) -> &'static str {
    match error {
        RagError::SchemaIncompatible => "rag.error.schema_incompatible",
        RagError::CorpusNotFound => "rag.error.ingest_request_invalid",
        RagError::SourceNotFound => "rag.error.source_not_found",
        RagError::IngestRequestInvalid => "rag.error.ingest_request_invalid",
        RagError::IngestScopeInvalid => "rag.error.ingest_scope_invalid",
        RagError::EmbeddingFailed | RagError::EmbeddingFailure(_) => "rag.error.embedding_failed",
        RagError::SearchRequestInvalid => "rag.error.search_request_invalid",
        RagError::SearchFilterInvalid => "rag.error.search_filter_invalid",
        RagError::SearchScopeInvalid => "rag.error.search_scope_invalid",
        RagError::SearchRouteUnavailable => "rag.error.search_route_unavailable",
        RagError::DebugRequestInvalid => "rag.error.debug_request_invalid",
        RagError::TracePayloadInvalid => "rag.error.trace_payload_invalid",
        RagError::StorageBindingInvalid => "rag.error.ingest_request_invalid",
        RagError::StorageFailure => "rag.error.storage_failed",
    }
}
impl From<crate::domain::resource::ResourceError> for KnowledgeBaseError {
    fn from(e: crate::domain::resource::ResourceError) -> Self {
        use crate::domain::resource::ResourceError;
        match e {
            ResourceError::NotFound => Self::NotFound,
            ResourceError::Forbidden => Self::Forbidden,
            ResourceError::Conflict => Self::Conflict,
            ResourceError::InvalidInput => Self::invalid("resource.error.invalid_input"),
            ResourceError::InvalidCode => Self::invalid("resource.error.invalid_code"),
            ResourceError::InvalidWorkspace => Self::invalid("resource.error.invalid_workspace"),
            ResourceError::WorkspaceRequired => Self::invalid("resource.error.workspace_required"),
            ResourceError::InvalidVariableReference => {
                Self::invalid("resource.error.invalid_variable_reference")
            }
            _ => Self::storage_detail(e.to_string()),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub owner_user_code: String,
    pub default_chunk_config: ChunkConfig,
    #[serde(default)]
    pub default_parse_preference: DocumentParsePreference,
    #[serde(default)]
    pub documents: Vec<KnowledgeBaseDocument>,
    #[serde(default)]
    pub document_count: usize,
    #[serde(default)]
    pub agent_codes: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBaseDocument {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub stage: Option<ingestion::IngestionStage>,
    #[serde(default)]
    pub wait_reason: Option<ingestion::IngestionWaitReason>,
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub resource_code: String,
    pub resource_revision: u64,
    pub chunk_config: Option<ChunkConfig>,
    pub status: String,
    pub error: Option<String>,
    pub chunk_count: usize,
    pub index_revision: u64,
    pub revision: u64,
    pub active_source_code: Option<String>,
    pub embedding_model_code: Option<String>,
    pub embedding_dimension: Option<usize>,
    #[serde(default)]
    pub embedding_provider_model: Option<String>,
    #[serde(default)]
    pub embedding_space_fingerprint: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_analysis: Option<ImageAnalysisSummary>,
    #[serde(default)]
    pub parsing_tier: Option<ParsingTier>,
    #[serde(default)]
    pub ocr_mode: Option<OcrMode>,
    #[serde(default)]
    pub parse_plan: Option<DocumentParsePlan>,
    #[serde(default)]
    pub parsing_artifact: Option<ParsingArtifact>,
    #[serde(default)]
    pub derived_resource_codes: Vec<String>,
    #[serde(default)]
    pub visual_source: Option<pdf_visual_source::PdfVisualSource>,
    #[serde(default)]
    pub image_description_artifacts: Vec<image_descriptions::ImageDescriptionArtifact>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBasePreview {
    pub fidelity_issues: Vec<fidelity::FidelityIssue>,
    pub chunk_count: usize,
    pub chunks: Vec<LocatedChunk>,
    pub warnings: Vec<String>,
    pub image_analysis: Option<ImageAnalysisSummary>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBaseHit {
    pub knowledge_base_id: String,
    pub document_id: String,
    pub resource_code: String,
    pub resource_revision: u64,
    pub source_code: String,
    pub chunk_code: String,
    pub title: String,
    pub text: String,
    pub location: Value,
    pub score: f64,
}
#[derive(Debug, Clone)]
pub struct KnowledgeBaseContext {
    pub text: String,
    pub source_version: String,
    pub hits: Vec<KnowledgeBaseHit>,
    /// Locale-independent structured warnings from the retrieval stage.
    pub warnings: Vec<String>,
}
#[derive(Clone)]
pub struct KnowledgeBaseService {
    pub(crate) quality_jobs: Arc<crate::application::knowledge_quality::QualityJobs>,
    pub(crate) quality_questions:
        Option<Arc<dyn crate::application::knowledge_quality::QualityQuestionGenerator>>,
    pub(crate) chunk_strategies: eliteforge_agent_core::ChunkStrategies,
    skills: Arc<std::sync::OnceLock<crate::application::skill::SkillRegistryService>>,
    pub(crate) cleanup_gate: Arc<std::sync::Mutex<()>>,
    pub(crate) queue: Arc<dyn ingestion::KnowledgeIngestionQueue>,
    ingestion_config: crate::config::KnowledgeIngestionConfig,
    workers: Arc<ingestion_worker::IngestionWorkers>,
    pub(crate) parse_checkpoint: Option<Arc<dyn parsing::ParseCheckpoint>>,
    pub(crate) repository: SqliteKnowledgeBaseRepository,
    pub(crate) rag: AppRagService,
    pub(crate) resources: ResourceService,
    agents: AgentManagementService,
    pub(crate) llm: LlmRuntimeService,
    pub(crate) parser: Option<Arc<dyn KnowledgeDocumentParser>>,
    office_renderer: Option<Arc<dyn parsing::KnowledgeOfficeRenderer>>,
    pub(crate) image_description: crate::config::ImageDescriptionConfig,
    visual_context_enabled: bool,
    pub(crate) runtime_config: Option<crate::application::system_config::BackendRuntimeConfig>,
}
impl KnowledgeBaseService {
    pub fn bind_skills(&self, skills: crate::application::skill::SkillRegistryService) {
        let _ = self.skills.set(skills);
    }
    fn allowed_skill_sources(&self, agent: &str) -> Result<Vec<String>, RagError> {
        let Some(skills) = self.skills.get() else {
            return Ok(vec![]);
        };
        Ok(skills
            .effective_agent_skill_names(agent)
            .map_err(|_| RagError::SearchScopeInvalid)?
            .into_iter()
            .map(|name| format!("skill:PLATFORM:{name}"))
            .collect())
    }

    pub fn new(
        repository: SqliteKnowledgeBaseRepository,
        rag: AppRagService,
        resources: ResourceService,
        agents: AgentManagementService,
        llm: LlmRuntimeService,
    ) -> Self {
        let queue = Arc::new(
            repository
                .ingestion_queue()
                .expect("knowledge ingestion schema"),
        );
        Self {
            quality_jobs: Default::default(),
            quality_questions: None,
            chunk_strategies: eliteforge_agent_capability_chunking::built_in_chunk_strategies(),
            skills: Default::default(),
            cleanup_gate: Default::default(),
            queue,
            ingestion_config: Default::default(),
            workers: Default::default(),
            parse_checkpoint: None,
            repository,
            rag,
            resources,
            agents,
            llm,
            parser: None,
            office_renderer: None,
            image_description: Default::default(),
            visual_context_enabled: false,
            runtime_config: None,
        }
    }
    pub fn with_chunk_strategies(
        mut self,
        strategies: eliteforge_agent_core::ChunkStrategies,
    ) -> Self {
        self.chunk_strategies = strategies;
        self
    }
    pub fn with_quality_question_generator(
        mut self,
        generator: Arc<dyn crate::application::knowledge_quality::QualityQuestionGenerator>,
    ) -> Self {
        self.quality_questions = Some(generator);
        self
    }
    pub fn chunking_strategies(&self) -> Vec<eliteforge_agent_core::ChunkStrategyDescriptor> {
        self.chunk_strategies.descriptors()
    }
    /// Disabled by default; enabling allows authorized KB image bytes to reach the answer model.
    pub fn with_visual_context(mut self, enabled: bool) -> Self {
        self.visual_context_enabled = enabled;
        self
    }
    pub fn visual_context_enabled(&self) -> bool {
        self.runtime_config
            .as_ref()
            .map(|runtime| runtime.snapshot().mineru.visual_context_enabled)
            .unwrap_or(self.visual_context_enabled)
    }
    pub fn with_runtime_config(
        mut self,
        runtime: crate::application::system_config::BackendRuntimeConfig,
    ) -> Self {
        self.runtime_config = Some(runtime);
        self
    }
    fn image_description_config(&self) -> crate::config::ImageDescriptionConfig {
        self.runtime_config
            .as_ref()
            .map(|runtime| runtime.snapshot().mineru.image_description)
            .unwrap_or_else(|| self.image_description.clone())
    }
    fn active_ingestion_config(&self) -> crate::config::KnowledgeIngestionConfig {
        self.runtime_config
            .as_ref()
            .map(|runtime| runtime.snapshot().mineru.ingestion)
            .unwrap_or_else(|| self.ingestion_config.clone())
    }
    pub fn with_image_descriptions(
        mut self,
        config: crate::config::ImageDescriptionConfig,
    ) -> Self {
        self.image_description = config;
        self
    }
    pub fn with_office_renderer(
        mut self,
        renderer: Arc<dyn parsing::KnowledgeOfficeRenderer>,
    ) -> Self {
        self.office_renderer = Some(renderer);
        self
    }
    pub fn with_document_parser(mut self, parser: Arc<dyn KnowledgeDocumentParser>) -> Self {
        self.parser = Some(parser);
        self
    }
    pub async fn parsing_capabilities(
        &self,
        user: &str,
    ) -> Result<ParsingCapabilities, KnowledgeBaseError> {
        self.require_user(user)?;
        self.parser
            .as_ref()
            .ok_or_else(|| {
                KnowledgeBaseError::invalid("knowledge.base.parse_service_unconfigured")
            })?
            .capabilities()
            .await
    }
    pub(crate) fn require_user(&self, user: &str) -> Result<(), KnowledgeBaseError> {
        if user.trim().is_empty() {
            return Err(KnowledgeBaseError::Forbidden);
        }
        Ok(())
    }
    pub(crate) fn download_resource(
        &self,
        user: &str,
        id: &str,
        code: &str,
    ) -> Result<(std::fs::File, u64), KnowledgeBaseError> {
        self.owned(user, id)?;
        let mut reachable = self
            .repository
            .library_derived_resources(id)?
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        for document in self.repository.documents(id)? {
            deletion::collect_codes(
                &serde_json::to_value(&document).map_err(invalid_storage)?,
                &mut reachable,
            );
            reachable.insert(document.resource_code);
        }
        if !reachable.contains(code) {
            return Err(KnowledgeBaseError::NotFound);
        }
        let (download, record) = self.resources.download_binary_to_file(
            user,
            ResourceBinaryDownloadCommand {
                workspace_code: "PLATFORM".into(),
                resource_code: code.into(),
            },
        )?;
        let mut file = tempfile::tempfile().map_err(invalid_storage)?;
        self.resources.download_binary_storage(&record, &mut file)?;
        std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0)).map_err(invalid_storage)?;
        Ok((file, download.byte_size))
    }
    pub(crate) fn owned(&self, user: &str, id: &str) -> Result<KnowledgeBase, KnowledgeBaseError> {
        self.require_user(user)?;
        if !self.repository.owns(user, id) {
            return Err(KnowledgeBaseError::NotFound);
        }
        self.repository.get(id)
    }
    pub fn detail(&self, user: &str, id: &str) -> Result<KnowledgeBase, KnowledgeBaseError> {
        let mut kb = self.owned(user, id)?;
        kb.documents = self.repository.documents(id)?;
        let document_ids = kb
            .documents
            .iter()
            .map(|doc| doc.id.clone())
            .collect::<Vec<_>>();
        let latest_tasks = self
            .queue
            .latest_for_library(id, &document_ids)
            .map_err(ingestion_worker::queue_error)?;
        for doc in &mut kb.documents {
            if let Some(task) = latest_tasks.get(&doc.id) {
                doc.task_id = Some(task.id.clone());
                doc.stage = Some(task.stage);
                doc.wait_reason = task.wait_reason.clone();
            }
        }
        kb.document_count = kb.documents.len();
        kb.agent_codes = self.repository.grants(id)?;
        Ok(kb)
    }
    pub fn published_chunks(
        &self,
        user: &str,
        id: &str,
        document: &str,
        page: usize,
        page_size: usize,
    ) -> Result<RagSourceUnitsPage, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        let _guard = self.cleanup_gate.lock().map_err(invalid_storage)?;
        let kb = self.owned(user, id)?;
        let doc = self.repository.document(&kb.id, document)?;
        if page == 0 || page_size == 0 || page_size > 100 || page > 1_000_000 {
            return Err(KnowledgeBaseError::invalid(
                "knowledge.base.chunk_pagination_invalid",
            ));
        }
        let Some(source) = doc.active_source_code else {
            return Ok(RagSourceUnitsPage {
                items: vec![],
                total: 0,
                page,
                page_size,
            });
        };
        self.rag
            .repository_handle()
            .source_units_page(
                RagDebugSourceRequest {
                    workspace_code: namespace.into(),
                    corpus_id: kb.id,
                    source_code: source,
                    source_version: "v1".into(),
                    scope_type: "SPACE".into(),
                    scope_code: namespace.into(),
                    include_text_units: true,
                    include_warnings: false,
                },
                page,
                page_size,
            )
            .map_err(Into::into)
    }

    pub fn create(
        &self,
        user: &str,
        name: &str,
        description: &str,
    ) -> Result<KnowledgeBase, KnowledgeBaseError> {
        self.create_with_index_settings(
            user,
            name,
            description,
            RagSettings::knowledge_base_default(),
        )
    }

    pub fn create_with_index_settings(
        &self,
        user: &str,
        name: &str,
        description: &str,
        index_settings: RagSettings,
    ) -> Result<KnowledgeBase, KnowledgeBaseError> {
        self.create_with_parsing_settings(
            user,
            name,
            description,
            index_settings,
            DocumentParsePreference::Auto,
        )
    }
    pub fn create_with_parsing_settings(
        &self,
        user: &str,
        name: &str,
        description: &str,
        index_settings: RagSettings,
        default_parse_preference: DocumentParsePreference,
    ) -> Result<KnowledgeBase, KnowledgeBaseError> {
        self.require_user(user)?;
        // Validate before inserting the library: no partially configured draft on invalid input.
        index_settings
            .validate()
            .map_err(|_| KnowledgeBaseError::invalid("knowledge.base.index_settings_invalid"))?;
        if index_settings.graph && index_settings.graph_model_code.is_none() {
            return Err(KnowledgeBaseError::invalid(
                "knowledge.base.graph_model_required",
            ));
        }
        if name.trim().is_empty() || name.chars().count() > 120 {
            return Err(KnowledgeBaseError::invalid("knowledge.base.name_invalid"));
        }
        let id = format!("kb_{}", new_id());
        let kb = KnowledgeBase {
            id: id.clone(),
            name: name.trim().into(),
            description: description.trim().into(),
            owner_user_code: user.into(),
            default_chunk_config: ChunkConfig::default(),
            default_parse_preference,
            documents: vec![],
            document_count: 0,
            agent_codes: vec![],
        };
        // Ownership exists before corpus publication, so generic RAG never observes an unprotected KB.
        self.repository.insert(&kb, user)?;
        let table = format!("rag_{}_units", id);
        let target = RagStorageSegmentTarget {
            id: "documents".into(),
            backend: RagStorageBackend::Sqlite,
            layout: "EMBEDDED".into(),
            table: Some(table),
            namespace: None,
            node_label: None,
            code_strategy: Some("CORPUS_SOURCE_VERSION_SCOPE_HASH".into()),
        };
        let mapping: BTreeMap<String, String> = [
            ("sourceCode", "source_code"),
            ("sourceVersion", "source_version"),
            ("scopeType", "scope_type"),
            ("scopeCode", "scope_code"),
            ("status", "source_status"),
            ("title", "title"),
            ("metadata", "source_metadata"),
            ("text", "text"),
            ("textUnitStatus", "status"),
        ]
        .into_iter()
        .map(|(a, b)| (a.into(), b.into()))
        .collect();
        let corpus = RagCorpusRecord {
            corpus_id: id.clone(),
            display_name: kb.name.clone(),
            description: Some(kb.description.clone()),
            status: RagStatus::Enabled,
            capabilities: json!({"fulltext":true,"vector":true,"tree":false,"graph":true,"settings":index_settings}),
            filter_schema: json!({"fields":{"sourceCode":{"type":"string","column":"source_code","operators":["eq","in"]}}}),
            field_mapping: mapping.clone(),
            source_field_mapping: mapping
                .iter()
                .filter(|(k, _)| k.as_str() != "text" && k.as_str() != "textUnitStatus")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            text_unit_field_mapping: mapping
                .iter()
                .filter(|(k, _)| {
                    matches!(
                        k.as_str(),
                        "sourceCode"
                            | "sourceVersion"
                            | "scopeType"
                            | "scopeCode"
                            | "text"
                            | "textUnitStatus"
                    )
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            storage_binding: RagStorageBinding {
                segment_bindings: ["source", "textUnit", "fulltext", "vector"]
                    .into_iter()
                    .map(|s| (s.into(), "documents".into()))
                    .collect(),
                segment_targets: BTreeMap::from([("documents".into(), target)]),
            },
        };
        self.rag.repository_handle().save_managed_corpus(&corpus)?;
        Ok(kb)
    }
    pub fn upload(
        &self,
        user: &str,
        kb_id: &str,
        filename: &str,
        bytes: &[u8],
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        self.upload_original(user, namespace, kb_id, filename, bytes, false)
    }
    pub(super) fn upload_original(
        &self,
        user: &str,
        namespace: &str,
        kb_id: &str,
        filename: &str,
        bytes: &[u8],
        request_ingestion: bool,
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        self.upload_original_with_plan(
            user,
            namespace,
            kb_id,
            filename,
            bytes,
            request_ingestion,
            None,
            None,
        )
    }
    pub(super) fn upload_original_with_plan(
        &self,
        user: &str,
        namespace: &str,
        kb_id: &str,
        filename: &str,
        bytes: &[u8],
        request_ingestion: bool,
        preference: Option<DocumentParsePreference>,
        confirmed: Option<&DocumentParsePlan>,
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let plan = self.preflight_document(user, kb_id, filename, bytes, preference)?;
        if confirmed.is_some_and(|confirmed| confirmed != &plan) {
            return Err(KnowledgeBaseError::invalid(
                "knowledge.base.parse_choice_changed",
            ));
        }
        // Serialize original creation across service clones; stable content IDs also
        // prevent duplicate persisted documents after request replay or restart.
        static UPLOAD_GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _upload_guard = UPLOAD_GATE.lock().map_err(invalid_storage)?;
        crate::domain::resource::validate_binary_name(filename, 255)?;
        if bytes.len() > 20 * 1024 * 1024 {
            return Err(KnowledgeBaseError::invalid("knowledge.base.file_too_large"));
        }
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !crate::application::document::preflight::PARSE_EXTENSIONS.contains(&ext.as_str())
            && !crate::application::document::preflight::DIRECT_READ_EXTENSIONS
                .contains(&ext.as_str())
        {
            return Err(KnowledgeBaseError::invalid(
                "knowledge.base.format_unsupported",
            ));
        }
        let id = format!(
            "doc_{}",
            hash(
                &serde_json::to_string(&(
                    user,
                    namespace,
                    kb_id,
                    filename,
                    format!("{:x}", Sha256::digest(bytes))
                ))
                .map_err(invalid_storage)?
            )
        );
        if let Ok(existing) = self.repository.document(kb_id, &id) {
            if existing
                .parse_plan
                .as_ref()
                .is_some_and(|stored| stored.method != plan.method)
            {
                return Err(KnowledgeBaseError::invalid("knowledge.base.duplicate_file"));
            }
            if request_ingestion {
                self.repository.request_ingestion(kb_id, &existing)?;
            }
            return self.repository.document(kb_id, &id);
        }
        let resource = format!("KB_{}", new_id());
        self.repository.mark_private(user, namespace, &resource)?;
        let mut temporary = tempfile::NamedTempFile::new().map_err(invalid_storage)?;
        temporary.write_all(bytes).map_err(invalid_storage)?;
        self.resources.create_file_resource(
            user,
            ResourceCreateCommand {
                workspace_code: namespace.into(),
                resource_code: resource.clone(),
                kind: if ext == "pdf" {
                    ResourceKind::Pdf
                } else {
                    ResourceKind::File
                },
                name: filename.into(),
                description: "知识库原件".into(),
                allow_inheritance: false,
            },
            Some(ResourceCommitBinary {
                source: temporary.reopen().map_err(invalid_storage)?,
                file_name: filename.into(),
            }),
        )?;
        let (_, stored) = self.resources.download_binary_to_file(
            user,
            ResourceBinaryDownloadCommand {
                workspace_code: namespace.into(),
                resource_code: resource.clone(),
            },
        )?;
        let mut doc = KnowledgeBaseDocument {
            task_id: None,
            stage: None,
            wait_reason: None,
            id,
            name: filename.into(),
            file_name: filename.into(),
            resource_code: resource,
            resource_revision: stored.revision,
            chunk_config: None,
            status: if request_ingestion {
                "QUEUED"
            } else {
                "UPLOADED"
            }
            .into(),
            error: None,
            chunk_count: 0,
            index_revision: 0,
            revision: 0,
            active_source_code: None,
            embedding_model_code: None,
            embedding_dimension: None,
            embedding_provider_model: None,
            embedding_space_fingerprint: None,
            warnings: vec![],
            image_analysis: None,
            parsing_tier: None,
            ocr_mode: None,
            parse_plan: None,
            parsing_artifact: None,
            derived_resource_codes: vec![],
            visual_source: None,
            image_description_artifacts: vec![],
        };
        document_intake::apply_plan(&mut doc, plan);
        self.repository
            .insert_document_with_intent(kb_id, &doc, request_ingestion)?;
        Ok(doc)
    }
    /// Product upload uses the existing per-document index lifecycle; no extra job layer.
    pub async fn upload_and_index(
        &self,
        user: &str,
        kb: &str,
        filename: &str,
        bytes: &[u8],
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        let doc = self.upload_original(user, namespace, kb, filename, bytes, true)?;
        self.index(user, kb, &doc.id).await
    }
    /// Saving document settings automatically rebuilds only that document.
    pub async fn configure_and_index(
        &self,
        user: &str,
        kb: &str,
        doc: &str,
        config: Option<ChunkConfig>,
        tier: Option<Option<ParsingTier>>,
        ocr: Option<Option<OcrMode>>,
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        let task = self
            .enqueue_configured_index(user, namespace, kb, doc, config, tier, ocr)
            .await?;
        self.run_task_inline(&task.id).await?;
        self.document_with_task(user, kb, doc)
    }
    pub(crate) fn document_parser(
        &self,
        doc: &KnowledgeBaseDocument,
    ) -> Result<Option<Arc<dyn KnowledgeDocumentParser>>, KnowledgeBaseError> {
        if doc
            .parse_plan
            .as_ref()
            .is_some_and(|plan| plan.features.local_text)
        {
            return Ok(None);
        }
        match (&self.parser, doc.ocr_mode) {
            (Some(parser), Some(mode)) => parser.with_ocr_mode(mode).map(Some),
            (_, None) => Ok(self.parser.clone()),
            (None, Some(_)) => Err(KnowledgeBaseError::invalid(
                "knowledge.base.parse_service_unconfigured",
            )),
        }
    }
    pub(crate) async fn chunks(
        &self,
        user: &str,
        namespace: &str,
        kb: &KnowledgeBase,
        doc: &KnowledgeBaseDocument,
        generate_descriptions: bool,
    ) -> Result<
        (
            Vec<LocatedChunk>,
            Vec<eliteforge_agent_core::ChunkContext>,
            Vec<String>,
            KnowledgeBaseDocument,
        ),
        KnowledgeBaseError,
    > {
        let (_, record) = self.resources.download_binary_to_file(
            user,
            ResourceBinaryDownloadCommand {
                workspace_code: namespace.into(),
                resource_code: doc.resource_code.clone(),
            },
        )?;
        if record.revision != doc.resource_revision {
            return Err(KnowledgeBaseError::Conflict);
        }
        let mut bytes = Vec::new();
        self.resources
            .download_binary_storage(&record, &mut bytes)?;
        if let Some(checkpoint) = &self.parse_checkpoint {
            let task = checkpoint.task()?;
            if task.configuration["originalSha256"].as_str()
                != Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
            {
                return Err(KnowledgeBaseError::Conflict);
            }
        }
        let reuse_only = self
            .parse_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.task())
            .transpose()?
            .is_some_and(|task| task.configuration["reuseParsedOnly"] == true);
        let (parsed, parsed_doc) = if reuse_only {
            (self.existing_parsed_document(user, doc)?, doc.clone())
        } else {
            self.parsed_document(user, namespace, kb, doc, &bytes)
                .await?
        };
        if generate_descriptions && fidelity::blocked(&parsed_doc) {
            return Err(KnowledgeBaseError::invalid_message(
                fidelity::failure_message(&parsed_doc),
            ));
        }
        let (parsed, mut parsed_doc) = self
            .supplement_image_descriptions(
                user,
                namespace,
                kb,
                &parsed_doc,
                parsed,
                generate_descriptions,
            )
            .await?;
        parsed_doc.image_analysis = parsed.image_analysis.clone();
        if let Some(checkpoint) = &self.parse_checkpoint {
            checkpoint.chunking()?;
        }
        let mut chunked = crate::application::document::split_document_with_strategies(
            &parsed,
            doc.chunk_config
                .as_ref()
                .unwrap_or(&kb.default_chunk_config),
            &self.chunk_strategies,
            &doc.id,
            &doc.file_name,
        )
        .map_err(|e| KnowledgeBaseError::from_detail(e))?;
        let image_vectors = match self
            .parse_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.task())
            .transpose()?
        {
            Some(task) => match task.configuration["embeddingImages"].as_bool() {
                Some(value) => value,
                // Preserve image candidates without resolving an unavailable
                // embedding model in the parsing task. Indexing decides which
                // candidates to consume after the model identity is pinned.
                None if task.configuration["ragSettings"]["vector"] == true => true,
                None => false,
            },
            None => false,
        };
        if image_vectors {
            image_index::append_image_chunks(&parsed, &doc.file_name, &mut chunked.chunks)?;
        }
        chunked.warnings.extend(parsed.warnings);
        Ok((
            chunked.chunks,
            chunked.contexts,
            chunked.warnings,
            parsed_doc,
        ))
    }
    pub async fn preview(
        &self,
        user: &str,
        kb: &str,
        doc: &str,
    ) -> Result<KnowledgeBasePreview, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        let kb = self.owned(user, kb)?;
        let doc = self.repository.document(&kb.id, doc)?;
        let (chunks, _, warnings, parsed_doc) =
            self.chunks(user, namespace, &kb, &doc, false).await?;
        Ok(KnowledgeBasePreview {
            fidelity_issues: vec![], // Legacy response field; no interactive audit workflow.
            chunk_count: chunks.len(),
            chunks: chunks.into_iter().take(20).collect(),
            warnings,
            image_analysis: parsed_doc.image_analysis,
        })
    }
    pub fn set_grants(
        &self,
        user: &str,
        kb: &str,
        agents: Vec<String>,
    ) -> Result<KnowledgeBase, KnowledgeBaseError> {
        self.owned(user, kb)?;
        self.assign_library_agents(user, kb, &agents)?;
        self.detail(user, kb)
    }
    pub fn remove_document(
        &self,
        user: &str,
        kb: &str,
        doc: &str,
    ) -> Result<deletion::DocumentDeletion, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        self.start_ingestion_workers()?;
        self.request_document_deletion(user, namespace, kb, doc)
    }
    pub fn start_index(
        &self,
        user: &str,
        kb: &str,
        doc: &str,
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        self.enqueue_index(user, namespace, kb, doc)?;
        self.start_ingestion_workers()?;
        self.document_with_task(user, kb, doc)
    }
    pub fn upload_and_start_index(
        &self,
        user: &str,
        kb: &str,
        name: &str,
        bytes: &[u8],
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        let doc = self.upload_original(user, namespace, kb, name, bytes, true)?;
        let tasks = self
            .queue
            .for_document(&doc.id)
            .map_err(ingestion_worker::queue_error)?;
        if tasks.first().is_none() {
            return self.start_index(user, kb, &doc.id);
        }
        self.start_ingestion_workers()?;
        self.document_with_task(user, kb, &doc.id)
    }
    pub async fn configure_and_start_index(
        &self,
        user: &str,
        kb: &str,
        doc: &str,
        config: Option<ChunkConfig>,
        tier: Option<Option<ParsingTier>>,
        ocr: Option<Option<OcrMode>>,
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        self.enqueue_configured_index(user, namespace, kb, doc, config, tier, ocr)
            .await?;
        self.start_ingestion_workers()?;
        self.document_with_task(user, kb, doc)
    }
    async fn enqueue_configured_index(
        &self,
        user: &str,
        namespace: &str,
        kb: &str,
        doc: &str,
        config: Option<ChunkConfig>,
        tier: Option<Option<ParsingTier>>,
        ocr: Option<Option<OcrMode>>,
    ) -> Result<ingestion::IngestionTask, KnowledgeBaseError> {
        let document = self
            .prepare_parsing_configuration(user, kb, doc, config, tier, ocr)
            .await?;
        let library = self.owned(user, kb)?;
        let task = self.prepare_index_task(user, namespace, &library, &document)?;
        self.queue
            .enqueue_configured(task, document)
            .map_err(ingestion_worker::queue_error)
    }
    /// Awaited entrypoint uses exactly the same durable stages as background work.
    pub async fn index(
        &self,
        user: &str,
        kb: &str,
        doc: &str,
    ) -> Result<KnowledgeBaseDocument, KnowledgeBaseError> {
        let namespace = "PLATFORM";
        let task = self.enqueue_index(user, namespace, kb, doc)?;
        self.run_task_inline(&task.id).await?;
        self.document_with_task(user, kb, doc)
    }
    fn require_rebuild(
        &self,
        kb: &str,
        mut document: KnowledgeBaseDocument,
        key: &'static str,
    ) -> Result<(), KnowledgeBaseError> {
        if ingestion_worker::in_progress(&document.status) || document.status == "NEEDS_REBUILD" {
            return Ok(());
        }
        let revision = document.revision;
        document.revision += 1;
        document.status = "NEEDS_REBUILD".into();
        document.error = Some(crate::shared::i18n_message::MessageText::new(key).encode());
        // Preserve the published pointer and avoid overwriting concurrent document changes.
        match self.repository.save_document(kb, &document, revision) {
            Ok(()) | Err(KnowledgeBaseError::Conflict) => Ok(()),
            Err(error) => Err(error),
        }
    }
    pub async fn search_for_agent(
        &self,
        user: &str,
        workspace: &str,
        agent: &str,
        query: &str,
        token_budget: usize,
    ) -> Result<KnowledgeBaseContext, KnowledgeBaseError> {
        self.search_allowed(user, workspace, agent, query, token_budget, None)
            .await
    }
    pub async fn search_for_agent_in_run(
        &self,
        user: &str,
        workspace: &str,
        agent: &str,
        query: &str,
        token_budget: usize,
        run: &eliteforge_agent_framework::RunContext,
    ) -> Result<KnowledgeBaseContext, KnowledgeBaseError> {
        run.check_cancelled().map_err(invalid_storage)?;
        let result = self
            .search_allowed(user, workspace, agent, query, token_budget, Some(run))
            .await;
        run.check_cancelled().map_err(invalid_storage)?;
        result
    }
    fn allowed(
        &self,
        user: &str,
        workspace: &str,
        agent: &str,
    ) -> Result<Vec<KnowledgeBase>, KnowledgeBaseError> {
        self.require_user(user)?;
        if self
            .agents
            .workspace_internal_agent_policy(workspace, agent)
            .is_err()
        {
            return Ok(vec![]);
        }
        self.repository
            .authorized_libraries_with_documents(user, agent)
    }
    pub(crate) fn authorized_rag_ranges(
        &self,
        user: &str,
        workspace: &str,
        agent: &str,
    ) -> Result<(Vec<AuthorizedRagRange>, Vec<KnowledgeBase>, Vec<String>), KnowledgeBaseError>
    {
        self.rag_ranges_for_libraries(self.allowed(user, workspace, agent)?)
    }
    fn rag_ranges_for_libraries(
        &self,
        allowed: Vec<KnowledgeBase>,
    ) -> Result<(Vec<AuthorizedRagRange>, Vec<KnowledgeBase>, Vec<String>), KnowledgeBaseError>
    {
        let ids = allowed.iter().map(|kb| kb.id.clone()).collect::<Vec<_>>();
        let acl_revisions = self.repository.acl_revisions(&ids)?;
        let corpora = self.rag.repository_handle().get_corpora(&ids)?;
        let mut fingerprints = BTreeMap::new();
        let mut versions = vec![];
        let mut ranges = vec![];
        for kb in &allowed {
            let sources: Vec<String> = kb
                .documents
                .iter()
                .filter(|d| !fidelity::blocked(d))
                .filter_map(|d| d.active_source_code.clone())
                .collect();
            let mut identities = kb
                .documents
                .iter()
                .map(|d| {
                    format!(
                        "{}:{}:{}:{}:{}:{}",
                        d.id,
                        d.resource_revision,
                        d.active_source_code.as_deref().unwrap_or(""),
                        d.index_revision,
                        d.revision,
                        d.status
                    )
                })
                .collect::<Vec<_>>();
            identities.sort();
            versions.push(format!(
                "{}:{}:{}",
                kb.id,
                acl_revisions
                    .get(&kb.id)
                    .ok_or(KnowledgeBaseError::NotFound)?,
                identities.join("|")
            ));
            let settings = RagSettings::from_capabilities(
                &corpora
                    .get(&kb.id)
                    .ok_or(KnowledgeBaseError::NotFound)?
                    .capabilities,
            )?;
            ranges.push(AuthorizedRagRange {
                corpus_id: kb.id.clone(),
                scopes: vec![RagSearchScope {
                    scope_type: "SPACE".into(),
                    scope_code: "PLATFORM".into(),
                }],
                source_version: "v1".into(),
                filters: json!({"sourceCode":{"op":"in","value":sources}}),
                embedding_model_code: None,
                embedding_dimension: None,
                provider_model: None,
                embedding_fingerprint: None,
                return_fields: vec![],
                fulltext: settings.fulltext,
                vector: false,
                graph: settings.graph,
            });
            let mut groups =
                BTreeMap::<(String, usize, String, Option<String>), Vec<String>>::new();
            for document in kb.documents.iter().filter(|document| {
                document.active_source_code.is_some() && !fidelity::blocked(document)
            }) {
                let (Some(model), Some(dimension), Some(stored)) = (
                    &document.embedding_model_code,
                    document.embedding_dimension,
                    &document.embedding_space_fingerprint,
                ) else {
                    continue;
                };
                let fingerprint = fingerprints
                    .entry((model.clone(), dimension))
                    .or_insert_with(|| {
                        self.llm
                            .embedding_space_fingerprint("PLATFORM", Some(model), dimension)
                    });
                if !fingerprint.as_ref().is_ok_and(|current| current == stored) {
                    self.require_rebuild(
                        &kb.id,
                        document.clone(),
                        "knowledge.base.embedding_config_changed",
                    )?;
                }
                // Preserve the published partition so RAG reports a failed vector
                // stage when its model changes instead of silently omitting it.
                if settings.vector {
                    groups
                        .entry((
                            model.clone(),
                            dimension,
                            stored.clone(),
                            document.embedding_provider_model.clone(),
                        ))
                        .or_default()
                        .push(document.active_source_code.clone().unwrap());
                }
            }
            for ((model, dimension, fingerprint, provider_model), source_codes) in groups {
                ranges.push(AuthorizedRagRange {
                    corpus_id: kb.id.clone(),
                    scopes: vec![RagSearchScope {
                        scope_type: "SPACE".into(),
                        scope_code: "PLATFORM".into(),
                    }],
                    source_version: "v1".into(),
                    filters: json!({"sourceCode":{"op":"in","value":source_codes}}),
                    embedding_model_code: Some(model),
                    embedding_dimension: Some(dimension),
                    provider_model,
                    embedding_fingerprint: Some(fingerprint),
                    return_fields: vec![],
                    fulltext: false,
                    vector: true,
                    graph: false,
                });
            }
        }
        Ok((ranges, allowed, versions))
    }
    pub(crate) fn recheck_rag_evidence(
        &self,
        user: &str,
        workspace: &str,
        agent: &str,
        result: &mut RagEvidenceResult,
    ) -> Result<(), KnowledgeBaseError> {
        let allowed = self.allowed(user, workspace, agent)?;
        result.hits.retain(|hit| {
            matches!(hit.row.corpus_id.as_str(), "memory-card" | "skill")
                || allowed.iter().any(|kb| {
                    kb.id == hit.row.corpus_id
                        && kb.documents.iter().any(|doc| {
                            !fidelity::blocked(doc)
                                && doc.active_source_code.as_deref() == Some(&hit.row.source_code)
                        })
                })
        });
        Ok(())
    }
    async fn search_allowed(
        &self,
        user: &str,
        workspace: &str,
        agent: &str,
        query: &str,
        token_budget: usize,
        run: Option<&eliteforge_agent_framework::RunContext>,
    ) -> Result<KnowledgeBaseContext, KnowledgeBaseError> {
        if token_budget == 0 || query.trim().is_empty() {
            return Ok(KnowledgeBaseContext {
                text: String::new(),
                source_version: String::new(),
                hits: vec![],
                warnings: vec![],
            });
        }
        let mut hits = vec![];
        let (_, allowed, mut versions) = self.authorized_rag_ranges(user, workspace, agent)?;
        if allowed.is_empty() {
            return Ok(KnowledgeBaseContext {
                text: String::new(),
                source_version: String::new(),
                hits: vec![],
                warnings: vec![],
            });
        }
        let result = self
            .rag
            .search(
                self,
                &RagIdentity {
                    user_code: user.into(),
                    workspace_code: workspace.into(),
                    agent_code: agent.into(),
                },
                RagQueryInput {
                    query: query.into(),
                    corpus_ids: allowed.iter().map(|kb| kb.id.clone()).collect(),
                    filters: json!({}),
                    limit: 8,
                },
                run,
            )
            .await?;
        let retrieval_warnings = result.warnings;
        for evidence in result.evidence {
            let row = &evidence;
            let Some(kb) = allowed.iter().find(|kb| kb.id == row.corpus_id) else {
                continue;
            };
            let Some(doc) = kb.documents.iter().find(|d| {
                !fidelity::blocked(d) && d.active_source_code.as_ref() == Some(&row.source_code)
            }) else {
                continue;
            };
            if hits
                .iter()
                .any(|h: &KnowledgeBaseHit| h.chunk_code == row.chunk_code)
            {
                continue;
            }
            hits.push(KnowledgeBaseHit {
                knowledge_base_id: kb.id.clone(),
                document_id: doc.id.clone(),
                resource_code: doc.resource_code.clone(),
                resource_revision: doc.resource_revision,
                source_code: row.source_code.clone(),
                chunk_code: row.chunk_code.clone(),
                title: doc.name.clone(),
                text: row.text.clone(),
                location: {
                    let mut location = row.location.clone();
                    let name = doc.file_name.to_ascii_lowercase();
                    if (name.ends_with(".docx") || name.ends_with(".xlsx"))
                        && let Some(object) = location.as_object_mut()
                    {
                        object.remove("page");
                    }
                    location
                },
                score: evidence.score,
            });
        }
        let currently_allowed = self.allowed(user, workspace, agent)?;
        hits.retain(|hit| {
            currently_allowed.iter().any(|kb| {
                kb.id == hit.knowledge_base_id
                    && kb.documents.iter().any(|d| {
                        !fidelity::blocked(d)
                            && d.active_source_code.as_ref() == Some(&hit.source_code)
                    })
            })
        });
        let mut remaining = token_budget.saturating_mul(4);
        let mut selected = vec![];
        let mut text = String::new();
        for mut hit in hits.into_iter().take(8) {
            if remaining < 32 {
                break;
            }
            let location = compact_location(&hit.location);
            let title: String = hit.title.chars().take(32).collect();
            let reference = format!(
                "[文档:{title} ID:{}{}]",
                hit.document_id,
                if location.is_empty() {
                    String::new()
                } else {
                    format!(" 位置:{location}")
                }
            );
            // Keep evidence first: the SDK may apply a stricter token budget than
            // our conservative character estimate. Prefix clipping must retain
            // document content, while full citation metadata remains in hits.
            let overhead = reference.chars().count() + 3;
            let body_budget = if remaining > overhead + 32 {
                remaining - overhead
            } else {
                remaining.saturating_sub(2)
            };
            hit.text = hit.text.chars().take(body_budget).collect();
            if hit.text.is_empty() {
                continue;
            }
            let body_len = hit.text.chars().count();
            text.push_str(&hit.text);
            remaining = remaining.saturating_sub(body_len);
            if remaining >= overhead {
                text.push('\n');
                text.push_str(&reference);
                remaining = remaining.saturating_sub(overhead - 2);
            }
            text.push_str("\n\n");
            remaining = remaining.saturating_sub(2);
            selected.push(hit);
        }
        Ok(KnowledgeBaseContext {
            text,
            source_version: {
                versions.sort();
                hash(&format!(
                    "{user}:{workspace}:{agent}:{}",
                    versions.join("|")
                ))
            },
            hits: selected,
            warnings: retrieval_warnings,
        })
    }
}
fn compact_location(location: &Value) -> String {
    let mut parts = Vec::new();
    if let Some(page) = location.get("page").and_then(Value::as_u64) {
        parts.push(format!("页{page}"));
    }
    if let Some(page) = location.get("renderedPage").and_then(Value::as_u64) {
        parts.push(format!("转换预览{page}（非原文页）"));
    }
    if let Some(sheet) = location.get("sheet").and_then(Value::as_str) {
        parts.push(sheet.chars().take(24).collect::<String>());
    }
    for (start_key, end_key, label) in [
        ("rowStart", "rowEnd", "行"),
        ("paragraph", "paragraphEnd", "段"),
        ("lineStart", "lineEnd", "行"),
    ] {
        if let Some(start) = location.get(start_key).and_then(Value::as_u64) {
            let end = location.get(end_key).and_then(Value::as_u64);
            parts.push(match end {
                Some(end) if end != start => format!("{label}{start}-{end}"),
                _ => format!("{label}{start}"),
            });
        }
    }
    if let Some(cells) = location.get("cellRanges").and_then(Value::as_array) {
        let values: Vec<_> = cells.iter().filter_map(Value::as_str).collect();
        if values.len() <= 4 {
            parts.extend(values.into_iter().map(str::to_owned));
        } else {
            // Do not imply the first few cells cover all evidence in a merged chunk.
            parts.push(format!("{}个单元格；完整地址见来源", values.len()));
        }
    }
    if parts.is_empty() {
        if let Some(blocks) = location.get("blocks").and_then(Value::as_array) {
            parts.extend(
                blocks
                    .iter()
                    .filter_map(Value::as_str)
                    .take(2)
                    .map(|b| format!("块{b}")),
            );
        }
    }
    if let Some(heading) = location.get("heading").and_then(Value::as_str)
        && !heading.is_empty()
    {
        parts.push(heading.chars().take(24).collect());
    }
    parts.join(" ").chars().take(64).collect()
}
pub(crate) fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub(crate) fn invalid_storage(error: impl std::fmt::Display) -> KnowledgeBaseError {
    KnowledgeBaseError::storage_detail(error.to_string())
}

fn new_id() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl RagAccessResolver for KnowledgeBaseService {
    fn resolve(&self, identity: &RagIdentity) -> Result<Vec<AuthorizedRagRange>, RagError> {
        self.require_user(&identity.user_code)
            .map_err(|_| RagError::SearchScopeInvalid)?;
        self.agents
            .workspace_internal_agent_policy(&identity.workspace_code, &identity.agent_code)
            .map_err(|_| RagError::SearchScopeInvalid)?;
        let (mut ranges, _, _) = self
            .authorized_rag_ranges(
                &identity.user_code,
                &identity.workspace_code,
                &identity.agent_code,
            )
            .map_err(|_| RagError::SearchScopeInvalid)?;
        let corpus = "skill";
        let settings = self.rag.settings_for_corpus(corpus)?;
        let sources = self.allowed_skill_sources(&identity.agent_code)?;
        if !sources.is_empty() {
            ranges.extend(self.rag.partition_ranges(AuthorizedRagRange {
                corpus_id: corpus.into(),
                scopes: vec![RagSearchScope {
                    scope_type: "SPACE".into(),
                    scope_code: "PLATFORM".into(),
                }],
                source_version: "v1".into(),
                filters: json!({"sourceCode":{"op":"in","value":sources}}),
                embedding_model_code: settings.embedding_model_code,
                embedding_dimension: Some(TRON_VECTOR_INDEX_DIMENSION),
                provider_model: None,
                embedding_fingerprint: None,
                return_fields: vec![],
                fulltext: settings.fulltext,
                vector: settings.vector,
                graph: settings.graph,
            })?);
        }
        Ok(ranges)
    }
    fn recheck_sources(
        &self,
        identity: &RagIdentity,
        result: &mut RagEvidenceResult,
    ) -> Result<(), RagError> {
        self.agents
            .workspace_internal_agent_policy(&identity.workspace_code, &identity.agent_code)
            .map_err(|_| RagError::SearchScopeInvalid)?;
        self.recheck_rag_evidence(
            &identity.user_code,
            &identity.workspace_code,
            &identity.agent_code,
            result,
        )
        .map_err(|_| RagError::SearchScopeInvalid)?;
        let allowed_skills = self.allowed_skill_sources(&identity.agent_code)?;
        result.hits.retain(|hit| {
            hit.row.corpus_id != "skill" || allowed_skills.contains(&hit.row.source_code)
        });
        let now = chrono::Utc::now().timestamp_millis();
        result.hits.retain(|hit| {
            hit.row.corpus_id != "memory-card"
                || hit
                    .row
                    .field_values
                    .get("expiresAt")
                    .and_then(Value::as_i64)
                    .is_none_or(|expiry| expiry > now)
        });

        Ok(())
    }
    fn recheck(
        &self,
        identity: &RagIdentity,
        result: &mut RagEvidenceResult,
    ) -> Result<(), RagError> {
        self.recheck_sources(identity, result)
    }
    fn assemble_context(
        &self,
        identity: &RagIdentity,
        result: &mut RagEvidenceResult,
    ) -> Result<(), RagError> {
        self.expand_context_evidence(&identity.user_code, result)
            .map_err(|_| RagError::StorageFailure)
    }
}
