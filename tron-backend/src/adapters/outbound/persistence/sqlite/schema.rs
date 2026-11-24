//! Tron 全新数据库初始化。两个数据库共用 SQL 定义，各自只执行所属区段。
use rusqlite::{Connection, TransactionBehavior, params};

use super::framework_storage::business_transaction;

const INITIAL_SCHEMA: &str = include_str!("../../../../../migrations/0001_init.sql");
const PROFILE_SECTION: &str = "-- tron:profiles\n";
const BUSINESS_MIGRATIONS: &[&str] = &[
    include_str!("../../../../../migrations/0002_post_1_0_2.sql"),
    include_str!("../../../../../migrations/0010_task_execution_records.sql"),
    include_str!("../../../../../migrations/0011_automation_concurrency_mode.sql"),
];
const MIGRATION_SECTION: &str = "-- tron:migration:";

pub(crate) fn initialize_business(connection: &Connection) -> rusqlite::Result<()> {
    let (business, _) = INITIAL_SCHEMA
        .split_once(PROFILE_SECTION)
        .expect("身份登记库 SQL 区段");
    initialize(connection, "business", business)?;
    // Initialize additive module storage from the same schema source. No existing
    // navigation rows or user layouts are rebuilt or migrated.
    let packs = business
        .split_once("-- tron:navigation-packs:start\n")
        .expect("navigation pack schema")
        .1
        .split_once("-- tron:navigation-packs:end")
        .expect("navigation pack schema end")
        .0;
    let tx = business_transaction(connection, TransactionBehavior::Immediate)?;
    tx.execute_batch(packs)?;
    let shortcuts = business
        .split_once("-- tron:shortcut-defaults:start\n")
        .expect("shortcut defaults schema")
        .1
        .split_once("-- tron:shortcut-defaults:end")
        .expect("shortcut defaults schema end")
        .0;
    tx.execute_batch(shortcuts)?;
    tx.commit()
}

pub(crate) fn initialize_profiles(connection: &Connection) -> rusqlite::Result<()> {
    let (_, profiles) = INITIAL_SCHEMA
        .split_once(PROFILE_SECTION)
        .expect("身份登记库 SQL 区段");
    initialize(connection, "profiles", profiles)
}

fn initialize(connection: &Connection, scope: &str, sql: &str) -> rusqlite::Result<()> {
    let transaction = business_transaction(connection, TransactionBehavior::Immediate)?;
    let has_marker: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'tron_schema_initialized')",
        [], |row| row.get(0),
    )?;
    let initialized = has_marker
        && transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM tron_schema_initialized WHERE scope = ?1)",
            params![scope],
            |row| row.get::<_, bool>(0),
        )?;
    if !initialized {
        transaction.execute_batch(sql)?;
    }
    if scope == "business" {
        refresh_builtin_run_count_defaults(&transaction)?;
        apply_business_migrations(&transaction)?;
    }
    transaction.commit()
}

// Replace the old built-in defaults once. A later user-selected limit, including
// 100, must survive subsequent startups; custom agents and other limits stay put.
fn refresh_builtin_run_count_defaults(connection: &Connection) -> rusqlite::Result<()> {
    const MARKER: &str = "builtin-unlimited-run-counts";
    let initialized: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM tron_schema_initialized WHERE scope = ?1)",
        [MARKER],
        |row| row.get(0),
    )?;
    if initialized {
        return Ok(());
    }
    connection.execute_batch(
        "UPDATE platform_internal_agent_configs
         SET config_json = json_set(config_json,
             '$.maxSteps', CASE WHEN json_extract(config_json, '$.maxSteps') = 100
                 THEN 0 ELSE json_extract(config_json, '$.maxSteps') END,
             '$.maxToolCalls', CASE WHEN json_extract(config_json, '$.maxToolCalls') = 100
                 THEN 0 ELSE json_extract(config_json, '$.maxToolCalls') END)
         WHERE agent_code IN ('main-agent', 'explore-agent', 'prd-agent', 'main-assistant', 'memory-assistant')
           AND (json_extract(config_json, '$.maxSteps') = 100
             OR json_extract(config_json, '$.maxToolCalls') = 100);",
    )?;
    connection.execute(
        "INSERT INTO tron_schema_initialized (scope) VALUES (?1)",
        [MARKER],
    )?;
    Ok(())
}

// Apply each business migration once. The marker holds the migration file stem
// so a migration never runs twice, including on databases created from 0001.
fn apply_business_migrations(connection: &Connection) -> rusqlite::Result<()> {
    for migration in BUSINESS_MIGRATIONS {
        for section in migration.split(MIGRATION_SECTION).skip(1) {
            let (name, sql) = section.split_once('\n').expect("migration section name");
            let name = name.trim();
            let applied: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM tron_schema_initialized WHERE scope = ?1)",
                [name],
                |row| row.get(0),
            )?;
            if applied {
                continue;
            }
            connection.execute_batch(sql)?;
            connection.execute(
                "INSERT INTO tron_schema_initialized (scope) VALUES (?1)",
                [name],
            )?;
        }
    }
    Ok(())
}
