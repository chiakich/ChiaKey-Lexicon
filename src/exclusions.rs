use crate::phonetics;
use crate::types::{ImportResult, SourceRecord};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct UnigramExclusion {
    pub qstring: String,
    pub phrase: String,
}

pub fn parse(path: &Path) -> Result<Vec<UnigramExclusion>> {
    let reader =
        BufReader::new(File::open(path).with_context(|| format!("read {}", path.display()))?);
    let mut exclusions = Vec::new();
    let mut seen = HashSet::new();

    for (index, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let columns = line.split('\t').collect::<Vec<_>>();
        if columns.len() != 3 || columns.iter().any(|column| column.trim().is_empty()) {
            bail!(
                "invalid exclusion {}:{}: expected qstring, phrase, reason",
                path.display(),
                index + 1
            );
        }
        let Some(reading) = phonetics::bpmf_for_qstring(columns[0]) else {
            bail!("invalid exclusion qstring {}:{}", path.display(), index + 1);
        };
        if reading.split_whitespace().count() != columns[1].chars().count() {
            bail!(
                "exclusion reading length differs from phrase {}:{}",
                path.display(),
                index + 1
            );
        }
        let exclusion = UnigramExclusion {
            qstring: columns[0].to_string(),
            phrase: columns[1].to_string(),
        };
        if !seen.insert((exclusion.qstring.clone(), exclusion.phrase.clone())) {
            bail!("duplicate exclusion {}:{}", path.display(), index + 1);
        }
        exclusions.push(exclusion);
    }
    Ok(exclusions)
}

pub fn apply(
    conn: &mut Connection,
    exclusions: &[UnigramExclusion],
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    source_path: &str,
    source_sha256: &str,
) -> Result<ImportResult> {
    let tx = conn.transaction()?;
    for exclusion in exclusions {
        let present: i64 = tx.query_row(
            "SELECT COUNT(*) FROM unigrams WHERE qstring = ?1 AND current = ?2",
            params![exclusion.qstring, exclusion.phrase],
            |row| row.get(0),
        )?;
        if present == 0 {
            bail!(
                "stale unigram exclusion: {} ({})",
                exclusion.phrase,
                exclusion.qstring
            );
        }
        tx.execute(
            "DELETE FROM unigrams WHERE qstring = ?1 AND current = ?2",
            params![exclusion.qstring, exclusion.phrase],
        )?;
        tx.execute(
            "DELETE FROM 'Mandarin-bpmf-cin' WHERE key = ?1 AND value = ?2",
            params![exclusion.qstring, exclusion.phrase],
        )?;
        // A bigram can introduce an excluded word even without its unigram row.
        tx.execute(
            "DELETE FROM bigrams
             WHERE (current = ?2 AND substr(qstring, -length(?1)-1) = ' ' || ?1)
                OR (previous = ?2 AND substr(qstring, 1, length(?1)+1) = ?1 || ' ')",
            params![exclusion.qstring, exclusion.phrase],
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
            "unigram-exclusion",
            source_sha256,
            exclusions.len() as i64,
            exclusions.len() as i64,
            0
        ],
    )?;
    tx.commit()?;
    for exclusion in exclusions {
        source_keys.remove(&(exclusion.qstring.clone(), exclusion.phrase.clone()));
    }
    Ok(ImportResult {
        source_path: source_path.to_string(),
        seen: exclusions.len(),
        added: exclusions.len(),
        skipped: 0,
        records: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{apply, UnigramExclusion};
    use rusqlite::{params, Connection};
    use std::collections::HashMap;

    #[test]
    fn removes_only_the_excluded_reading_and_its_bigrams() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE unigrams (qstring TEXT, current TEXT, probability REAL, backoff REAL);
             CREATE TABLE 'Mandarin-bpmf-cin' (key TEXT, value TEXT);
             CREATE TABLE bigrams (qstring TEXT, previous TEXT, current TEXT, probability REAL);
             CREATE TABLE chiaki_db_sources (source TEXT, kind TEXT, sha256 TEXT, seen INTEGER, added INTEGER, skipped INTEGER);",
        )
        .unwrap();
        for (code, phrase) in [("nqvf", "得票"), ("0Cvf", "得票"), ("nqvf", "的票")] {
            conn.execute(
                "INSERT INTO unigrams VALUES(?1, ?2, -1, 0)",
                params![code, phrase],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO 'Mandarin-bpmf-cin' VALUES(?1, ?2)",
                params![code, phrase],
            )
            .unwrap();
        }
        for (code, previous, current) in [
            ("nqvf 0_", "得票", "是"),
            ("0Cvf 0_", "得票", "是"),
            ("0_ nqvf", "是", "得票"),
        ] {
            conn.execute(
                "INSERT INTO bigrams VALUES(?1, ?2, ?3, -1)",
                params![code, previous, current],
            )
            .unwrap();
        }
        let excluded = [UnigramExclusion {
            qstring: "nqvf".into(),
            phrase: "得票".into(),
        }];
        apply(&mut conn, &excluded, &mut HashMap::new(), "test.tsv", "sha").unwrap();

        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM unigrams WHERE current = '得票'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
        let correct: String = conn
            .query_row(
                "SELECT qstring FROM unigrams WHERE current = '得票'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(correct, "0Cvf");
        let wrong_cin: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM 'Mandarin-bpmf-cin' WHERE key = 'nqvf' AND value = '得票'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(wrong_cin, 0);
        let remaining_bigrams: i64 = conn
            .query_row("SELECT COUNT(*) FROM bigrams", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining_bigrams, 1);
    }
}
