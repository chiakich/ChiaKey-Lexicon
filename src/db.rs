use crate::config::{Config, BONEYARD_SOURCE_ID};
use crate::files::count_lines;
use crate::importers::{dedupe_records, format_weight};
use crate::prepopulated::ServiceData;
use crate::types::{
    BigramRecord, ImportResult, KeyValueRecord, SourceRecord, VariantDemotionRecord,
};
use anyhow::Result;
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn apply_records(
    conn: &mut Connection,
    records: Vec<SourceRecord>,
    source_path: &str,
    kind: &str,
    source_sha256: &str,
    seen: usize,
    skipped: usize,
    replace_phrases: bool,
) -> Result<ImportResult> {
    let records = dedupe_records(records);
    let tx = conn.transaction()?;

    if replace_phrases && !records.is_empty() {
        tx.execute(
            "CREATE TEMP TABLE chiaki_import_replace_phrases (phrase TEXT PRIMARY KEY)",
            [],
        )?;
        {
            let mut insert =
                tx.prepare("INSERT OR IGNORE INTO chiaki_import_replace_phrases VALUES(?1)")?;
            let mut phrases = records
                .iter()
                .map(|record| record.phrase.clone())
                .collect::<Vec<_>>();
            phrases.sort();
            phrases.dedup();
            for phrase in phrases {
                insert.execute(params![phrase])?;
            }
        }
        tx.execute(
            "DELETE FROM unigrams WHERE current IN (SELECT phrase FROM chiaki_import_replace_phrases)",
            [],
        )?;
        tx.execute(
            "DELETE FROM 'Mandarin-bpmf-cin'
             WHERE value IN (SELECT phrase FROM chiaki_import_replace_phrases)
               AND key NOT LIKE '__property_%'",
            [],
        )?;
    }

    {
        let mut delete = tx.prepare("DELETE FROM unigrams WHERE qstring = ?1 AND current = ?2")?;
        let mut insert_unigram = tx.prepare("INSERT INTO unigrams VALUES(?1, ?2, ?3, 0.0)")?;
        let mut insert_cin = tx.prepare(
            "INSERT INTO 'Mandarin-bpmf-cin'
             SELECT ?1, ?2
             WHERE NOT EXISTS (
               SELECT 1 FROM 'Mandarin-bpmf-cin' WHERE key = ?1 AND value = ?2
             )",
        )?;
        for record in &records {
            delete.execute(params![record.qstring, record.phrase])?;
            insert_unigram.execute(params![record.qstring, record.phrase, record.weight])?;
            insert_cin.execute(params![record.qstring, record.phrase])?;
        }
    }

    if replace_phrases {
        tx.execute("DROP TABLE IF EXISTS chiaki_import_replace_phrases", [])?;
    }
    tx.execute(
        "DELETE FROM chiaki_db_sources WHERE source = ?1",
        params![source_path],
    )?;
    tx.execute(
        "INSERT INTO chiaki_db_sources VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source_path,
            kind,
            source_sha256,
            seen as i64,
            records.len() as i64,
            skipped as i64
        ],
    )?;
    tx.commit()?;

    Ok(ImportResult {
        source_path: source_path.to_string(),
        seen,
        added: records.len(),
        skipped,
        records,
    })
}

pub fn refresh_metadata_counts(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM chiaki_db_metadata
         WHERE key IN (
             'unigram_count',
             'candidate_count',
             'associated_phrase_head_count',
             'associated_phrase_tail_count',
             'bopomofo_correction_count'
         )",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata VALUES('unigram_count', (SELECT COUNT(*) FROM unigrams))",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata VALUES('candidate_count', (SELECT COUNT(*) FROM 'Mandarin-bpmf-cin' WHERE key NOT LIKE '__property_%'))",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata
         VALUES('associated_phrase_head_count', (SELECT COUNT(*) FROM associated_phrases))",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata
         VALUES(
             'associated_phrase_tail_count',
             (
                 SELECT COALESCE(SUM(1 + LENGTH(data) - LENGTH(REPLACE(data, ',', ''))), 0)
                 FROM associated_phrases
                 WHERE data <> ''
             )
         )",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata
         VALUES(
             'bopomofo_correction_count',
             (SELECT COUNT(*) FROM 'BopomofoCorrection-bopomofo-correction-cin')
         )",
        [],
    )?;
    Ok(())
}

pub fn apply_prepopulated_service_data(
    conn: &mut Connection,
    data: &ServiceData,
    source_rows: &[(String, String, String)],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "CREATE TABLE IF NOT EXISTS prepopulated_service_data (key, value)",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS prepopulated_service_data_index
         ON prepopulated_service_data (key)",
        [],
    )?;

    let timestamp = data.timestamp.to_string();
    let rows = [
        ("canned_messages", data.canned_messages.as_str()),
        ("canned_messages_timestamp", timestamp.as_str()),
    ];

    {
        let mut delete = tx.prepare("DELETE FROM prepopulated_service_data WHERE key = ?1")?;
        let mut insert = tx.prepare("INSERT INTO prepopulated_service_data VALUES(?1, ?2)")?;
        for (key, value) in rows {
            delete.execute(params![key])?;
            insert.execute(params![key, value])?;
        }
    }
    tx.execute(
        "DELETE FROM prepopulated_service_data
         WHERE key IN ('onekey_services', 'onekey_services_timestamp')",
        [],
    )?;

    {
        let mut delete = tx.prepare("DELETE FROM chiaki_db_sources WHERE source = ?1")?;
        let mut insert =
            tx.prepare("INSERT INTO chiaki_db_sources VALUES(?1, ?2, ?3, ?4, ?5, ?6)")?;
        for (source, kind, sha256) in source_rows {
            delete.execute(params![source])?;
            insert.execute(params![source, kind, sha256, 1_i64, 2_i64, 0_i64])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub fn apply_key_value_records(
    conn: &mut Connection,
    table_name: &str,
    records: &[KeyValueRecord],
    source_path: &str,
    kind: &str,
    source_sha256: &str,
    seen: usize,
    skipped: usize,
    indexes: &[(&str, &str)],
) -> Result<ImportResult> {
    let table = quote_identifier(table_name);
    let tx = conn.transaction()?;

    tx.execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
    tx.execute(&format!("CREATE TABLE {table} (key, value)"), [])?;

    {
        let mut insert = tx.prepare(&format!("INSERT INTO {table} VALUES(?1, ?2)"))?;
        for record in records {
            insert.execute(params![record.key, record.value])?;
        }
    }

    for (index_name, column) in indexes {
        tx.execute(
            &format!(
                "CREATE INDEX {} ON {table} ({})",
                quote_identifier(index_name),
                quote_identifier(column)
            ),
            [],
        )?;
    }

    tx.execute(
        "DELETE FROM chiaki_db_sources WHERE source = ?1",
        params![source_path],
    )?;
    tx.execute(
        "INSERT INTO chiaki_db_sources VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source_path,
            kind,
            source_sha256,
            seen as i64,
            records.len() as i64,
            skipped as i64
        ],
    )?;

    tx.commit()?;
    Ok(ImportResult {
        source_path: source_path.to_string(),
        seen,
        added: records.len(),
        skipped,
        records: Vec::new(),
    })
}

pub fn apply_associated_phrase_records(
    conn: &mut Connection,
    records: &[KeyValueRecord],
    source_path: &str,
    kind: &str,
    source_sha256: &str,
    seen: usize,
    added: usize,
    skipped: usize,
) -> Result<ImportResult> {
    let tx = conn.transaction()?;

    tx.execute("DROP INDEX IF EXISTS associated_phrases_index", [])?;
    tx.execute("DROP TABLE IF EXISTS associated_phrases", [])?;
    tx.execute("CREATE TABLE associated_phrases (headchar, data)", [])?;

    {
        let mut insert = tx.prepare("INSERT INTO associated_phrases VALUES(?1, ?2)")?;
        for record in records {
            insert.execute(params![record.key, record.value])?;
        }
    }

    tx.execute(
        "CREATE INDEX associated_phrases_index ON associated_phrases (headchar)",
        [],
    )?;
    tx.execute(
        "DELETE FROM chiaki_db_sources WHERE source = ?1",
        params![source_path],
    )?;
    tx.execute(
        "INSERT INTO chiaki_db_sources VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source_path,
            kind,
            source_sha256,
            seen as i64,
            added as i64,
            skipped as i64
        ],
    )?;

    tx.commit()?;
    Ok(ImportResult {
        source_path: source_path.to_string(),
        seen,
        added,
        skipped,
        records: Vec::new(),
    })
}

pub fn apply_bigram_records(
    conn: &mut Connection,
    records: &[BigramRecord],
    source_path: &str,
    kind: &str,
    source_sha256: &str,
    seen: usize,
    skipped: usize,
) -> Result<ImportResult> {
    let tx = conn.transaction()?;

    tx.execute(
        "CREATE TABLE IF NOT EXISTS bigrams (qstring, previous, current, probability)",
        [],
    )?;
    tx.execute(
        "CREATE INDEX IF NOT EXISTS bigrams_index ON bigrams (qstring)",
        [],
    )?;

    {
        let mut delete = tx.prepare(
            "DELETE FROM bigrams
             WHERE qstring = ?1 AND previous = ?2 AND current = ?3",
        )?;
        let mut insert = tx.prepare("INSERT INTO bigrams VALUES(?1, ?2, ?3, ?4)")?;
        for record in records {
            delete.execute(params![record.qstring, record.previous, record.current])?;
            insert.execute(params![
                record.qstring,
                record.previous,
                record.current,
                record.probability
            ])?;
        }
    }

    tx.execute(
        "DELETE FROM chiaki_db_sources WHERE source = ?1",
        params![source_path],
    )?;
    tx.execute(
        "INSERT INTO chiaki_db_sources VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source_path,
            kind,
            source_sha256,
            seen as i64,
            records.len() as i64,
            skipped as i64
        ],
    )?;

    tx.commit()?;
    Ok(ImportResult {
        source_path: source_path.to_string(),
        seen,
        added: records.len(),
        skipped,
        records: Vec::new(),
    })
}

pub fn apply_variant_demotions(
    conn: &mut Connection,
    records: &[VariantDemotionRecord],
    source_path: &str,
    kind: &str,
    source_sha256: &str,
    seen: usize,
    skipped: usize,
    source_id: &'static str,
) -> Result<ImportResult> {
    let tx = conn.transaction()?;
    let mut affected = Vec::new();

    for record in records {
        {
            let mut stmt = tx.prepare(
                "SELECT qstring, current
                 FROM unigrams
                 WHERE current = ?1 AND probability > ?2",
            )?;
            let rows = stmt.query_map(params![record.phrase, record.max_weight], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (qstring, phrase) = row?;
                affected.push(SourceRecord {
                    qstring,
                    phrase,
                    weight: record.max_weight,
                    source_id,
                    tags: record.tags.clone(),
                });
            }
        }
        tx.execute(
            "UPDATE unigrams
             SET probability = ?2
             WHERE current = ?1 AND probability > ?2",
            params![record.phrase, record.max_weight],
        )?;
    }

    tx.execute(
        "DELETE FROM chiaki_db_sources WHERE source = ?1",
        params![source_path],
    )?;
    tx.execute(
        "INSERT INTO chiaki_db_sources VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            source_path,
            kind,
            source_sha256,
            seen as i64,
            affected.len() as i64,
            skipped as i64
        ],
    )?;

    tx.commit()?;
    Ok(ImportResult {
        source_path: source_path.to_string(),
        seen,
        added: affected.len(),
        skipped,
        records: affected,
    })
}

pub fn update_release_metadata_rows(conn: &Connection, cfg: &Config) -> Result<()> {
    conn.execute("DELETE FROM cooked_information WHERE key = 'version'", [])?;
    conn.execute(
        "INSERT INTO cooked_information VALUES('version', ?1)",
        params![cfg.language_model_version],
    )?;
    conn.execute(
        "DELETE FROM chiaki_db_metadata WHERE key IN ('version', 'lexicon_release_version', 'lexicon_release_generator', 'generated_at')",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata VALUES('version', ?1)",
        params![cfg.language_model_version],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata VALUES('lexicon_release_version', ?1)",
        params![cfg.release_version],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata VALUES('lexicon_release_generator', 'cargo run --release -- prepare-release')",
        [],
    )?;
    conn.execute(
        "INSERT INTO chiaki_db_metadata VALUES('generated_at', ?1)",
        params![cfg.generated_at],
    )?;
    Ok(())
}

pub fn write_normalized(
    conn: &Connection,
    path: &Path,
    source_keys: &HashMap<(String, String), SourceRecord>,
) -> Result<()> {
    let mut file = File::create(path)?;
    let mut stmt = conn.prepare(
        "SELECT qstring AS reading, current AS phrase, probability AS weight
         FROM unigrams
         WHERE current <> ''
         ORDER BY qstring, current",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;
    for row in rows {
        let (reading, phrase, weight) = row?;
        if phrase.contains('\t') || phrase.contains('\n') {
            continue;
        }
        let key = (reading.clone(), phrase.clone());
        let (source_id, tags) = match source_keys.get(&key) {
            Some(record) => (record.source_id, record.tags.as_str()),
            None => (BONEYARD_SOURCE_ID, "unigram,keykey-boneyard"),
        };
        writeln!(
            file,
            "{}\t{}\t{}\t{}\t{}",
            reading,
            phrase,
            format_weight(weight),
            source_id,
            tags
        )?;
    }
    Ok(())
}

pub fn db_metadata(conn: &Connection) -> Result<BTreeMap<String, Value>> {
    let mut stmt = conn.prepare("SELECT key, value FROM chiaki_db_metadata ORDER BY key")?;
    let rows = stmt.query_map([], |row| {
        let key = row.get::<_, String>(0)?;
        let value = match row.get_ref(1)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(value) => json!(value),
            ValueRef::Real(value) => json!(value),
            ValueRef::Text(value) => json!(String::from_utf8_lossy(value).to_string()),
            ValueRef::Blob(value) => json!(hex_string(value)),
        };
        Ok((key, value))
    })?;
    let mut map = BTreeMap::new();
    for row in rows {
        let (key, value) = row?;
        map.insert(key, value);
    }
    Ok(map)
}

pub fn db_source_rows(conn: &Connection) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT source, kind, sha256, seen, added, skipped FROM chiaki_db_sources ORDER BY source",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "source": row.get::<_, String>(0)?,
            "kind": row.get::<_, String>(1)?,
            "sha256": row.get::<_, String>(2)?,
            "seen": row.get::<_, i64>(3)?,
            "added": row.get::<_, i64>(4)?,
            "skipped": row.get::<_, i64>(5)?
        }))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn stats_for_source_rows(source_rows: &[Value], prefix_or_path: &str) -> Vec<Value> {
    source_rows
        .iter()
        .filter(|row| {
            row.get("source")
                .and_then(Value::as_str)
                .is_some_and(|source| {
                    source == prefix_or_path || source.starts_with(prefix_or_path)
                })
        })
        .cloned()
        .collect()
}

pub fn db_counts(
    conn: &Connection,
    normalized_path: &Path,
    metadata: &BTreeMap<String, Value>,
) -> Result<Value> {
    let unigrams: i64 = conn.query_row("SELECT COUNT(*) FROM unigrams", [], |row| row.get(0))?;
    let bigrams: i64 = conn.query_row("SELECT COUNT(*) FROM bigrams", [], |row| row.get(0))?;
    let mandarin_rows: i64 =
        conn.query_row("SELECT COUNT(*) FROM 'Mandarin-bpmf-cin'", [], |row| {
            row.get(0)
        })?;
    let associated_phrase_rows: i64 =
        conn.query_row("SELECT COUNT(*) FROM associated_phrases", [], |row| {
            row.get(0)
        })?;
    let bopomofo_correction_rows: i64 = conn.query_row(
        "SELECT COUNT(*) FROM 'BopomofoCorrection-bopomofo-correction-cin'",
        [],
        |row| row.get(0),
    )?;
    let normalized_rows = count_lines(normalized_path)? as i64;
    let candidate_rows = metadata
        .get("candidate_count")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or_default();
    let associated_phrase_tails = metadata
        .get("associated_phrase_tail_count")
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse::<i64>().ok())
        })
        .unwrap_or_default();
    Ok(json!({
        "unigrams": unigrams,
        "bigrams": bigrams,
        "candidate_rows": candidate_rows,
        "mandarin_bpmf_cin_rows": mandarin_rows,
        "bopomofo_correction_rows": bopomofo_correction_rows,
        "associated_phrase_rows": associated_phrase_rows,
        "associated_phrase_tails": associated_phrase_tails,
        "normalized_rows": normalized_rows
    }))
}

fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

pub fn load_existing_exact_keys(conn: &Connection) -> Result<HashSet<(String, String)>> {
    let mut stmt = conn.prepare("SELECT qstring, current FROM unigrams WHERE current <> ''")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(Into::into)
}

pub fn load_existing_phrases(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT current FROM unigrams WHERE current <> ''")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(Into::into)
}

pub fn load_existing_phrase_weights(conn: &Connection) -> Result<Vec<(String, String, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT qstring, current, probability
         FROM unigrams
         WHERE current <> ''",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn load_best_qstring_weights(conn: &Connection) -> Result<HashMap<String, f64>> {
    let mut stmt = conn.prepare(
        "SELECT qstring, MAX(probability)
         FROM unigrams
         WHERE current <> ''
         GROUP BY qstring",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    rows.collect::<std::result::Result<HashMap<_, _>, _>>()
        .map_err(Into::into)
}

// Best unigram probability keyed by the word itself (current), for calibrating
// bigram log-probs relative to the unigram floor the walker compares against.
pub fn load_best_unigram_weights_by_current(conn: &Connection) -> Result<HashMap<String, f64>> {
    let mut stmt = conn.prepare(
        "SELECT current, MAX(probability)
         FROM unigrams
         WHERE current <> ''
         GROUP BY current",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    rows.collect::<std::result::Result<HashMap<_, _>, _>>()
        .map_err(Into::into)
}

pub fn load_character_phrase_evidence(
    conn: &Connection,
    min_phrase_weight: f64,
) -> Result<Vec<(String, String, f64, f64, usize)>> {
    let mut stmt = conn.prepare(
        "SELECT c.qstring,
                c.current,
                c.probability,
                MAX(p.probability),
                COUNT(*)
         FROM unigrams c
         JOIN unigrams p
           ON length(c.current) = 1
          AND length(p.current) > 1
          AND length(p.qstring) = length(p.current) * 2
          AND substr(p.current, 1, 1) = c.current
          AND substr(p.qstring, 1, 2) = c.qstring
         WHERE p.probability >= ?1
         GROUP BY c.qstring, c.current, c.probability",
    )?;
    let rows = stmt.query_map(params![min_phrase_weight], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, i64>(4)? as usize,
        ))
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub fn load_primary_character_readings(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn.prepare(
        "SELECT qstring, current, probability
         FROM unigrams
         WHERE current <> ''
         ORDER BY current, probability DESC, qstring",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut readings = HashMap::new();
    for row in rows {
        let (qstring, phrase) = row?;
        if phrase.chars().count() == 1
            && !(qstring.starts_with("_punctuation_") || qstring.starts_with("_ctrl_"))
        {
            readings.entry(phrase).or_insert(qstring);
        }
    }
    Ok(readings)
}
