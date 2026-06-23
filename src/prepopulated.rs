use crate::config::PREPOPULATED_SERVICE_SOURCE_ID;
use anyhow::{bail, Context, Result};
use chrono::DateTime;
use rusqlite::Connection;
use std::fs;
use std::path::Path;

pub struct ServiceData {
    pub canned_messages: String,
    pub timestamp: i64,
}

pub fn load(canned_messages_path: &Path, generated_at: &str) -> Result<ServiceData> {
    let timestamp = release_timestamp(generated_at)?;
    let canned_messages = fs::read_to_string(canned_messages_path)
        .with_context(|| format!("read {}", canned_messages_path.display()))?;

    Ok(ServiceData {
        canned_messages,
        timestamp,
    })
}

pub fn validate_payload(data: &ServiceData) -> Result<()> {
    if data.canned_messages.len() <= 1000 {
        bail!("canned_messages payload is too small");
    }
    if data.timestamp <= 0 {
        bail!("prepopulated service data timestamp must be greater than 0");
    }
    Ok(())
}

pub fn validate_runtime_required_data(conn: &Connection) -> Result<()> {
    let punctuation_list_unigrams: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unigrams WHERE qstring = '_punctuation_list'",
        [],
        |row| row.get(0),
    )?;
    if punctuation_list_unigrams < 50 {
        bail!("missing runtime punctuation list rows in unigrams");
    }

    let punctuation_list_cin: i64 = conn.query_row(
        "SELECT COUNT(*) FROM 'Mandarin-bpmf-cin' WHERE key = '_punctuation_list'",
        [],
        |row| row.get(0),
    )?;
    if punctuation_list_cin < 50 {
        bail!("missing runtime punctuation list rows in Mandarin-bpmf-cin");
    }

    let punctuation_less_than: String = conn.query_row(
        "SELECT current FROM unigrams WHERE qstring = '_punctuation_<'
         ORDER BY probability DESC, current LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    if punctuation_less_than != "，" {
        bail!("_punctuation_< resolves to {punctuation_less_than}, expected ，");
    }

    let punctuation_standard_less_than: String = conn.query_row(
        "SELECT current FROM unigrams WHERE qstring = '_punctuation_Standard_<'
         ORDER BY probability DESC, current LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    if punctuation_standard_less_than != "，" {
        bail!("_punctuation_Standard_< resolves to {punctuation_standard_less_than}, expected ，");
    }

    let bpmf_cname: i64 = conn.query_row(
        "SELECT COUNT(*) FROM 'Mandarin-bpmf-cin'
         WHERE key = '__property_cname' AND value = '注音'",
        [],
        |row| row.get(0),
    )?;
    if bpmf_cname == 0 {
        bail!("missing Mandarin-bpmf-cin __property_cname metadata");
    }

    require_service_row(
        conn,
        "canned_messages",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'canned_messages' AND LENGTH(value) > 1000",
    )?;
    require_service_row(
        conn,
        "canned_messages_timestamp",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'canned_messages_timestamp' AND CAST(value AS INTEGER) > 0",
    )?;
    forbid_service_row(
        conn,
        "onekey_services",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'onekey_services'",
    )?;
    forbid_service_row(
        conn,
        "onekey_services_timestamp",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'onekey_services_timestamp'",
    )?;

    Ok(())
}

pub fn source_kind() -> &'static str {
    PREPOPULATED_SERVICE_SOURCE_ID
}

fn release_timestamp(generated_at: &str) -> Result<i64> {
    let timestamp = DateTime::parse_from_rfc3339(generated_at)
        .with_context(|| format!("parse generated_at timestamp {generated_at}"))?
        .timestamp();
    if timestamp <= 0 {
        bail!("generated_at timestamp must be greater than 0");
    }
    Ok(timestamp)
}

fn require_service_row(conn: &Connection, key: &str, sql: &str) -> Result<()> {
    let count: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    if count == 0 {
        bail!("missing prepopulated_service_data/{key}");
    }
    Ok(())
}

fn forbid_service_row(conn: &Connection, key: &str, sql: &str) -> Result<()> {
    let count: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    if count > 0 {
        bail!("obsolete prepopulated_service_data/{key} must not be shipped");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_payload, ServiceData};

    #[test]
    fn rejects_empty_service_payloads_and_zero_timestamp() {
        let data = ServiceData {
            canned_messages: String::new(),
            timestamp: 0,
        };

        assert!(validate_payload(&data).is_err());
    }
}
