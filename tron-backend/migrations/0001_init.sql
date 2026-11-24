-- 全新初始化：只定义当前表结构和必要的内置目录数据，不迁移历史数据库。
-- 运行时按业务库和身份登记库分别执行对应区段。
-- Framework 自有表由 Framework 存储插件创建；动态全文和向量索引由 RAG 运行时创建。

-- tron:business


CREATE TABLE agent_workspace_grant_policies(agent_code TEXT PRIMARY KEY);

CREATE TABLE agent_workspace_grants(agent_code TEXT NOT NULL, workspace_code TEXT NOT NULL, PRIMARY KEY(agent_code,workspace_code));

CREATE TABLE app_tags (
    id INTEGER PRIMARY KEY,
    app_code TEXT NOT NULL REFERENCES apps(app_code),
    tag_name TEXT NOT NULL,
    linked_at INTEGER NOT NULL,
    UNIQUE (app_code, tag_name)
);

CREATE TABLE apps (
    id INTEGER PRIMARY KEY,
    app_code TEXT NOT NULL UNIQUE,
    workspace_code TEXT NOT NULL,
    app_name TEXT NOT NULL,
    description TEXT NULL,
    local_directory TEXT NOT NULL,
    runtime_role TEXT NOT NULL CHECK (runtime_role IN ('frontend', 'backend', 'mono')),
    repository_url TEXT NULL,
    default_branch TEXT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL
);

CREATE TABLE automation_execution_records (
    id INTEGER PRIMARY KEY,
    execution_code TEXT NOT NULL UNIQUE,
    automation_code TEXT NOT NULL REFERENCES automations(automation_code),
    workspace_code TEXT NOT NULL,
    session_code TEXT NULL,
    trigger_source TEXT NOT NULL CHECK (trigger_source IN ('Scheduler', 'Manual')),
    status TEXT NOT NULL CHECK (status IN ('Pending', 'Running', 'Success', 'Failed', 'Canceled', 'Skipped')),
    summary TEXT NULL,
    scheduled_at INTEGER NULL,
    started_at INTEGER NOT NULL,
    finished_at INTEGER NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE automations (
    id INTEGER PRIMARY KEY,
    automation_code TEXT NOT NULL UNIQUE,
    workspace_code TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NULL,
    agent_code TEXT NOT NULL DEFAULT 'main-agent',
    model_code TEXT NULL,
    prompt_text TEXT NOT NULL,
    cron_expression TEXT NOT NULL,
    scheduler_expression TEXT NOT NULL,
    timezone TEXT NOT NULL DEFAULT 'UTC',
    status TEXT NOT NULL CHECK (status IN ('Active', 'Paused', 'Archived')),
    previous_status TEXT NULL CHECK (previous_status IS NULL OR previous_status IN ('Active', 'Paused')),
    last_execution_status TEXT NULL CHECK (last_execution_status IS NULL OR last_execution_status IN ('Pending', 'Running', 'Success', 'Failed', 'Canceled', 'Skipped')),
    last_execution_code TEXT NULL,
    last_execution_summary TEXT NULL,
    last_started_at INTEGER NULL,
    last_finished_at INTEGER NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL
);

CREATE TABLE chat_session_auto_model_routes (
    workspace_code TEXT NOT NULL,
    session_code TEXT NOT NULL,
    agent_code TEXT NOT NULL,
    selection_json TEXT NOT NULL,
    model_code TEXT NOT NULL,
    context_window_tokens INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (workspace_code, session_code, agent_code)
);

CREATE TABLE default_model_selections (
    scope_key TEXT NOT NULL,
    workspace_code TEXT,
    model_type TEXT NOT NULL,
    selection_kind TEXT NOT NULL,
    provider_code TEXT,
    model_code TEXT,
    quality_type TEXT,
    route_code TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(scope_key, model_type)
);

CREATE TABLE internal_agent_groups (
    scope_code TEXT NOT NULL,
    name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 60),
    PRIMARY KEY (scope_code, name)
);

INSERT INTO internal_agent_groups(scope_code, name) VALUES
    ('', '探索'),
    ('', '开发'),
    ('', '设计');

CREATE TABLE knowledge_base_derived_resources (
 document_id TEXT NOT NULL, workspace_code TEXT NOT NULL, resource_code TEXT NOT NULL,
 PRIMARY KEY(document_id,workspace_code,resource_code));

CREATE TABLE knowledge_base_documents (
 id TEXT PRIMARY KEY, knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id),
 payload_json TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 0);

CREATE TABLE knowledge_base_grants (
 knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id), agent_code TEXT NOT NULL,
 PRIMARY KEY(knowledge_base_id, agent_code));

CREATE TABLE knowledge_base_ingestion_intents (
 document_id TEXT PRIMARY KEY,
 knowledge_base_id TEXT NOT NULL,
 document_revision INTEGER NOT NULL,
 retry_after_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE knowledge_base_ingestion_tasks (
 id TEXT PRIMARY KEY,
 document_id TEXT NOT NULL,
 knowledge_base_id TEXT NOT NULL,
 input_config_hash TEXT NOT NULL,
 state TEXT NOT NULL,
 stage TEXT NOT NULL,
 attempt INTEGER NOT NULL,
 lease_owner TEXT,
 lease_until_ms INTEGER NOT NULL,
 retry_after_ms INTEGER NOT NULL,
 endpoint_id TEXT,
 resource_group TEXT,
 payload_json TEXT NOT NULL
);

CREATE TABLE knowledge_base_resource_privacy (
 workspace_code TEXT NOT NULL, resource_code TEXT NOT NULL, owner_user_code TEXT NOT NULL,
 PRIMARY KEY(workspace_code,resource_code));

CREATE TABLE knowledge_base_upload_requests (
 request_id TEXT PRIMARY KEY,
 content_identity TEXT NOT NULL,
 knowledge_base_id TEXT NOT NULL,
 document_id TEXT NOT NULL
);

CREATE TABLE knowledge_bases (
 id TEXT PRIMARY KEY, owner_user_code TEXT NOT NULL,
 payload_json TEXT NOT NULL, acl_revision INTEGER NOT NULL DEFAULT 0);

CREATE TABLE memory_records (
    memory_code TEXT PRIMARY KEY,
    user_code TEXT NOT NULL,
    scope_type TEXT NOT NULL CHECK(scope_type IN ('USER','SPACE','AGENT')),
    scope_code TEXT NOT NULL,
    semantic_key TEXT NOT NULL,
    title TEXT NOT NULL,
    text TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    version INTEGER NOT NULL,
    source_refs_json TEXT NOT NULL,
    source_label TEXT NOT NULL,
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(user_code, scope_type, scope_code, semantic_key)
);
CREATE INDEX idx_memory_records_query ON memory_records(user_code,scope_type,scope_code,updated_at DESC,memory_code);

CREATE TABLE memory_work_domains (
    domain_code TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'DISABLED')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE knowledge_evaluation_datasets (
 knowledge_base_id TEXT PRIMARY KEY REFERENCES knowledge_bases(id) ON DELETE CASCADE,
 owner_user_code TEXT NOT NULL, revision INTEGER NOT NULL, payload_json TEXT NOT NULL
);
CREATE TABLE knowledge_quality_runs (
 id TEXT PRIMARY KEY, knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
 owner_user_code TEXT NOT NULL, request_id TEXT NOT NULL, state TEXT NOT NULL,
 created_at INTEGER NOT NULL, payload_json TEXT NOT NULL,
 UNIQUE(owner_user_code,knowledge_base_id,request_id)
);
CREATE UNIQUE INDEX idx_knowledge_quality_active ON knowledge_quality_runs(knowledge_base_id)
 WHERE state IN ('QUEUED','RUNNING','PUBLISHING','CANCEL_REQUESTED');
CREATE INDEX idx_knowledge_quality_owner ON knowledge_quality_runs(owner_user_code,knowledge_base_id,created_at DESC);
CREATE TABLE knowledge_chunk_contexts (
 knowledge_base_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
 document_id TEXT NOT NULL REFERENCES knowledge_base_documents(id) ON DELETE CASCADE,
 source_code TEXT NOT NULL, context_id TEXT NOT NULL, payload_json TEXT NOT NULL,
 PRIMARY KEY(knowledge_base_id,source_code,context_id)
);


CREATE TABLE automatic_router_sources (
    router_kind TEXT PRIMARY KEY, sources_json TEXT NOT NULL CHECK(json_valid(sources_json))
);

CREATE TABLE configured_model_providers (
    provider_id TEXT PRIMARY KEY,
    configuration_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE configured_provider_models (
    provider_id TEXT NOT NULL REFERENCES configured_model_providers(provider_id) ON DELETE CASCADE,
    model_code TEXT NOT NULL UNIQUE,
    PRIMARY KEY (provider_id, model_code)
);

CREATE TABLE navigation_domain_translations (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    domain_id INTEGER NOT NULL REFERENCES navigation_domains(id) ON DELETE CASCADE,
    locale TEXT NOT NULL CHECK (locale IN ('zh-CN', 'en-US')),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    UNIQUE(domain_id, locale)
);

-- tron:navigation-packs:start
CREATE TABLE IF NOT EXISTS navigation_pack_state (
    pack_id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    manifest_json TEXT NOT NULL,
    layouts_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS navigation_pack_ownership (
    pack_id TEXT NOT NULL REFERENCES navigation_pack_state(pack_id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('Domain','Menu')),
    code TEXT NOT NULL,
    PRIMARY KEY(kind, code)
);
-- tron:navigation-packs:end

CREATE TABLE navigation_domains (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    domain_code TEXT NOT NULL UNIQUE,
    built_in INTEGER NOT NULL DEFAULT 0 CHECK (built_in IN (0, 1)),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE navigation_menu_translations (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    menu_id INTEGER NOT NULL REFERENCES navigation_menus(id) ON DELETE CASCADE,
    locale TEXT NOT NULL CHECK (locale IN ('zh-CN', 'en-US')),
    label TEXT NOT NULL,
    UNIQUE(menu_id, locale)
);

CREATE TABLE navigation_menu_tree_items (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    tree_id INTEGER NOT NULL REFERENCES navigation_menu_trees(id) ON DELETE CASCADE,
    menu_id INTEGER NOT NULL REFERENCES navigation_menus(id) ON DELETE RESTRICT,
    parent_item_id INTEGER REFERENCES navigation_menu_tree_items(id) ON DELETE RESTRICT,
    position INTEGER NOT NULL CHECK (position >= 0),
    child_display TEXT NOT NULL DEFAULT 'Inline' CHECK (child_display IN ('Inline', 'Flyout')),
    UNIQUE(tree_id, menu_id)
);

CREATE TABLE navigation_menu_tree_translations (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    tree_id INTEGER NOT NULL REFERENCES navigation_menu_trees(id) ON DELETE CASCADE,
    locale TEXT NOT NULL CHECK (locale IN ('zh-CN', 'en-US')),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    UNIQUE(tree_id, locale)
);

CREATE TABLE navigation_menu_trees (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    tree_code TEXT NOT NULL UNIQUE,
    tree_type TEXT NOT NULL CHECK (tree_type = 'Domain'),
    domain_id INTEGER NOT NULL UNIQUE REFERENCES navigation_domains(id) ON DELETE CASCADE,
    built_in INTEGER NOT NULL DEFAULT 0 CHECK (built_in IN (0, 1)),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE "navigation_menus" (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    menu_code TEXT NOT NULL UNIQUE,
    source TEXT NOT NULL CHECK (source IN ('BuiltIn', 'Custom')),
    menu_type TEXT NOT NULL CHECK (menu_type IN ('Group', 'Internal', 'ExternalLink', 'Widget', 'PackPage')),
    page_code TEXT,
    url TEXT,
    icon TEXT,
    description_zh_cn TEXT NOT NULL DEFAULT '',
    description_en_us TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    CHECK (
        (menu_type = 'Group' AND page_code IS NULL AND url IS NULL)
        OR (menu_type IN ('Internal', 'PackPage') AND page_code IS NOT NULL AND url IS NULL)
        OR (menu_type = 'ExternalLink' AND page_code IS NULL AND url IS NOT NULL)
        OR (menu_type = 'Widget' AND page_code IS NOT NULL AND url IS NULL)
    )
);

CREATE TABLE person_ai_device_identity (
                    singleton_id INTEGER PRIMARY KEY CHECK(singleton_id = 1),
                    device_id TEXT NOT NULL
                );

CREATE TABLE person_ai_profile_credentials (
    person_ai_user_id TEXT PRIMARY KEY,
    person_ai_username TEXT NOT NULL,
    server_base_url TEXT NOT NULL,
    credentials_encrypted TEXT NOT NULL,
    status TEXT NOT NULL,
    authorized_at INTEGER NOT NULL,
    authorization_expires_at INTEGER,
    last_error TEXT,
    updated_at INTEGER NOT NULL
);


CREATE TABLE platform_discovered_tools(tool_code TEXT PRIMARY KEY, config_json TEXT NOT NULL);

CREATE TABLE platform_internal_agent_configs (
    agent_code TEXT PRIMARY KEY,
    config_json TEXT NOT NULL
);

CREATE TABLE platform_main_agent_configs (
    singleton_key TEXT PRIMARY KEY,
    personality TEXT NOT NULL DEFAULT '',
    instructions TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE platform_mcp_gateway_configs (
    singleton_key TEXT PRIMARY KEY,
    gateway_enabled INTEGER NOT NULL,
    variables_endpoint_enabled INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE platform_mcp_servers (
    server_code TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    url TEXT NOT NULL,
    auth_kind TEXT NOT NULL,
    auth_payload_json TEXT,
    headers_json TEXT,
    enabled INTEGER NOT NULL,
    last_health_status TEXT NOT NULL CHECK (last_health_status IN ('never_checked', 'healthy', 'unhealthy')),
    last_health_message TEXT,
    last_health_checked_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
, transport_kind TEXT NOT NULL DEFAULT 'streamable_http', headers_variable_refs_json TEXT NOT NULL DEFAULT '{}', command TEXT NOT NULL DEFAULT '', args_json TEXT NOT NULL DEFAULT '[]', env_variable_refs_json TEXT NOT NULL DEFAULT '{}', env_passthrough_refs_json TEXT NOT NULL DEFAULT '[]', working_directory TEXT);

CREATE TABLE platform_tool_permissions(tool_code TEXT PRIMARY KEY, config_json TEXT NOT NULL);

CREATE TABLE platform_variables (
    id INTEGER PRIMARY KEY,
    variable_key TEXT NOT NULL UNIQUE,
    variable_value TEXT NOT NULL,
    description TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
, is_sensitive INTEGER NOT NULL DEFAULT 0, value_nonce TEXT NULL, value_tag TEXT NULL, value_algorithm TEXT NULL, value_key_version TEXT NULL);

CREATE TABLE project_app_links (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    project_code TEXT NOT NULL REFERENCES projects(project_code) ON DELETE CASCADE,
    app_code TEXT NOT NULL REFERENCES apps(app_code) ON DELETE RESTRICT,
    position INTEGER NOT NULL CHECK (position >= 0),
    is_primary INTEGER NOT NULL CHECK (is_primary IN (0, 1)),
    UNIQUE (project_code, app_code),
    UNIQUE (project_code, position)
);

CREATE TABLE projects (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    project_code TEXT NOT NULL UNIQUE,
    workspace_code TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER
);
CREATE UNIQUE INDEX idx_projects_active_name
    ON projects(workspace_code, name) WHERE status = 'ACTIVE';

CREATE TABLE directories (
    directory_code TEXT PRIMARY KEY,
    workspace_code TEXT NOT NULL,
    execution_host TEXT NOT NULL,
    canonical_path TEXT NOT NULL,
    display_name TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    UNIQUE (workspace_code, execution_host, canonical_path)
);

CREATE TABLE connector_connections (
    connection_code TEXT PRIMARY KEY,
    workspace_code TEXT NOT NULL,
    owner_user_code TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    name TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    encrypted_credential TEXT NOT NULL,
    UNIQUE (workspace_code, owner_user_code, name)
);

CREATE TABLE topic_memos (
    project_code TEXT PRIMARY KEY REFERENCES projects(project_code) ON DELETE CASCADE,
    notes TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0)
);

CREATE TABLE topic_references (
    reference_code TEXT PRIMARY KEY,
    project_code TEXT NOT NULL REFERENCES projects(project_code) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('Directory', 'Repository', 'Resource', 'Task', 'Link')),
    target_code TEXT NOT NULL,
    connection_code TEXT NOT NULL DEFAULT '',
    label TEXT NOT NULL,
    url TEXT,
    UNIQUE (project_code, kind, target_code, connection_code)
);

CREATE TABLE prompt_parameters (
    prompt_code TEXT NOT NULL REFERENCES prompts(prompt_code) ON DELETE CASCADE,
    parameter_name TEXT NOT NULL,
    parameter_type TEXT NOT NULL DEFAULT 'STRING'
        CHECK (parameter_type IN ('STRING', 'NUMBER', 'BOOLEAN', 'JSON')),
    description TEXT NOT NULL CHECK (length(trim(description)) BETWEEN 1 AND 200),
    position INTEGER NOT NULL,
    PRIMARY KEY (prompt_code, parameter_name)
);

CREATE TABLE prompt_translations (
    prompt_code TEXT NOT NULL REFERENCES prompts(prompt_code) ON DELETE CASCADE,
    locale TEXT NOT NULL CHECK (locale IN ('en-US', 'zh-CN')),
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 200),
    prompt_body TEXT NOT NULL CHECK (length(trim(prompt_body)) BETWEEN 1 AND 100000),
    description TEXT NULL,
    translation_status TEXT NOT NULL DEFAULT 'PUBLISHED'
        CHECK (translation_status IN ('PUBLISHED', 'DRAFT', 'UPDATE_AVAILABLE')),
    source_version INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (prompt_code, locale)
);

CREATE TABLE prompt_variables (
    prompt_code TEXT NOT NULL REFERENCES prompts(prompt_code) ON DELETE CASCADE,
    variable_name TEXT NOT NULL,
    variable_type TEXT NOT NULL DEFAULT 'STRING'
        CHECK (variable_type IN ('STRING', 'NUMBER', 'BOOLEAN', 'JSON')),
    description TEXT NOT NULL CHECK (
        length(trim(description)) BETWEEN 1 AND 200
    ),
    position INTEGER NOT NULL,
    source_kind TEXT NOT NULL DEFAULT 'MANAGED' CHECK (source_kind IN ('MANAGED', 'RUNTIME')),
    managed_variable_key TEXT NULL,
    runtime_provider TEXT NULL,
    PRIMARY KEY (prompt_code, variable_name)
);

CREATE TABLE prompts (
    id INTEGER PRIMARY KEY,
    prompt_code TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    category TEXT NOT NULL CHECK (category IN ('SYSTEM', 'DEVELOPER', 'PERSONA', 'TOOL', 'TEMPLATE', 'ACTION', 'AGENT_ROLE')),
    prompt_body TEXT NOT NULL,
    description TEXT NULL,
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    built_in INTEGER NOT NULL DEFAULT 0 CHECK (built_in IN (0, 1)),
    runtime_required INTEGER NOT NULL DEFAULT 0 CHECK (runtime_required IN (0, 1)),
    catalog_key TEXT NULL
);

CREATE TABLE provider_authorization_sessions (
    session_id TEXT PRIMARY KEY,
    provider_code TEXT NOT NULL,
    server_base_url TEXT NOT NULL,
    client_name TEXT NOT NULL,
    login_url TEXT NOT NULL,
    status TEXT NOT NULL,
    expires_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE "provider_catalog_models" (
    tags_json TEXT NOT NULL DEFAULT '[]',
    provider_code TEXT NOT NULL,
    model_code TEXT NOT NULL,
    model_name TEXT NOT NULL,
    upstream_vendor_code TEXT NOT NULL DEFAULT '',
    upstream_vendor_name TEXT,
    context_window_tokens INTEGER,
    protocols TEXT NOT NULL DEFAULT '["ChatCompletions"]',
    quality_type TEXT NOT NULL DEFAULT 'High',
    input_coefficient REAL,
    cache_hit_coefficient REAL,
    output_coefficient REAL,
    status TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(provider_code, model_code, upstream_vendor_code, quality_type)
);

CREATE TABLE provider_connections (
    provider_code TEXT PRIMARY KEY,
    person_ai_user_id TEXT,
    person_ai_username TEXT,
    server_base_url TEXT NOT NULL,
    status TEXT NOT NULL,
    credentials_encrypted TEXT,
    authorized_at INTEGER,
    authorization_expires_at INTEGER,
    last_sync_at INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE provider_model_run_snapshots (
    run_code TEXT PRIMARY KEY,
    session_code TEXT,
    workspace_code TEXT NOT NULL,
    selection_kind TEXT NOT NULL,
    provider_code TEXT NOT NULL,
    requested_model_code TEXT,
    quality_type TEXT,
    route_code TEXT,
    resolved_model_code TEXT,
    resolved_model_name TEXT,
    resolved_upstream_vendor_code TEXT,
    resolved_upstream_vendor_name TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE provider_multimedia_catalog_models (
    upstream_vendor_code TEXT, upstream_vendor_name TEXT,
    input_coefficient REAL, cache_hit_coefficient REAL, output_coefficient REAL,
    dimension INTEGER NULL CHECK (dimension IS NULL OR dimension > 0),
    tags_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(tags_json) AND json_type(tags_json)='array'),
    provider_code TEXT NOT NULL,
    model_id TEXT NOT NULL,
    model_code TEXT NOT NULL,
    model_name TEXT NOT NULL,
    media_type INTEGER NOT NULL,
    status TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    last_seen_at INTEGER,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(provider_code, model_id)
);

CREATE TABLE provider_sync_runs (
    sync_run_code TEXT PRIMARY KEY,
    provider_code TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    model_count INTEGER,
    error_summary TEXT
);

CREATE TABLE rag_corpora (
    corpus_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('enabled', 'reserved', 'disabled')),
    description TEXT,
    capabilities_json TEXT NOT NULL,
    filter_schema_json TEXT NOT NULL,
    field_mapping_json TEXT NOT NULL,
    storage_binding_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE rag_diagnostics (
    id INTEGER PRIMARY KEY,
    record_type TEXT NOT NULL CHECK (record_type IN ('INDEXING_WARNING', 'SEARCH_TRACE')),
    record_code TEXT NOT NULL UNIQUE,
    corpus_id TEXT,
    task_id TEXT,
    source_code TEXT,
    source_version TEXT,
    scope_type TEXT,
    scope_code TEXT,
    workspace_code TEXT,
    user_code TEXT,
    agent_code TEXT,
    stage_id TEXT,
    query_text TEXT,
    message TEXT,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER, "details_json" TEXT, "stage" TEXT, "status" TEXT, "updated_at" TEXT,
    CHECK (
        record_type <> 'SEARCH_TRACE'
        OR (
            task_id IS NOT NULL
            AND workspace_code IS NOT NULL
            AND query_text IS NOT NULL
            AND expires_at IS NOT NULL
        )
    )
);

-- Grafeo owns graph facts; these tables control visibility and rebuildable node indexes.
CREATE TABLE rag_graph_generations (
    generation_id TEXT PRIMARY KEY,
    partition_key TEXT NOT NULL,
    corpus_id TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_code TEXT NOT NULL,
    logical_source_code TEXT NOT NULL,
    source_code TEXT NOT NULL,
    source_version TEXT NOT NULL,
    task_id TEXT NOT NULL,
    configuration_hash TEXT NOT NULL,
    model_code TEXT NOT NULL,
    model_fingerprint TEXT NOT NULL,
    provider_model TEXT,
    vector_cache_invalidated INTEGER NOT NULL DEFAULT 0 CHECK(vector_cache_invalidated IN (0,1)),
    dimension INTEGER NOT NULL CHECK (dimension BETWEEN 1 AND 8192),
    state TEXT NOT NULL CHECK (state IN ('Staging','Ready','Active','Retired','Failed','Cleaning')),
    expected_projections INTEGER NOT NULL CHECK (expected_projections >= 0),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_rag_graph_generation_source
    ON rag_graph_generations(partition_key, logical_source_code, state);

CREATE TABLE rag_graph_active_sources (
    partition_key TEXT NOT NULL,
    logical_source_code TEXT NOT NULL,
    generation_id TEXT NOT NULL REFERENCES rag_graph_generations(generation_id),
    publication_epoch INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (partition_key, logical_source_code)
);

CREATE TABLE rag_graph_node_projections (
    id INTEGER PRIMARY KEY,
    entity_id TEXT NOT NULL,
    generation_id TEXT NOT NULL REFERENCES rag_graph_generations(generation_id),
    chunk_codes_json TEXT NOT NULL,
    text TEXT NOT NULL,
    embedding BLOB NOT NULL,
    UNIQUE (entity_id, generation_id)
);
CREATE INDEX idx_rag_graph_projection_generation ON rag_graph_node_projections(generation_id);

CREATE TABLE rag_graph_cleanup_jobs (
    generation_id TEXT PRIMARY KEY REFERENCES rag_graph_generations(generation_id),
    not_before INTEGER NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    lease_token TEXT,
    lease_until INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_rag_graph_cleanup_due ON rag_graph_cleanup_jobs(not_before,lease_until);

-- Resumable computation, not graph facts. Published-source cache reuse stays scoped.
CREATE TABLE rag_graph_task_checkpoints (
    task_id TEXT NOT NULL,
    cache_scope TEXT NOT NULL,
    checkpoint_key TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (task_id, cache_scope, checkpoint_key)
);
CREATE INDEX idx_rag_graph_checkpoint_scope
    ON rag_graph_task_checkpoints(cache_scope, checkpoint_key, updated_at);

CREATE TABLE rag_graph_query_snapshots (
    snapshot_id TEXT PRIMARY KEY,
    owner_hash TEXT NOT NULL,
    identity_hash TEXT NOT NULL,
    request_json TEXT NOT NULL,
    traversal_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX idx_rag_graph_snapshot_expiry ON rag_graph_query_snapshots(expires_at);
CREATE INDEX idx_rag_graph_snapshot_identity ON rag_graph_query_snapshots(identity_hash, expires_at);
CREATE INDEX idx_rag_graph_snapshot_owner ON rag_graph_query_snapshots(owner_hash, created_at);
CREATE TABLE rag_graph_snapshot_generations (
    snapshot_id TEXT NOT NULL REFERENCES rag_graph_query_snapshots(snapshot_id) ON DELETE CASCADE,
    generation_id TEXT NOT NULL REFERENCES rag_graph_generations(generation_id),
    PRIMARY KEY (snapshot_id, generation_id)
);
CREATE INDEX idx_rag_graph_snapshot_generation ON rag_graph_snapshot_generations(generation_id);

CREATE TABLE rag_memory_text_units (
    id INTEGER PRIMARY KEY,
    text_unit_code TEXT NOT NULL UNIQUE,
    source_code TEXT NOT NULL,
    source_version TEXT NOT NULL,
    scope_type TEXT NOT NULL,
    scope_code TEXT NOT NULL,
    search_text TEXT NOT NULL DEFAULT '',
    fulltext_status TEXT NOT NULL DEFAULT 'NOT_ENABLED',
    embedding_model_code TEXT,
    embedding_version TEXT,
    embedding_text_hash TEXT,
    embedding_dimension INTEGER,
    embedding BLOB,
    embedding_status TEXT NOT NULL DEFAULT 'NOT_ENABLED',
    unit_type TEXT NOT NULL CHECK (unit_type IN ('LEAF', 'SUMMARY', 'SYMBOL', 'NOTE')),
    text TEXT NOT NULL,
    heading_path TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    token_count INTEGER,
    content_hash TEXT,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'DISABLED', 'DELETED')),
    metadata TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    source_status TEXT NOT NULL DEFAULT 'PENDING',
    title TEXT,
    source_content_hash TEXT,
    body_hash TEXT,
    source_metadata TEXT NOT NULL DEFAULT '{}',
    source_created_at INTEGER NOT NULL DEFAULT 0,
    source_updated_at INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER,
    UNIQUE (source_code, source_version, scope_type, scope_code, ordinal)
);

CREATE TABLE rag_record_tasks (
            id TEXT PRIMARY KEY, user_code TEXT NOT NULL, corpus_id TEXT NOT NULL,
            state TEXT NOT NULL, payload_json TEXT NOT NULL, updated_at INTEGER NOT NULL
        );

-- History visibility is user-scoped. Task rows remain as publication/checkpoint fences.
CREATE TABLE rag_task_history_deletions (
    user_code TEXT NOT NULL,
    task_id TEXT NOT NULL,
    deleted_at INTEGER NOT NULL,
    PRIMARY KEY (user_code, task_id)
);

CREATE TABLE rag_skill_text_units (
    id INTEGER PRIMARY KEY,
    text_unit_code TEXT NOT NULL UNIQUE,
    source_code TEXT NOT NULL,
    source_version TEXT NOT NULL,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('USER', 'AGENT', 'SPACE')),
    scope_code TEXT NOT NULL,
    source_status TEXT NOT NULL CHECK (source_status IN ('PENDING', 'ACTIVE', 'FAILED', 'DISABLED', 'ARCHIVED', 'DELETED')),
    title TEXT,
    source_content_hash TEXT,
    source_metadata TEXT NOT NULL,
    source_created_at INTEGER NOT NULL,
    source_updated_at INTEGER NOT NULL,
    skill_name TEXT NOT NULL,
    skill_description TEXT NOT NULL,
    search_text TEXT NOT NULL DEFAULT '',
    fulltext_status TEXT NOT NULL DEFAULT 'NOT_ENABLED',
    embedding_model_code TEXT,
    embedding_version TEXT,
    embedding_text_hash TEXT,
    embedding_dimension INTEGER,
    embedding BLOB,
    embedding_status TEXT NOT NULL DEFAULT 'NOT_ENABLED',
    unit_type TEXT NOT NULL CHECK (unit_type IN ('LEAF', 'SUMMARY', 'SYMBOL', 'NOTE')),
    text TEXT NOT NULL,
    heading_path TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    token_count INTEGER,
    content_hash TEXT,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'DISABLED', 'DELETED')),
    metadata TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, "body_hash" TEXT,
    UNIQUE (source_code, source_version, scope_type, scope_code, ordinal)
);

CREATE TABLE registry_skills (
    skill_name TEXT PRIMARY KEY,
    provider_pack_id TEXT,
    available INTEGER NOT NULL DEFAULT 1,
    source_ref TEXT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('Marketplace', 'GitHub', 'Git', 'Url', 'Local', 'Manual', 'BuiltIn')),
    registry_path TEXT NOT NULL,
    declared_version TEXT NULL,
    compatibility TEXT NULL,
    update_status TEXT NOT NULL CHECK (update_status IN ('Unknown', 'UpToDate', 'UpdateAvailable', 'Manual', 'Error')),
    managed_by TEXT NOT NULL CHECK (managed_by IN ('Cli', 'Manual', 'BuiltIn')),
    last_checked_at INTEGER NULL,
    last_updated_at INTEGER NULL,
    last_error TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE resource_chat_session_attachments (
    id INTEGER PRIMARY KEY,
    workspace_code TEXT NOT NULL COLLATE BINARY,
    session_code TEXT NOT NULL COLLATE BINARY,
    attachment_code TEXT NOT NULL COLLATE BINARY,
    file_name TEXT NOT NULL,
    content_type TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK (byte_size > 0),
    storage_locator TEXT NOT NULL COLLATE BINARY,
    created_at INTEGER NOT NULL,
    UNIQUE(session_code, attachment_code)
);

CREATE TABLE resource_code_registry (
    workspace_code TEXT NOT NULL COLLATE BINARY,
    resource_code TEXT NOT NULL COLLATE BINARY,
    kind TEXT NOT NULL,
    PRIMARY KEY (workspace_code, resource_code)
);

CREATE TABLE resource_files (
    id INTEGER PRIMARY KEY,
    workspace_code TEXT NOT NULL COLLATE BINARY,
    resource_code TEXT NOT NULL COLLATE BINARY,
    kind TEXT NOT NULL CHECK (kind IN ('FILE', 'IMAGE', 'PDF')),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    file_name TEXT NOT NULL,
    file_extension TEXT NOT NULL DEFAULT '',
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
    storage_locator TEXT NOT NULL COLLATE BINARY,
    allow_inheritance INTEGER NOT NULL CHECK (allow_inheritance IN (0, 1)) DEFAULT 0,
    revision INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(workspace_code, resource_code)
);

CREATE TABLE skill_rag_index_states (
    skill_name TEXT NOT NULL,
    source_version TEXT NOT NULL,
    desired_enabled INTEGER NOT NULL,
    vector_status TEXT NOT NULL CHECK (vector_status IN ('PENDING', 'INDEXED', 'DISABLED', 'WARNING', 'FAILED', 'STALE')),
    graph_status TEXT NOT NULL CHECK (graph_status IN ('PENDING', 'INDEXED', 'DISABLED', 'WARNING', 'FAILED', 'STALE')),
    content_hash TEXT NULL,
    structure_hash TEXT NULL,
    last_indexed_at INTEGER NULL,
    last_error TEXT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (skill_name)
);

CREATE TABLE system_agent_registries (
    fast_mode INTEGER NOT NULL DEFAULT 0,
    reasoning_effort TEXT,
    avatar_id TEXT,
    agent_code TEXT PRIMARY KEY,
    base_agent_code TEXT,
    model_id TEXT,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    runtime_type TEXT NOT NULL,
    command_name TEXT NOT NULL,
    source TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    available INTEGER NOT NULL DEFAULT 0,
    last_scanned_at INTEGER,
    last_seen_at INTEGER,
    last_error TEXT
);

CREATE TABLE task_agent_links (
    id INTEGER PRIMARY KEY,
    task_code TEXT NOT NULL REFERENCES tasks(task_code),
    agent_code TEXT NOT NULL,
    linked_at INTEGER NOT NULL,
    UNIQUE (task_code, agent_code)
);

CREATE TABLE task_app_links (
    id INTEGER PRIMARY KEY,
    task_code TEXT NOT NULL REFERENCES tasks(task_code),
    app_code TEXT NOT NULL,
    linked_at INTEGER NOT NULL,
    UNIQUE (task_code, app_code)
);

CREATE TABLE task_flow_statuses (
    flow_code TEXT NOT NULL REFERENCES task_flows(flow_code) ON DELETE CASCADE,
    status_code TEXT NOT NULL,
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    required INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
    PRIMARY KEY (flow_code, status_code),
    UNIQUE (flow_code, position)
);

CREATE TABLE task_flows (
    flow_code TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE "tasks" (
                id INTEGER PRIMARY KEY,
                task_code TEXT NOT NULL UNIQUE,
                workspace_code TEXT NULL,
                owner_dude_code TEXT NULL,
                due_at INTEGER NULL,
                planned_start_at INTEGER NULL,
                next_followup_at INTEGER NULL,
                work_session_code TEXT NULL,
                schedule_version INTEGER NOT NULL DEFAULT 0,
                parent_task_code TEXT NULL REFERENCES tasks(task_code),
                title TEXT NOT NULL,
                description TEXT NULL,
                execution_result TEXT NULL,
                flow_code TEXT NOT NULL,
                status TEXT NOT NULL,
                previous_status TEXT NULL,
                priority TEXT NOT NULL CHECK (priority IN ('Low', 'Medium', 'High', 'Extra High')),
                task_type TEXT NOT NULL CHECK (task_type IN ('General', 'Feature', 'Bug')),
                source_type TEXT NOT NULL CHECK (source_type IN ('Internal', 'GitHub', 'GitLab')),
                source_url TEXT NULL,
                agent_code TEXT NOT NULL DEFAULT 'main-agent',
                model_code TEXT NULL,
                progress_value INTEGER NOT NULL DEFAULT 0 CHECK (progress_value >= 0 AND progress_value <= 100),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                archived_at INTEGER NULL
            );


CREATE TABLE workspace_internal_agent_definitions (
    workspace_code TEXT NOT NULL,
    agent_code TEXT NOT NULL,
    name TEXT,
    description TEXT NULL CHECK (description IS NULL OR length(trim(description)) <= 200),
    role_prompt TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, avatar_id TEXT, group_name TEXT,
    candidate_model_codes_json TEXT NOT NULL DEFAULT 'null',
    PRIMARY KEY (workspace_code, agent_code)
);

CREATE TABLE workspace_internal_agent_mcp_server_assignments (
    workspace_code TEXT NOT NULL,
    agent_code TEXT NOT NULL,
    server_code TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (workspace_code, agent_code, server_code),
    FOREIGN KEY (workspace_code, agent_code)
        REFERENCES workspace_internal_agent_policies (workspace_code, agent_code)
        ON DELETE CASCADE,
    FOREIGN KEY (server_code)
        REFERENCES platform_mcp_servers (server_code)
        ON DELETE CASCADE
);

CREATE TABLE workspace_internal_agent_policies (
    selected_category_codes TEXT NOT NULL DEFAULT '[]',
    workspace_code TEXT NOT NULL,
    agent_code TEXT NOT NULL,
    tool_assignment_mode TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, max_steps INTEGER NOT NULL DEFAULT 0, max_tool_calls INTEGER NOT NULL DEFAULT 0, max_parallel_tool_calls INTEGER NOT NULL DEFAULT 4, mcp_access_mode TEXT NOT NULL DEFAULT 'DISABLED', overridden_fields TEXT NOT NULL
                 DEFAULT '["toolAssignmentMode","mcpAccessMode","maxSteps","maxToolCalls","maxParallelToolCalls","selectedToolCodes","selectedServerCodes"]',
    PRIMARY KEY (workspace_code, agent_code)
);

CREATE TABLE workspace_internal_agent_tool_assignments (
    workspace_code TEXT NOT NULL,
    agent_code TEXT NOT NULL,
    tool_code TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (workspace_code, agent_code, tool_code)
);

CREATE TABLE workspace_mcp_server_policies (
    workspace_code TEXT NOT NULL,
    server_code TEXT NOT NULL REFERENCES platform_mcp_servers(server_code) ON DELETE CASCADE,
    enabled INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (workspace_code, server_code)
);

CREATE TABLE workspace_session_links (session_id TEXT PRIMARY KEY, workspace_code TEXT NOT NULL, business_json TEXT NOT NULL);

CREATE TABLE agent_skill_assignments (
    agent_code TEXT NOT NULL,
    skill_name TEXT NOT NULL REFERENCES registry_skills(skill_name) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (agent_code, skill_name)
);

CREATE TABLE capability_resource_ownership (
    pack_id TEXT NOT NULL,
    resource_kind TEXT NOT NULL CHECK(resource_kind IN ('skill','tool','mcp')),
    resource_code TEXT NOT NULL,
    available INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY(resource_kind, resource_code)
);

CREATE TABLE workspace_system_agent_policies (
    workspace_code TEXT NOT NULL,
    agent_code TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (workspace_code, agent_code)
);

CREATE TABLE workspace_task_flows (
    workspace_code TEXT PRIMARY KEY,
    flow_code TEXT NOT NULL REFERENCES task_flows(flow_code),
    updated_at INTEGER NOT NULL
);

CREATE TABLE workspace_variables (
    id INTEGER PRIMARY KEY,
    workspace_code TEXT NOT NULL,
    variable_key TEXT NOT NULL,
    variable_value TEXT NOT NULL,
    description TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, is_sensitive INTEGER NOT NULL DEFAULT 0, value_nonce TEXT NULL, value_tag TEXT NULL, value_algorithm TEXT NULL, value_key_version TEXT NULL,
    UNIQUE (workspace_code, variable_key)
);

CREATE TABLE workspaces (
    id INTEGER PRIMARY KEY,
    workspace_code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NULL,
    parent_workspace_code TEXT NULL REFERENCES workspaces(workspace_code),
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    archived_at INTEGER NULL
);

CREATE INDEX idx_app_tags_app_code ON app_tags(app_code);

CREATE INDEX idx_apps_workspace_name ON apps(workspace_code, app_name);

CREATE INDEX idx_apps_workspace_status ON apps(workspace_code, status);

CREATE INDEX idx_automation_executions_automation_started ON automation_execution_records(automation_code, started_at DESC);

CREATE INDEX idx_automation_executions_session ON automation_execution_records(session_code);

CREATE INDEX idx_automation_executions_workspace_started ON automation_execution_records(workspace_code, started_at DESC);

CREATE UNIQUE INDEX idx_automation_scheduler_tick ON automation_execution_records(automation_code, trigger_source, scheduled_at)
WHERE trigger_source = 'Scheduler' AND scheduled_at IS NOT NULL;

CREATE INDEX idx_automations_workspace_status ON automations(workspace_code, status);

CREATE INDEX idx_automations_workspace_updated ON automations(workspace_code, updated_at);

CREATE INDEX idx_chat_session_auto_model_routes_session
    ON chat_session_auto_model_routes(session_code);

CREATE INDEX idx_kb_derived_resource_reference
 ON knowledge_base_derived_resources(workspace_code,resource_code);

CREATE INDEX idx_kb_documents_parent ON knowledge_base_documents(knowledge_base_id);

CREATE INDEX idx_kb_ingestion_claim
 ON knowledge_base_ingestion_tasks(state,stage,retry_after_ms,lease_until_ms);
CREATE INDEX idx_kb_ingestion_library_document
 ON knowledge_base_ingestion_tasks(knowledge_base_id,document_id);

-- Workers seek a bounded size/FIFO page without sorting the entire backlog.
CREATE INDEX idx_kb_ingestion_parse_order
 ON knowledge_base_ingestion_tasks(
     (json_extract(payload_json,'$.fileSizeBytes') IS NULL),
     COALESCE(json_extract(payload_json,'$.fileSizeBytes'),0))
 WHERE stage IN ('Parsing','Chunking')
   AND state IN ('QUEUED','RUNNING')
   AND COALESCE(json_extract(payload_json,'$.cancellationPending'),0)=0;
CREATE INDEX idx_kb_ingestion_embed_order
 ON knowledge_base_ingestion_tasks(
     (json_extract(payload_json,'$.fileSizeBytes') IS NULL),
     COALESCE(json_extract(payload_json,'$.fileSizeBytes'),0))
 WHERE stage IN ('Embedding','Publishing')
   AND state IN ('QUEUED','RUNNING')
   AND COALESCE(json_extract(payload_json,'$.cancellationPending'),0)=0;

CREATE UNIQUE INDEX idx_kb_ingestion_one_live_document
 ON knowledge_base_ingestion_tasks(document_id)
 WHERE state IN ('QUEUED', 'RUNNING');

CREATE INDEX idx_knowledge_bases_owner ON knowledge_bases(owner_user_code);

CREATE INDEX idx_memory_work_domains_status
    ON memory_work_domains (status, name);





CREATE UNIQUE INDEX idx_navigation_menu_tree_item_position
ON navigation_menu_tree_items (tree_id, COALESCE(parent_item_id, 0), position);

CREATE INDEX idx_person_ai_profile_credentials_status
    ON person_ai_profile_credentials(status, updated_at DESC);


CREATE INDEX idx_platform_mcp_servers_enabled_updated_at
    ON platform_mcp_servers (enabled, updated_at);

CREATE INDEX idx_platform_variables_variable_key
    ON platform_variables(variable_key);

CREATE UNIQUE INDEX idx_project_app_links_primary
    ON project_app_links(project_code)
    WHERE is_primary = 1;

CREATE INDEX idx_projects_workspace_updated
    ON projects(workspace_code, status, updated_at DESC, id DESC);

CREATE INDEX idx_prompt_parameters_prompt_position
    ON prompt_parameters(prompt_code, position);

CREATE INDEX idx_prompt_translations_locale
    ON prompt_translations(locale, prompt_code);

CREATE INDEX idx_prompt_variables_prompt_position
    ON prompt_variables(prompt_code, position);

CREATE UNIQUE INDEX idx_prompts_catalog_key
    ON prompts(catalog_key)
    WHERE catalog_key IS NOT NULL;

CREATE UNIQUE INDEX idx_prompts_display_name
    ON prompts(display_name);

CREATE INDEX idx_provider_authorization_sessions_provider_updated
    ON provider_authorization_sessions(provider_code, updated_at DESC);

CREATE INDEX idx_provider_catalog_models_provider_status
    ON provider_catalog_models(provider_code, status, model_name);

CREATE INDEX idx_provider_multimedia_catalog_models_provider_status
    ON provider_multimedia_catalog_models(provider_code, status, media_type, model_name);
CREATE INDEX idx_provider_multimedia_catalog_models_embedding_route
    ON provider_multimedia_catalog_models(provider_code, media_type, status, model_code, model_id);

CREATE INDEX idx_provider_run_snapshots_session
    ON provider_model_run_snapshots(session_code);

CREATE INDEX idx_provider_sync_runs_provider_started
    ON provider_sync_runs(provider_code, started_at DESC);

CREATE INDEX "idx_rag_diagnostics_source_scope"
                ON "rag_diagnostics" (source_code, source_version, scope_type, scope_code, stage);


CREATE INDEX "idx_rag_memory_text_units_source_scope"
                ON "rag_memory_text_units" (source_code, source_version, scope_type, scope_code, ordinal);

CREATE INDEX idx_rag_record_tasks_owner ON rag_record_tasks(user_code, updated_at);

CREATE INDEX "idx_rag_skill_text_units_source_scope"
                ON "rag_skill_text_units" (source_code, source_version, scope_type, scope_code, ordinal);

CREATE INDEX idx_registry_skills_managed_by
    ON registry_skills(managed_by);

CREATE INDEX idx_registry_skills_source_type
    ON registry_skills(source_type);

CREATE INDEX idx_resource_chat_session_attachments_session
    ON resource_chat_session_attachments (session_code, created_at, id);

CREATE INDEX idx_resource_chat_session_attachments_workspace
    ON resource_chat_session_attachments (workspace_code);

CREATE INDEX idx_resource_files_workspace
    ON resource_files (workspace_code);

CREATE INDEX idx_skill_rag_index_states_status
    ON skill_rag_index_states(vector_status, graph_status);

CREATE INDEX idx_task_agent_links_task_code ON task_agent_links(task_code);

CREATE INDEX idx_task_app_links_task_code ON task_app_links(task_code);

CREATE INDEX idx_task_flow_statuses_flow_position
    ON task_flow_statuses(flow_code, position);

CREATE INDEX idx_tasks_dude_followup ON tasks(owner_dude_code, next_followup_at) WHERE owner_dude_code IS NOT NULL AND archived_at IS NULL;

CREATE INDEX idx_tasks_parent_task_code ON tasks(parent_task_code);

CREATE INDEX idx_tasks_workspace_flow ON tasks(workspace_code, flow_code);

-- Supports count and ordered pagination without decoding or sorting every task.
CREATE INDEX idx_tasks_list_scope ON tasks(owner_dude_code, workspace_code, task_code, archived_at);
CREATE INDEX idx_tasks_owner_list ON tasks(owner_dude_code, task_code, archived_at);

CREATE INDEX idx_tasks_workspace_status ON tasks(workspace_code, status);


CREATE INDEX idx_workspace_internal_agent_mcp_server_assignments_workspace
    ON workspace_internal_agent_mcp_server_assignments (workspace_code, agent_code);

CREATE INDEX idx_workspace_internal_agent_policies_workspace_code
    ON workspace_internal_agent_policies (workspace_code);

CREATE INDEX idx_workspace_internal_agent_tool_assignments_agent_code
    ON workspace_internal_agent_tool_assignments (agent_code);

CREATE INDEX idx_workspace_internal_agent_tool_assignments_workspace_code
    ON workspace_internal_agent_tool_assignments (workspace_code);

CREATE INDEX idx_workspace_mcp_server_policies_workspace_code
    ON workspace_mcp_server_policies (workspace_code);



CREATE INDEX idx_workspace_system_agent_policies_workspace_code
    ON workspace_system_agent_policies (workspace_code);

CREATE INDEX idx_workspace_variables_variable_key
    ON workspace_variables(variable_key);

CREATE INDEX idx_workspace_variables_workspace_code
    ON workspace_variables(workspace_code);

CREATE INDEX idx_workspaces_parent_workspace_code
    ON workspaces(parent_workspace_code);

CREATE INDEX idx_workspaces_status
    ON workspaces(status);

CREATE INDEX workspace_session_links_workspace ON workspace_session_links(workspace_code);
CREATE INDEX workspace_session_links_owner_source ON workspace_session_links(json_extract(business_json,'$.user_code'),json_extract(business_json,'$.source_code'));



CREATE TRIGGER trg_prompts_immutable_identity
BEFORE UPDATE ON prompts
FOR EACH ROW
WHEN NEW.prompt_code <> OLD.prompt_code
    OR (NEW.category <> OLD.category AND NOT (OLD.built_in = 1 AND NEW.built_in = 1))
BEGIN
    SELECT RAISE(ABORT, 'prompt identity fields are immutable');
END;



-- 内置目录数据

-- memory_work_domains
INSERT INTO memory_work_domains (domain_code, name, description, status, created_at, updated_at) VALUES
    ('main-domain', 'main-domain', 'Default work domain used by existing agent memory tools.', 'ACTIVE', 1781568000000, 1781568000000),
    ('frontend', 'frontend', 'Frontend product and interface implementation knowledge.', 'ACTIVE', 1781568000000, 1781568000000),
    ('backend', 'backend', 'Backend service, storage, and API implementation knowledge.', 'ACTIVE', 1781568000000, 1781568000000),
    ('testing', 'testing', 'Testing, validation, and quality assurance knowledge.', 'ACTIVE', 1781568000000, 1781568000000),
    ('product', 'product', 'Product design, requirements, and workflow knowledge.', 'ACTIVE', 1781568000000, 1781568000000);

-- Default navigation definitions and arrangement.

INSERT INTO navigation_domains (id, domain_code, built_in, enabled, is_default, position, created_at, updated_at) VALUES
    (734500000000000001, 'ADVANCED', 1, 1, 1, 0, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z');

INSERT INTO navigation_domain_translations (id, domain_id, locale, name, description) VALUES
    (734500100000000001, 734500000000000001, 'zh-CN', '日常工作', '日常工作'),
    (734500100000000002, 734500000000000001, 'en-US', 'Daily Work', 'Daily Work');

INSERT INTO navigation_menus (id, menu_code, source, menu_type, page_code, url, icon, description_zh_cn, description_en_us, enabled, created_at, updated_at) VALUES
    (734510000000000002, 'APPS', 'BuiltIn', 'Internal', 'apps.index', NULL, 'FolderGit2', '查看和管理工作区中的应用。', 'Browse and manage applications in the workspace.', 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z'),
    (734510000000000003, 'TASKS', 'BuiltIn', 'Internal', 'tasks.index', NULL, 'ListTodo', '创建、查看和跟踪工作任务。', 'Create, review, and track work tasks.', 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z'),
    (734510000000000004, 'AUTOMATIONS', 'BuiltIn', 'Internal', 'automations.index', NULL, 'AlarmClock', '配置自动化规则并查看执行情况。', 'Configure automation rules and review their executions.', 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z'),
    (734510000000000008, 'MCP_SERVERS', 'BuiltIn', 'Internal', 'mcpServers.index', NULL, 'PlugZap', '配置和管理可供智能体使用的 MCP 服务。', 'Configure MCP servers available to agents.', 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z'),
    (734510000000000018, 'FILE_MANAGEMENT', 'BuiltIn', 'Internal', 'resourceManagement.files', NULL, 'Folder', '浏览和管理工作区文件与已上传资源。', 'Browse and manage workspace files and uploaded resources.', 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z'),
    (734510000000000022, 'VARIABLES', 'BuiltIn', 'Internal', 'variables.index', NULL, 'KeyRound', '管理平台或工作区可复用的变量。', 'Manage reusable platform or workspace variables.', 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z'),
    (734550000000000003, 'CHAT_PROJECTS', 'BuiltIn', 'Widget', 'chat.projects', NULL, 'FolderKanban', '查看主题及其关联的会话。', 'Browse topics and their associated conversations.', 1, '2026-09-22T00:00:00Z', '2026-09-22T00:00:00Z'),
    (734550000000000004, 'CHAT_RECENTS', 'BuiltIn', 'Widget', 'chat.recents', NULL, 'History', '查看并继续随聊。', 'View and continue quick chats.', 1, '2026-09-22T00:00:00Z', '2026-09-22T00:00:00Z'),
    (734550000000000005, 'CHAT_DIRECTORIES', 'BuiltIn', 'Widget', 'chat.directories', NULL, 'Folder', '查看工作区及其会话。', 'Browse projects and their conversations.', 1, '2026-09-22T00:00:00Z', '2026-09-22T00:00:00Z'),
    (734560000000000001, 'DUDES', 'BuiltIn', 'Internal', 'dudes.index', NULL, 'Bot', '与 Dude 助手交互，并查看其工作和历史记录。', 'Interact with Dude assistants and review their work and history.', 1, '2026-10-11T00:00:00Z', '2026-10-11T00:00:00Z'),
    (734560000000000002, 'NOTIFICATIONS', 'BuiltIn', 'Internal', 'notifications.index', NULL, 'Bell', '查看消息通知并处理需要关注的事项。', 'Review notifications and handle items that need attention.', 1, '2026-10-11T00:00:00Z', '2026-10-11T00:00:00Z'),
    (734570000000000001, 'EXPLORE_MORE', 'BuiltIn', 'Group', NULL, NULL, 'Ellipsis', '收纳不常用或可选的菜单入口。', 'Collect optional or less frequently used menu entries.', 1, '2026-10-13T00:00:00Z', '2026-10-13T00:00:00Z');

INSERT INTO navigation_menu_translations (id, menu_id, locale, label) VALUES
    (734530000000000003, 734510000000000002, 'zh-CN', '应用'),
    (734530000000000004, 734510000000000002, 'en-US', 'Apps'),
    (734530000000000005, 734510000000000003, 'zh-CN', '任务'),
    (734530000000000006, 734510000000000003, 'en-US', 'Tasks'),
    (734530000000000007, 734510000000000004, 'zh-CN', '自动化'),
    (734530000000000008, 734510000000000004, 'en-US', 'Automations'),
    (734530000000000015, 734510000000000008, 'zh-CN', 'MCP 服务'),
    (734530000000000016, 734510000000000008, 'en-US', 'MCP Servers'),
    (734530000000000035, 734510000000000018, 'zh-CN', '文件管理'),
    (734530000000000036, 734510000000000018, 'en-US', 'File Management'),
    (734530000000000043, 734510000000000022, 'zh-CN', '变量'),
    (734530000000000044, 734510000000000022, 'en-US', 'Variables'),
    (734550100000000005, 734550000000000003, 'zh-CN', '主题'),
    (734550100000000006, 734550000000000003, 'en-US', 'Topics'),
    (734550100000000007, 734550000000000004, 'zh-CN', '随聊'),
    (734550100000000008, 734550000000000004, 'en-US', 'Quick Chats'),
    (734550100000000009, 734550000000000005, 'zh-CN', '工作区'),
    (734550100000000010, 734550000000000005, 'en-US', 'Projects'),
    (734560100000000001, 734560000000000001, 'zh-CN', '好兄弟'),
    (734560100000000002, 734560000000000001, 'en-US', 'Dude'),
    (734560100000000003, 734560000000000002, 'zh-CN', '通知'),
    (734560100000000004, 734560000000000002, 'en-US', 'Notifications'),
    (734570100000000001, 734570000000000001, 'zh-CN', '探索更多'),
    (734570100000000002, 734570000000000001, 'en-US', 'Explore more');

INSERT INTO navigation_menu_trees (id, tree_code, tree_type, domain_id, built_in, enabled, created_at, updated_at) VALUES
    (734500000000000001, 'ADVANCED', 'Domain', 734500000000000001, 1, 1, '2026-08-29T00:00:00Z', '2026-08-29T00:00:00Z');

INSERT INTO navigation_menu_tree_items (id, tree_id, menu_id, parent_item_id, position, child_display) VALUES
    (734560200000000001, 734500000000000001, 734560000000000001, NULL, 0, 'Inline'),
    (734520000000000003, 734500000000000001, 734510000000000003, NULL, 1, 'Inline'),
    (734520000000000004, 734500000000000001, 734510000000000004, NULL, 2, 'Inline'),
    (734570200000000001, 734500000000000001, 734570000000000001, NULL, 3, 'Flyout'),
    (734520000000000002, 734500000000000001, 734510000000000002, 734570200000000001, 0, 'Inline'),
    (734560200000000002, 734500000000000001, 734560000000000002, 734570200000000001, 1, 'Inline'),
    (734520000000000022, 734500000000000001, 734510000000000022, 734570200000000001, 2, 'Inline'),
    (734520000000000008, 734500000000000001, 734510000000000008, 734570200000000001, 3, 'Inline'),
    (734520000000000018, 734500000000000001, 734510000000000018, 734570200000000001, 4, 'Inline'),
    (734550200000000003, 734500000000000001, 734550000000000003, NULL, 4, 'Inline'),
    (734550200000000005, 734500000000000001, 734550000000000005, NULL, 5, 'Inline'),
    (734550200000000004, 734500000000000001, 734550000000000004, NULL, 6, 'Inline');

-- task_flows
INSERT INTO task_flows (flow_code, name, created_at, updated_at) VALUES
    ('DEFAULT', '默认流程', CAST(strftime('%s', 'now') AS INTEGER) * 1000, CAST(strftime('%s', 'now') AS INTEGER) * 1000);

-- task_flow_statuses
INSERT INTO task_flow_statuses (flow_code, status_code, name, position, required) VALUES
    ('DEFAULT', 'NOT_STARTED', '未开始', 0, 1),
    ('DEFAULT', 'IN_PROGRESS', '进行中', 1, 0),
    ('DEFAULT', 'REOPEN', '重新打开', 2, 0),
    ('DEFAULT', 'CLOSED', '关闭', 3, 1);

-- rag_corpora
INSERT INTO rag_corpora (corpus_id, display_name, status, description, capabilities_json, filter_schema_json, field_mapping_json, storage_binding_json, created_at, updated_at) VALUES
    ('memory-card', '记忆', 'enabled', 'User, agent, and space scoped memory cards.', '{"fulltext":true,"vector":true,"metadataGroups":[{"name":"memory","fields":["expiresAt"]}],"settings":{"fulltext":true,"vector":true,"graph":false,"embeddingModelCode":"PERSON_AI:ROUTE:EmbeddingAuto","rerankModelCode":"PERSON_AI:ROUTE:RerankAuto","graphModelCode":"PERSON_AI:ROUTE:ChatAuto"},"graph":false}', '{"fields":{"scopeType":{"type":"string","column":"scope_type"},"scopeCode":{"type":"string","column":"scope_code"},"sourceCode":{"type":"string","column":"source_code"},"sourceVersion":{"type":"string","column":"source_version"},"status":{"type":"string","column":"source_status"},"sourceStatus":{"type":"string","column":"source_status"},"textUnitStatus":{"type":"string","column":"status"},"expiresAt":{"type":"integer","column":"expires_at","operators":["eq","in","gt","gte","lt","lte","gtOrNull"]}}}', '{"source":{"sourceCode":"source_code","sourceVersion":"source_version","scopeType":"scope_type","scopeCode":"scope_code","status":"source_status","sourceStatus":"source_status","title":"title","sourceContentHash":"source_content_hash","sourceMetadata":"source_metadata","expiresAt":"expires_at","sourceCreatedAt":"source_created_at","sourceUpdatedAt":"source_updated_at"},"textUnit":{"sourceCode":"source_code","sourceVersion":"source_version","scopeType":"scope_type","scopeCode":"scope_code","textUnitStatus":"status","text":"text"}}', '{"mode":"TARGETED","bindings":{"source":"memory-store","textUnit":"memory-store","fulltext":"memory-store","vector":"memory-store","diagnostics":"diagnostics-store"},"targets":[{"id":"memory-store","backend":"SQLITE","layout":"TABLE","table":"rag_memory_text_units"},{"id":"diagnostics-store","backend":"SQLITE","layout":"TABLE","table":"rag_diagnostics"}]}', 1772294400000, 1781568000000),
    ('skill', '技能', 'enabled', 'Platform-shared skill content filtered by Agent assignment and source-linked graph evidence.', '{"fulltext":true,"vector":true,"metadataGroups":[{"name":"skill","fields":["skillName","skillDescription"]}],"settings":{"fulltext":true,"vector":true,"graph":true,"embeddingModelCode":"PERSON_AI:ROUTE:EmbeddingAuto","rerankModelCode":"PERSON_AI:ROUTE:RerankAuto","graphModelCode":"PERSON_AI:ROUTE:ChatAuto"},"graph":true}', '{"fields":{"scopeType":{"type":"string","column":"scope_type"},"scopeCode":{"type":"string","column":"scope_code"},"sourceCode":{"type":"string","column":"source_code"},"sourceVersion":{"type":"string","column":"source_version"},"status":{"type":"string","column":"source_status"},"sourceStatus":{"type":"string","column":"source_status"},"textUnitStatus":{"type":"string","column":"status"},"skillName":{"type":"string","column":"skill_name"}}}', '{"source":{"sourceCode":"source_code","sourceVersion":"source_version","scopeType":"scope_type","scopeCode":"scope_code","status":"source_status","sourceStatus":"source_status","title":"title","sourceContentHash":"source_content_hash","sourceMetadata":"source_metadata","skillName":"skill_name","skillDescription":"skill_description"},"textUnit":{"sourceCode":"source_code","sourceVersion":"source_version","scopeType":"scope_type","scopeCode":"scope_code","textUnitStatus":"status","text":"text"}}', '{"mode":"TARGETED","bindings":{"source":"skill-store","textUnit":"skill-store","fulltext":"skill-store","vector":"skill-store","diagnostics":"diagnostics-store"},"targets":[{"id":"skill-store","backend":"SQLITE","layout":"TABLE","table":"rag_skill_text_units"},{"id":"diagnostics-store","backend":"SQLITE","layout":"TABLE","table":"rag_diagnostics"}]}', 1780339200000, 1780339200000);

CREATE TABLE IF NOT EXISTS tron_schema_initialized (scope TEXT PRIMARY KEY);
INSERT INTO tron_schema_initialized (scope) VALUES ('business');
INSERT INTO tron_schema_initialized (scope) VALUES ('builtin-unlimited-run-counts');


-- Platform-owned always-on agents, sessions and durable delivery.
CREATE TABLE dudes (
 dude_code TEXT PRIMARY KEY, record_json TEXT NOT NULL, enabled INTEGER NOT NULL,
 heartbeat_minutes INTEGER NOT NULL CHECK(heartbeat_minutes>0), next_heartbeat_at INTEGER NOT NULL,
 is_main INTEGER NOT NULL DEFAULT 0 CHECK(is_main IN (0,1))
);
CREATE UNIQUE INDEX dudes_single_main ON dudes(is_main) WHERE is_main=1;
-- Souls are platform identities; builtin seeds exist before runtime definitions.
CREATE TABLE dude_souls (dude_code TEXT PRIMARY KEY, record_json TEXT NOT NULL CHECK(json_valid(record_json)));
INSERT INTO dude_souls VALUES
('main-dude','{"dudeCode":"main-dude","name":"好兄弟","nameConfirmed":true,"markdown":"# 性格与相处方式\n真诚、友善、好奇，清晰直接地沟通；尊重用户的选择，主动协助解决问题。","state":"Ready","sessionCode":null,"userCode":null,"pendingInteractionId":null,"awakeningStarted":false,"awakeningDeferred":false}'),
('memory-dude','{"dudeCode":"memory-dude","name":"记忆整理","nameConfirmed":true,"markdown":"谨慎、诚实、简洁。区分事实、推测和经验，保留不确定性。尊重用户选择，不将观察到的偏好擅自升级为稳定约定。","state":"Ready","sessionCode":null,"userCode":null,"pendingInteractionId":null,"awakeningStarted":false,"awakeningDeferred":false}');
-- Dude config serialization deliberately omits the authoritative soul name.
CREATE TRIGGER dude_config_name_insert AFTER INSERT ON platform_internal_agent_configs
WHEN EXISTS(SELECT 1 FROM dude_souls WHERE dude_code=NEW.agent_code)
BEGIN UPDATE platform_internal_agent_configs SET config_json=json_remove(NEW.config_json,'$.name') WHERE agent_code=NEW.agent_code; END;
CREATE TRIGGER dude_config_name_update AFTER UPDATE OF config_json ON platform_internal_agent_configs
WHEN EXISTS(SELECT 1 FROM dude_souls WHERE dude_code=NEW.agent_code) AND json_type(NEW.config_json,'$.name') IS NOT NULL
BEGIN UPDATE platform_internal_agent_configs SET config_json=json_remove(NEW.config_json,'$.name') WHERE agent_code=NEW.agent_code; END;
CREATE TABLE dude_sessions (
 session_code TEXT PRIMARY KEY, dude_code TEXT NOT NULL REFERENCES dudes(dude_code),
 purpose TEXT NOT NULL, correlation_key TEXT NOT NULL, record_json TEXT NOT NULL,
 UNIQUE(dude_code,purpose,correlation_key)
);
CREATE TABLE dude_events (
 event_id TEXT PRIMARY KEY, dude_code TEXT NOT NULL REFERENCES dudes(dude_code),
 dedup_key TEXT NOT NULL UNIQUE, source TEXT NOT NULL, session_code TEXT,
 status TEXT NOT NULL, available_at INTEGER NOT NULL, lease_until INTEGER, lease_token TEXT,
 attempts INTEGER NOT NULL DEFAULT 0, record_json TEXT NOT NULL
);
CREATE INDEX dude_events_ready ON dude_events(status,available_at,lease_until);
CREATE INDEX dude_events_dude ON dude_events(dude_code,source,status);
CREATE INDEX dude_events_session ON dude_events(session_code,status);
CREATE TABLE platform_notifications (
 notification_id TEXT PRIMARY KEY, dedup_key TEXT NOT NULL UNIQUE, record_json TEXT NOT NULL,
 created_at INTEGER NOT NULL
);
CREATE INDEX platform_notifications_created ON platform_notifications(created_at DESC);
CREATE TABLE collaboration_threads (
 session_id TEXT PRIMARY KEY, workspace_code TEXT NOT NULL, user_code TEXT,
 owner_kind TEXT NOT NULL, owner_code TEXT NOT NULL, record_json TEXT NOT NULL,
 updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX collaboration_threads_owner ON collaboration_threads(owner_kind,owner_code,workspace_code);

CREATE TABLE collaboration_thread_feedback(event_key TEXT PRIMARY KEY, session_id TEXT NOT NULL, payload_json TEXT NOT NULL, delivered INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP);
CREATE INDEX collaboration_thread_feedback_pending ON collaboration_thread_feedback(delivered,created_at);
CREATE TABLE dude_memory_batches(batch_code TEXT PRIMARY KEY,user_code TEXT NOT NULL,dude_code TEXT NOT NULL,status TEXT NOT NULL,record_json TEXT NOT NULL,created_at INTEGER NOT NULL);
CREATE UNIQUE INDEX dude_memory_one_active ON dude_memory_batches(user_code,dude_code) WHERE status='Active';
CREATE TABLE dude_memory_cursors(user_code TEXT NOT NULL,dude_code TEXT NOT NULL,source_key TEXT NOT NULL,source_version TEXT NOT NULL,PRIMARY KEY(user_code,dude_code,source_key));
CREATE TABLE dude_skill_candidates(candidate_code TEXT PRIMARY KEY,user_code TEXT NOT NULL,dude_code TEXT NOT NULL,batch_code TEXT NOT NULL,skill_name TEXT NOT NULL,record_json TEXT NOT NULL,created_at INTEGER NOT NULL,UNIQUE(batch_code,skill_name));
CREATE INDEX dude_skill_candidates_user ON dude_skill_candidates(user_code,created_at);
-- Durable business changes and notification delivery receipts.
CREATE TABLE dude_work_changes (
    change_id TEXT PRIMARY KEY,
    dude_code TEXT NOT NULL,
    task_code TEXT NOT NULL,
    change_key TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    record_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(dude_code, task_code, change_key),
    UNIQUE(dude_code, task_code, fingerprint)
);
CREATE INDEX idx_dude_work_changes_task ON dude_work_changes(dude_code, task_code, created_at);

-- tron:shortcut-defaults:start
CREATE TABLE IF NOT EXISTS shell_shortcut_defaults (
    action TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    mod_key INTEGER NOT NULL CHECK(mod_key IN (0,1)),
    alt_key INTEGER NOT NULL CHECK(alt_key IN (0,1)),
    shift_key INTEGER NOT NULL CHECK(shift_key IN (0,1))
);
INSERT OR IGNORE INTO shell_shortcut_defaults(action,key,mod_key,alt_key,shift_key) VALUES
    ('newTopic','1',1,0,1),
    ('newDirectory','2',1,0,1),
    ('newSession','N',1,0,0),
    ('toggleAgentWindow','N',1,0,1),
    ('toggleVoiceInput','T',1,0,0),
    ('openNotifications','M',1,0,1),
    ('closeCurrentTab','W',1,1,0),
    ('previousTab','H',1,1,0),
    ('nextTab','L',1,1,0),
    ('activateTab1','1',1,0,0),
    ('activateTab2','2',1,0,0),
    ('activateTab3','3',1,0,0),
    ('activateTab4','4',1,0,0),
    ('activateTab5','5',1,0,0),
    ('activateTab6','6',1,0,0),
    ('activateTab7','7',1,0,0),
    ('activateTab8','8',1,0,0),
    ('activateTab9','9',1,0,0),
    ('closeAllTabs','X',1,1,0),
    ('closeOtherTabs','O',1,1,0),
    ('toggleLeftSidebar','ArrowLeft',1,1,0),
    ('toggleRightSidebar','ArrowRight',1,1,0),
    ('toggleBottomPanel','ArrowDown',1,1,0);
-- tron:shortcut-defaults:end

-- tron:profiles

CREATE TABLE profile_avatar_cache (
    person_ai_user_id TEXT PRIMARY KEY NOT NULL,
    content_type TEXT NOT NULL,
    image_bytes BLOB NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (person_ai_user_id) REFERENCES profiles(person_ai_user_id) ON DELETE CASCADE
);

CREATE TABLE profiles (
    profile_id TEXT PRIMARY KEY NOT NULL,
    person_ai_user_id TEXT NOT NULL DEFAULT '' UNIQUE,
    person_ai_username TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'PENDING',
    created_at TEXT NOT NULL,
    last_activated_at TEXT
);

CREATE TABLE IF NOT EXISTS profile_preferences (
    person_ai_user_id TEXT PRIMARY KEY NOT NULL,
    time_zone TEXT NOT NULL DEFAULT 'Asia/Shanghai',
    updated_at TEXT NOT NULL,
    FOREIGN KEY (person_ai_user_id) REFERENCES profiles(person_ai_user_id) ON DELETE CASCADE
);

CREATE TABLE profile_account_cache (
    person_ai_user_id TEXT PRIMARY KEY NOT NULL REFERENCES profiles(person_ai_user_id) ON DELETE CASCADE,
    nickname TEXT,
    vip_level INTEGER,
    remaining_points REAL,
    account_updated_at INTEGER,
    checkin_date TEXT,
    today_checked_in INTEGER,
    today_reward_points REAL,
    checkin_updated_at INTEGER
);

CREATE TABLE IF NOT EXISTS tron_schema_initialized (scope TEXT PRIMARY KEY);
INSERT INTO tron_schema_initialized (scope) VALUES ('profiles');
