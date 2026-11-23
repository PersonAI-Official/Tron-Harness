//! Fixed retrieval flow shared by product context and the RAG search tool.
//! Callers supply server-authorized source snapshots, never client scopes.
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};

use super::{
    RagEmbeddingCall, RagRerankCall, RagRouteSearchQuery, RagSearchRepository, RagSearchRow,
    RagSearchRuntime, RagSearchScope, RagService,
};
use crate::domain::rag::RagError;

#[derive(Clone, Debug)]
pub struct AuthorizedRagRange {
    pub corpus_id: String,
    pub scopes: Vec<RagSearchScope>,
    pub source_version: String,
    pub filters: Value,
    pub embedding_model_code: Option<String>,
    pub embedding_dimension: Option<usize>,
    pub provider_model: Option<String>,
    pub embedding_fingerprint: Option<String>,
    pub return_fields: Vec<String>,
    pub fulltext: bool,
    pub vector: bool,
    pub graph: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagRetrievalStage {
    pub corpus_id: String,
    pub channel: String,
    pub status: String,
    pub hit_count: usize,
    pub duration_ms: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RagEvidence {
    pub row: RagSearchRow,
    pub channels: BTreeSet<String>,
    pub score: f64,
    /// Native cosine score survives fusion even when the row came from fulltext.
    pub vector_similarity: Option<f64>,
    /// Assembled source text; the matched publication row remains immutable.
    pub context_text: Option<String>,
    pub recall_ranks: BTreeMap<String, usize>,
    pub bundle: Option<RagEvidenceBundle>,
    pub selected_blocks: BTreeSet<String>,
}
#[derive(Clone, Debug)]
pub struct RagEvidenceBundle {
    /// Publication rows are independent of the assembled display text.
    pub members: Vec<RagEvidence>,
    pub display: Vec<RagEvidence>,
    pub partition: crate::domain::rag::graph::GraphPartition,
    pub generations: BTreeSet<String>,
    pub nodes: Vec<crate::domain::rag::graph::RagGraphNode>,
    pub edges: Vec<crate::domain::rag::graph::RagGraphEdge>,
    pub truncated_by: BTreeSet<String>,
}
impl RagEvidence {
    pub(crate) fn associated(row: RagSearchRow) -> Self {
        Self {
            row,
            channels: BTreeSet::new(),
            score: 0.,
            vector_similarity: None,
            context_text: None,
            recall_ranks: BTreeMap::new(),
            bundle: None,
            selected_blocks: BTreeSet::new(),
        }
    }
    pub fn text(&self) -> &str {
        self.context_text.as_deref().unwrap_or(&self.row.text)
    }
    pub(crate) fn select_blocks(&mut self, blocks: BTreeSet<String>) -> bool {
        let selected = super::source_parts::select(
            &self.row.text_metadata,
            &self.row.text,
            &self.row.target_code,
            &blocks,
        );
        if let Some(text) = selected {
            self.context_text = Some(text);
            self.selected_blocks = blocks;
            true
        } else {
            false
        }
    }
    pub fn rerank_text(&self) -> String {
        let Some(bundle) = &self.bundle else {
            return self.text().to_owned();
        };
        let rows = if bundle.display.is_empty() {
            std::iter::once(self)
                .chain(bundle.members.iter())
                .collect::<Vec<_>>()
        } else {
            bundle.display.iter().collect()
        };
        let mut seen = BTreeSet::new();
        rows.into_iter()
            .map(Self::text)
            .filter(|text| !text.trim().is_empty() && seen.insert(*text))
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Clone, Debug, Default)]
pub struct RagEvidenceResult {
    pub hits: Vec<RagEvidence>,
    pub stages: Vec<RagRetrievalStage>,
    pub warnings: Vec<String>,
    pub graph_context: Vec<super::graph::GraphRecallContext>,
}

impl<R, E> RagService<R, E>
where
    R: RagSearchRepository + Clone,
    E: RagSearchRuntime,
{
    pub async fn retrieve_authorized(
        &self,
        workspace: &str,
        query: &str,
        ranges: &[AuthorizedRagRange],
        limit: usize,
        rerank_model: Option<&str>,
        run: Option<&eliteforge_agent_framework::RunContext>,
    ) -> Result<RagEvidenceResult, RagError> {
        let result = self
            .recall_authorized(workspace, query, ranges, limit, run)
            .await?;
        self.rank_evidence(workspace, query, result, limit, rerank_model)
            .await
    }

    /// Recall retains the full candidate set so source evidence can be assembled
    /// and authorization checked before sending any text to a reranker.
    pub(crate) async fn recall_authorized(
        &self,
        workspace: &str,
        query: &str,
        ranges: &[AuthorizedRagRange],
        limit: usize,
        run: Option<&eliteforge_agent_framework::RunContext>,
    ) -> Result<RagEvidenceResult, RagError> {
        if !(1..=100).contains(&limit) {
            return Err(RagError::SearchRequestInvalid);
        }
        let relevance = self.retrieval_config();
        let mut result = RagEvidenceResult::default();
        let mut merged = BTreeMap::<String, RagEvidence>::new();
        let mut embeddings = BTreeMap::<
            (
                Option<String>,
                Option<usize>,
                Option<String>,
                Option<String>,
                String,
            ),
            Result<Vec<f32>, String>,
        >::new();
        for range in ranges {
            if range.scopes.is_empty()
                || range.filters["sourceCode"]["value"]
                    .as_array()
                    .is_some_and(|v| v.is_empty())
            {
                continue;
            }
            let model_workspace = if range.corpus_id == "memory-card" {
                super::MEMORY_MODEL_WORKSPACE
            } else {
                workspace
            };
            let geometry = (
                range.embedding_model_code.clone(),
                range.embedding_dimension,
                range.provider_model.clone(),
                range.embedding_fingerprint.clone(),
                model_workspace.to_owned(),
            );
            if range.vector && !query.trim().is_empty() && !embeddings.contains_key(&geometry) {
                let before = self.runtime.embedding_fingerprint(
                    model_workspace,
                    geometry.0.as_deref(),
                    geometry.1.unwrap_or(1024),
                );
                let fingerprint_matches = |value: &Result<Option<String>, String>| {
                    value.as_ref().is_ok_and(|actual| {
                        geometry
                            .3
                            .as_ref()
                            .is_none_or(|expected| actual.as_ref() == Some(expected))
                    })
                };
                let vector = if !fingerprint_matches(&before) {
                    Err(crate::shared::i18n_message::MessageText::new(
                        "rag.search.query_model_space_mismatch",
                    )
                    .encode()
                    .into())
                } else {
                    self.runtime
                        .embed_for_retrieval(
                            RagEmbeddingCall {
                                workspace_code: model_workspace.into(),
                                model_code: geometry.0.clone(),
                                input: query.into(),
                            },
                            geometry.1,
                            run,
                        )
                        .await
                        .and_then(|value| {
                            if geometry
                                .0
                                .as_ref()
                                .is_some_and(|expected| *expected != value.model_code)
                                || match (&geometry.2, &value.provider_model) {
                                    (Some(expected), actual) => actual.as_ref() != Some(expected),
                                    _ => false,
                                }
                            {
                                let mut invalidation = super::graph::graph_source_query(range);
                                invalidation.query_embedding_model_code = geometry.0.clone();
                                invalidation.policy =
                                    json!({"fingerprint":geometry.3,"providerModel":geometry.2});
                                self.repository
                                    .invalidate_vector_cache(invalidation)
                                    .map_err(|e| e.to_string())?;
                                return Err(crate::shared::i18n_message::MessageText::new(
                                    "rag.search.query_model_mismatch",
                                )
                                .encode()
                                .into());
                            }
                            let values = value.embedding;
                            if values.is_empty()
                                || values.iter().any(|value| !value.is_finite())
                                || geometry
                                    .1
                                    .is_some_and(|dimension| dimension != values.len())
                            {
                                Err(crate::shared::i18n_message::MessageText::new(
                                    "rag.search.query_dimension_mismatch",
                                )
                                .encode()
                                .into())
                            } else {
                                Ok(values)
                            }
                        })
                        .and_then(|values| {
                            let after = self.runtime.embedding_fingerprint(
                                model_workspace,
                                geometry.0.as_deref(),
                                geometry.1.unwrap_or(1024),
                            );
                            if after != before || !fingerprint_matches(&after) {
                                Err(crate::shared::i18n_message::MessageText::new(
                                    "rag.search.query_embedding_changed",
                                )
                                .encode()
                                .into())
                            } else {
                                Ok(values)
                            }
                        })
                };
                embeddings.insert(geometry.clone(), vector);
            }
            for (channel, retriever, enabled) in [
                (
                    "fulltext",
                    if query.trim().is_empty() {
                        "STRUCTURED_FILTER_SQLITE_V1"
                    } else {
                        "FULLTEXT_SQLITE_FTS5_SIMPLE_V1"
                    },
                    range.fulltext,
                ),
                ("vector", "VECTOR_SQLITE_VEC0_COSINE_V1", range.vector),
            ] {
                if !enabled || (channel == "vector" && query.trim().is_empty()) {
                    continue;
                }
                let started = std::time::Instant::now();
                let vector = if channel == "vector" {
                    match embeddings.get(&geometry) {
                        Some(Ok(values)) => Some(values.clone()),
                        Some(Err(error)) => {
                            result
                                .warnings
                                .push(format!("{}: {error}", range.corpus_id));
                            result.stages.push(stage(
                                range,
                                channel,
                                started,
                                0,
                                Some(error.clone()),
                            ));
                            continue;
                        }
                        None => return Err(RagError::SearchRequestInvalid),
                    }
                } else {
                    None
                };
                let rows = self.repository.search_route(RagRouteSearchQuery {
                    corpus_id: range.corpus_id.clone(), route_id: channel.into(),
                    retriever: retriever.into(), workspace_code: workspace.into(), query: query.into(),
                    query_embedding: vector, query_embedding_model_code: range.embedding_model_code.clone(),
                    source_version: range.source_version.clone(), scopes: range.scopes.clone(),
                    filters: range.filters.clone(), policy: json!({"providerModel":range.provider_model,"fingerprint":range.embedding_fingerprint}), return_fields: range.return_fields.clone(),
                    limit: limit.saturating_mul(3).min(100),
                });
                match rows {
                    Ok(mut rows) => {
                        result
                            .stages
                            .push(stage(range, channel, started, rows.len(), None));
                        if channel == "vector" {
                            rows.retain(|row| relevance.accepts_vector(row.storage_score));
                            result.stages.push(stage(
                                range,
                                "vector_relevance",
                                started,
                                rows.len(),
                                None,
                            ));
                        }
                        merge_rrf(&mut merged, rows, channel);
                    }
                    Err(error) => {
                        result
                            .warnings
                            .push(format!("{} {channel}: {error}", range.corpus_id));
                        result.stages.push(stage(
                            range,
                            channel,
                            started,
                            0,
                            Some(error.to_string()),
                        ));
                    }
                }
            }
            if range.graph && !query.trim().is_empty() {
                let started = std::time::Instant::now();
                match self
                    .recall_graph_range(model_workspace, query, range, run, &mut embeddings)
                    .await
                {
                    Ok(graph) => {
                        result.stages.extend(graph.stages);
                        result.warnings.extend(graph.warnings);
                        result.graph_context.extend(graph.contexts);
                        merge_rrf(&mut merged, graph.rows, "graph");
                    }
                    Err(error) => {
                        result
                            .warnings
                            .push(format!("{} graph: {error}", range.corpus_id));
                        result
                            .stages
                            .push(stage(range, "graph", started, 0, Some(error)));
                    }
                }
            }
        }
        let fusion_started = std::time::Instant::now();
        result.hits = merged.into_values().collect();
        result.hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.row.target_code.cmp(&b.row.target_code))
        });
        result.stages.push(RagRetrievalStage {
            corpus_id: String::new(),
            channel: "fusion".into(),
            status: "COMPLETED".into(),
            hit_count: result.hits.len(),
            duration_ms: fusion_started.elapsed().as_millis() as u64,
            error: None,
        });
        Ok(result)
    }

    pub(crate) async fn rank_evidence(
        &self,
        workspace: &str,
        query: &str,
        mut result: RagEvidenceResult,
        limit: usize,
        rerank_model: Option<&str>,
    ) -> Result<RagEvidenceResult, RagError> {
        if !(1..=100).contains(&limit) {
            return Err(RagError::SearchRequestInvalid);
        }
        if result.hits.is_empty() {
            return Ok(result);
        }
        // Filter-only memory listing has no relevance query. Keep the fused
        // order; never send an invalid empty query to a remote reranker.
        if query.trim().is_empty() {
            result.hits.truncate(limit);
            result.stages.push(RagRetrievalStage {
                corpus_id: String::new(),
                channel: "rerank".into(),
                status: "SKIPPED".into(),
                hit_count: result.hits.len(),
                duration_ms: 0,
                error: None,
            });
            return Ok(result);
        }
        let relevance = self.retrieval_config();
        let mut reranked = false;
        if let Some(model) = rerank_model.filter(|model| !model.trim().is_empty()) {
            let rerank_started = std::time::Instant::now();
            let order = self
                .runtime
                .rerank(RagRerankCall {
                    workspace_code: workspace.into(),
                    model_code: Some(model.into()),
                    query: query.into(),
                    documents: result.hits.iter().map(RagEvidence::rerank_text).collect(),
                    top_n: Some(limit),
                })
                .await;
            let mut rerank_error = None;
            match order {
                Ok(order)
                    if order
                        .iter()
                        .all(|(index, score)| *index < result.hits.len() && score.is_finite())
                        && order
                            .iter()
                            .map(|(index, _)| index)
                            .collect::<BTreeSet<_>>()
                            .len()
                            == order.len() =>
                {
                    reranked = true;
                    result.hits = order
                        .into_iter()
                        .filter(|(_, score)| relevance.accepts_rerank(*score))
                        .map(|(index, score)| {
                            let mut hit = result.hits[index].clone();
                            hit.score = score;
                            hit
                        })
                        .collect();
                }
                Ok(_) => {
                    let error = crate::shared::i18n_message::MessageText::new(
                        "rag.search.rerank_invalid_fallback",
                    )
                    .encode();
                    result.warnings.push(error.clone());
                    rerank_error = Some(error);
                }
                Err(error) => {
                    // Provider diagnostics stay in server logs. The response
                    // carries a stable message so the UI never renders raw SDK
                    // payloads, credentials metadata or upstream HTML.
                    tracing::warn!(error = %error, "rerank stage failed; keeping fused fallback order");
                    let message = crate::shared::i18n_message::MessageText::new(
                        "rag.search.rerank_unavailable_fallback",
                    )
                    .encode();
                    result.warnings.push(message.clone());
                    rerank_error = Some(message);
                }
            }
            result.stages.push(RagRetrievalStage {
                corpus_id: String::new(),
                channel: "rerank".into(),
                status: if rerank_error.is_some() {
                    "FAILED"
                } else {
                    "COMPLETED"
                }
                .into(),
                hit_count: result.hits.len(),
                duration_ms: rerank_started.elapsed().as_millis() as u64,
                error: rerank_error,
            });
        }
        if !reranked {
            relevance.filter_fallback(&mut result.hits);
        } else {
            result.hits.sort_by(|a, b| {
                b.score
                    .total_cmp(&a.score)
                    .then_with(|| a.row.target_code.cmp(&b.row.target_code))
            });
        }
        result.stages.push(RagRetrievalStage {
            corpus_id: String::new(),
            channel: "relevance_filter".into(),
            status: "COMPLETED".into(),
            hit_count: result.hits.len(),
            duration_ms: 0,
            error: None,
        });
        result.hits.truncate(limit);
        Ok(result)
    }
}

fn stage(
    range: &AuthorizedRagRange,
    channel: &str,
    started: std::time::Instant,
    count: usize,
    error: Option<String>,
) -> RagRetrievalStage {
    RagRetrievalStage {
        corpus_id: range.corpus_id.clone(),
        channel: channel.into(),
        status: if error.is_some() {
            "FAILED"
        } else {
            "COMPLETED"
        }
        .into(),
        hit_count: count,
        duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        error,
    }
}

fn merge_rrf(merged: &mut BTreeMap<String, RagEvidence>, rows: Vec<RagSearchRow>, channel: &str) {
    let mut seen = BTreeSet::new();
    for (rank, row) in rows.into_iter().enumerate() {
        let key = format!(
            "{}:{}:{}:{}:{}:{}",
            row.corpus_id,
            row.source_code,
            row.source_version,
            row.scope_type,
            row.scope_code,
            row.target_code
        );
        if !seen.insert(key.clone()) {
            continue;
        }
        let vector_similarity = (channel == "vector")
            .then_some(row.storage_score)
            .flatten()
            .filter(|score| score.is_finite());
        let evidence = merged.entry(key).or_insert_with(|| RagEvidence {
            row,
            channels: BTreeSet::new(),
            score: 0.0,
            vector_similarity: None,
            context_text: None,
            recall_ranks: BTreeMap::new(),
            bundle: None,
            selected_blocks: BTreeSet::new(),
        });
        if let Some(score) = vector_similarity {
            evidence.vector_similarity = Some(
                evidence
                    .vector_similarity
                    .map_or(score, |current| current.max(score)),
            );
        }
        evidence.score += 1.0 / (60.0 + rank as f64 + 1.0);
        evidence.channels.insert(channel.into());
        evidence.recall_ranks.insert(channel.into(), rank);
    }
}

impl<R: RagSearchRepository + Clone, E: Clone> RagService<R, E> {
    pub(crate) fn recheck_graph_context(
        &self,
        ranges: &[AuthorizedRagRange],
        result: &mut RagEvidenceResult,
    ) -> Result<(), RagError> {
        if result.graph_context.is_empty() {
            return Ok(());
        }
        let mut visible =
            BTreeMap::<crate::domain::rag::graph::GraphPartition, BTreeSet<String>>::new();
        let mut generation_ids = BTreeSet::new();
        let corpora = self.repository.get_corpora(
            &ranges
                .iter()
                .map(|range| range.corpus_id.clone())
                .collect::<Vec<_>>(),
        )?;
        for range in ranges.iter().filter(|r| r.graph) {
            let corpus = corpora
                .get(&range.corpus_id)
                .ok_or(RagError::CorpusNotFound)?;
            if !super::RagSettings::from_capabilities(&corpus.capabilities)?.graph {
                continue;
            }
            generation_ids.extend(
                self.repository
                    .published_graph_generations(super::graph::graph_source_query(range))?,
            );
            for scope in &range.scopes {
                visible
                    .entry(crate::domain::rag::graph::GraphPartition {
                        corpus_id: range.corpus_id.clone(),
                        scope_type: scope.scope_type.clone(),
                        scope_code: scope.scope_code.clone(),
                    })
                    .or_default();
            }
        }
        if let Some(infra) = &self.graph {
            for generation in infra
                .index
                .active_generations(&visible.keys().cloned().collect::<Vec<_>>())
                .map_err(|_| RagError::StorageFailure)?
            {
                if generation_ids.contains(&generation.id) && generation.expected_projections > 0 {
                    visible
                        .entry(generation.partition)
                        .or_default()
                        .insert(generation.id);
                }
            }
        }
        let old = result.graph_context.len();
        result
            .graph_context
            .retain(|context| visible.get(&context.partition) == Some(&context.generations));
        if result.graph_context.len() != old {
            result.warnings.push(
                crate::shared::i18n_message::MessageText::new("rag.search.graph_scope_changed")
                    .encode()
                    .into(),
            );
        }
        Ok(())
    }
    /// Build vector partitions from published metadata inside the authorized scope.
    pub fn partition_ranges(
        &self,
        range: AuthorizedRagRange,
    ) -> Result<Vec<AuthorizedRagRange>, RagError> {
        let corpus = self
            .repository
            .get_corpus(&range.corpus_id)?
            .ok_or(RagError::CorpusNotFound)?;
        self.partition_ranges_for_corpus(range, &corpus)
    }
    pub(crate) fn partition_ranges_for_corpus(
        &self,
        mut range: AuthorizedRagRange,
        corpus: &crate::domain::rag::RagCorpusRecord,
    ) -> Result<Vec<AuthorizedRagRange>, RagError> {
        if range.corpus_id != corpus.corpus_id {
            return Err(RagError::SearchScopeInvalid);
        }
        range.filters = super::merge_search_request_filters(&corpus, &range.filters, &json!({}))?;
        let vector = range.vector;
        range.vector = false;
        let mut ranges = vec![range.clone()];
        if !vector {
            return Ok(ranges);
        }
        let rows = self.repository.embedding_partitions(RagRouteSearchQuery {
            corpus_id: range.corpus_id.clone(),
            route_id: "partition".into(),
            retriever: "STRUCTURED_FILTER_SQLITE_V1".into(),
            workspace_code: String::new(),
            query: String::new(),
            query_embedding: None,
            query_embedding_model_code: None,
            source_version: range.source_version.clone(),
            scopes: range.scopes.clone(),
            filters: range.filters.clone(),
            policy: json!({}),
            return_fields: vec![],
            limit: 100,
        })?;
        let mut groups =
            BTreeMap::<(String, usize, Option<String>, Option<String>), Vec<String>>::new();
        for row in rows {
            groups
                .entry((
                    row.model_code,
                    row.dimension,
                    row.provider_model,
                    row.fingerprint,
                ))
                .or_default()
                .push(row.source_code);
        }
        for ((model, dimension, provider, fingerprint), sources) in groups {
            let mut partition = range.clone();
            partition.fulltext = false;
            partition.graph = false;
            partition.vector = true;
            partition.embedding_model_code = Some(model);
            partition.embedding_dimension = Some(dimension);
            partition.provider_model = provider;
            partition.embedding_fingerprint = fingerprint;

            partition.filters = super::merge_search_request_filters(
                &corpus,
                &range.filters,
                &json!({"sourceCode":{"op":"in","value":sources}}),
            )?;
            ranges.push(partition);
        }
        Ok(ranges)
    }
    pub fn recheck_published(
        &self,
        ranges: &[AuthorizedRagRange],
        result: &mut RagEvidenceResult,
    ) -> Result<(), RagError> {
        let mut visible = BTreeSet::new();
        let corpora = self.repository.get_corpora(
            &ranges
                .iter()
                .map(|range| range.corpus_id.clone())
                .collect::<Vec<_>>(),
        )?;
        for range in ranges {
            let sources = result
                .hits
                .iter()
                .filter(|hit| hit.row.corpus_id == range.corpus_id)
                .map(|hit| hit.row.source_code.clone())
                .collect::<BTreeSet<_>>();
            if sources.is_empty() {
                continue;
            }
            let corpus = corpora
                .get(&range.corpus_id)
                .ok_or(RagError::CorpusNotFound)?;
            let filters = super::merge_search_request_filters(
                &corpus,
                &range.filters,
                &json!({"sourceCode":{"op":"in","value":sources}}),
            )?;
            let rows = self.repository.search_route(RagRouteSearchQuery {
                corpus_id: range.corpus_id.clone(),
                route_id: "publication-check".into(),
                retriever: "STRUCTURED_FILTER_SQLITE_V1".into(),
                workspace_code: String::new(),
                query: String::new(),
                query_embedding: None,
                query_embedding_model_code: None,
                source_version: range.source_version.clone(),
                scopes: range.scopes.clone(),
                filters,
                policy: json!({"targetCodes":result.hits.iter().filter(|hit| hit.row.corpus_id==range.corpus_id).map(|hit|hit.row.target_code.clone()).collect::<BTreeSet<_>>()}),
                return_fields: vec![],
                limit: result.hits.len(),
            })?;
            for row in rows {
                visible.insert((
                    row.corpus_id,
                    row.target_code,
                    row.source_code,
                    row.source_version,
                    row.scope_type,
                    row.scope_code,
                    row.text,
                    serde_json::to_string(&row.source_metadata)
                        .map_err(|_| RagError::StorageFailure)?,
                ));
            }
        }
        let count = result.hits.len();
        result.hits.retain(|hit| {
            visible.contains(&(
                hit.row.corpus_id.clone(),
                hit.row.target_code.clone(),
                hit.row.source_code.clone(),
                hit.row.source_version.clone(),
                hit.row.scope_type.clone(),
                hit.row.scope_code.clone(),
                hit.row.text.clone(),
                serde_json::to_string(&hit.row.source_metadata).unwrap_or_default(),
            ))
        });
        if result.hits.len() != count {
            result.warnings.push(
                crate::shared::i18n_message::MessageText::new("rag.search.source_changed")
                    .encode()
                    .into(),
            );
        }
        Ok(())
    }
}
