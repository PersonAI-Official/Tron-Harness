use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Row, params};

use crate::{
    domain::automation::{
        Automation, AutomationError, AutomationExecutionRecord,
        automation_concurrency_mode_from_storage, automation_concurrency_mode_to_storage,
        automation_execution_status_from_storage, automation_execution_status_to_storage,
        automation_status_from_storage, automation_status_to_storage,
        automation_trigger_source_from_storage,
    },
    shared::id::SnowflakeId,
};

pub(super) fn storage_error(error: rusqlite::Error) -> AutomationError {
    tracing::error!("automation sqlite error: {error}");
    AutomationError::StorageFailure
}

pub(super) fn next_id_in_transaction(
    tx: &crate::adapters::outbound::persistence::sqlite::framework_storage::BusinessTransaction<'_>,
) -> Result<i64, AutomationError> {
    let max_id = tx
        .query_row(
            "SELECT max(id) FROM (SELECT id FROM automations UNION ALL SELECT id FROM automation_execution_records)",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .map_err(storage_error)?
        .unwrap_or(734_600_000_000_000_000);
    Ok(max_id + 1)
}

pub(super) fn update_automation_in_transaction(
    tx: &crate::adapters::outbound::persistence::sqlite::framework_storage::BusinessTransaction<'_>,
    automation: &Automation,
) -> Result<(), AutomationError> {
    tx.execute(
        "UPDATE automations SET workspace_code = ?2, name = ?3, description = ?4, agent_code = ?5, model_code = ?6, prompt_text = ?7, cron_expression = ?8, scheduler_expression = ?9, timezone = ?10, status = ?11, previous_status = ?12, last_execution_status = ?13, last_execution_code = ?14, last_execution_summary = ?15, last_started_at = ?16, last_finished_at = ?17, updated_at = ?18, archived_at = ?19, concurrency_mode = ?20 WHERE automation_code = ?1",
        params![
            automation.automation_code,
            automation.workspace_code,
            automation.name,
            automation.description,
            automation.agent_code,
            automation.model_code,
            automation.prompt_text,
            automation.cron_expression,
            automation.scheduler_expression,
            automation.timezone,
            automation_status_to_storage(automation.status),
            automation.previous_status.map(automation_status_to_storage),
            automation.last_execution_status.map(automation_execution_status_to_storage),
            automation.last_execution_code,
            automation.last_execution_summary,
            automation.last_started_at.as_deref().map(timestamp_to_millis),
            automation.last_finished_at.as_deref().map(timestamp_to_millis),
            timestamp_to_millis(&automation.updated_at),
            automation.archived_at.as_deref().map(timestamp_to_millis),
            automation_concurrency_mode_to_storage(automation.concurrency_mode),
        ],
    ).map_err(storage_error)?;
    Ok(())
}

pub(super) fn row_to_automation(row: &Row<'_>) -> rusqlite::Result<Automation> {
    let status: String = row.get(11)?;
    let previous_status: Option<String> = row.get(12)?;
    let last_execution_status: Option<String> = row.get(13)?;
    Ok(Automation {
        id: SnowflakeId::new(row.get(0)?),
        automation_code: row.get(1)?,
        workspace_code: row.get(2)?,
        name: row.get(3)?,
        description: row.get(4)?,
        agent_code: row.get(5)?,
        model_code: row.get(6)?,
        prompt_text: row.get(7)?,
        cron_expression: row.get(8)?,
        scheduler_expression: row.get(9)?,
        timezone: row.get(10)?,
        status: automation_status_from_storage(&status).map_err(automation_mapping_error)?,
        concurrency_mode: automation_concurrency_mode_from_storage(&row.get::<_, String>(21)?)
            .map_err(automation_mapping_error)?,
        previous_status: previous_status
            .as_deref()
            .map(automation_status_from_storage)
            .transpose()
            .map_err(automation_mapping_error)?,
        last_execution_status: last_execution_status
            .as_deref()
            .map(automation_execution_status_from_storage)
            .transpose()
            .map_err(automation_mapping_error)?,
        last_execution_code: row.get(14)?,
        last_execution_summary: row.get(15)?,
        last_started_at: row.get::<_, Option<i64>>(16)?.map(timestamp_from_millis),
        last_finished_at: row.get::<_, Option<i64>>(17)?.map(timestamp_from_millis),
        created_at: timestamp_from_millis(row.get(18)?),
        updated_at: timestamp_from_millis(row.get(19)?),
        archived_at: row.get::<_, Option<i64>>(20)?.map(timestamp_from_millis),
    })
}

pub(super) fn row_to_execution_record(
    row: &Row<'_>,
) -> rusqlite::Result<AutomationExecutionRecord> {
    let trigger_source: String = row.get(5)?;
    let status: String = row.get(6)?;
    Ok(AutomationExecutionRecord {
        id: SnowflakeId::new(row.get(0)?),
        execution_code: row.get(1)?,
        automation_code: row.get(2)?,
        workspace_code: row.get(3)?,
        session_code: row.get(4)?,
        trigger_source: automation_trigger_source_from_storage(&trigger_source)
            .map_err(automation_mapping_error)?,
        status: automation_execution_status_from_storage(&status)
            .map_err(automation_mapping_error)?,
        summary: row.get(7)?,
        scheduled_at: row.get::<_, Option<i64>>(8)?.map(timestamp_from_millis),
        started_at: timestamp_from_millis(row.get(9)?),
        finished_at: row.get::<_, Option<i64>>(10)?.map(timestamp_from_millis),
        created_at: timestamp_from_millis(row.get(11)?),
    })
}

fn automation_mapping_error(error: AutomationError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
}

pub(super) fn timestamp_to_millis(value: &str) -> i64 {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.timestamp_millis())
        .unwrap_or_else(|_| Utc::now().timestamp_millis())
}

fn timestamp_from_millis(value: i64) -> String {
    Utc.timestamp_millis_opt(value)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}
