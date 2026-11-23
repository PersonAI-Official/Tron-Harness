//! Each HTTP use case owns its root DTO; nested evidence is a public concept.
use crate::application::rag::{
    RagEvidenceView, RagQueryResult, RagRetrievalStage, graph::GraphEvidenceView,
};
use crate::shared::i18n_message::render_stored;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RecallInput {
    pub query: String,
    #[serde(default)]
    pub corpus_ids: Vec<String>,
    #[serde(default = "filters")]
    pub filters: Value,
    #[serde(default = "limit")]
    pub limit: usize,
}
fn filters() -> Value {
    json!({})
}
fn limit() -> usize {
    8
}

/// Warnings and stage failures are stored as locale-independent structured
/// messages. Render them at the HTTP edge, where the request locale is known,
/// and never forward raw provider diagnostics that were not migrated.
fn localized_warnings(warnings: Vec<String>, locale: &str) -> Vec<String> {
    warnings
        .into_iter()
        .map(|warning| render_stored(&warning, locale, Some("rag.search.warning_unavailable")))
        .collect()
}

fn localized_stages(stages: Vec<RagRetrievalStage>, locale: &str) -> Vec<RagRetrievalStage> {
    stages
        .into_iter()
        .map(|mut stage| {
            stage.error = stage.error.as_deref().map(|error| {
                render_stored(error, locale, Some("rag.search.stage_failure_unavailable"))
            });
            stage
        })
        .collect()
}

macro_rules! response {
    ($name:ident) => {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        pub(super) struct $name {
            query: String,
            authorized_corpus_ids: Vec<String>,
            stages: Vec<RagRetrievalStage>,
            warnings: Vec<String>,
            evidence: Vec<RagEvidenceView>,
            graph_context: Vec<GraphEvidenceView>,
            duration_ms: u64,
        }
        impl $name {
            pub(super) fn from_result(result: RagQueryResult, locale: &str) -> Self {
                Self {
                    query: result.query,
                    authorized_corpus_ids: result.authorized_corpus_ids,
                    stages: localized_stages(result.stages, locale),
                    warnings: localized_warnings(result.warnings, locale),
                    evidence: result.evidence,
                    graph_context: result.graph_context,
                    duration_ms: result.duration_ms,
                }
            }
        }
    };
}
response!(SearchResponse);
response!(RecallResponse);
