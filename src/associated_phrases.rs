use crate::files::sha256_bytes;
use crate::types::KeyValueRecord;
use anyhow::{bail, Result};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};

pub const SOURCE_PATH: &str = "generated/associated_phrases#from-unigrams";
pub const SOURCE_KIND: &str = "derived-associated-phrases";

const MIN_PHRASE_WEIGHT: f64 = -6.0;
const MAX_PHRASE_CODEPOINTS: usize = 4;
const MAX_TAILS_PER_HEAD: usize = 200;
const MIN_TABLE_ROWS: i64 = 1_000;
const BANWORDS: &[&str] = &["媽的"];

pub struct BuildResult {
    pub records: Vec<KeyValueRecord>,
    pub seen: usize,
    pub skipped: usize,
    pub tail_count: usize,
    pub sha256: String,
}

pub fn build_from_unigrams(conn: &Connection) -> Result<BuildResult> {
    let mut stmt = conn.prepare(
        "SELECT current, MAX(probability)
         FROM unigrams
         WHERE current <> ''
         GROUP BY current",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let phrases = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(build_from_phrase_weights(phrases))
}

pub fn validate_runtime_required_data(conn: &Connection) -> Result<()> {
    let row_count: i64 = conn.query_row("SELECT COUNT(*) FROM associated_phrases", [], |row| {
        row.get(0)
    })?;
    if row_count < MIN_TABLE_ROWS {
        bail!("associated_phrases has {row_count} rows, expected at least {MIN_TABLE_ROWS}");
    }

    let index_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master
         WHERE type = 'index' AND name = 'associated_phrases_index'",
        [],
        |row| row.get(0),
    )?;
    if index_count == 0 {
        bail!("associated_phrases_index is missing");
    }

    require_tail(conn, "我", "們")?;
    require_tail(conn, "我", "的")?;
    require_tail(conn, "不", "會")?;
    Ok(())
}

fn build_from_phrase_weights(phrases: Vec<(String, f64)>) -> BuildResult {
    let seen = phrases.len();
    let mut accepted = 0;
    let mut by_head: BTreeMap<String, HashMap<String, f64>> = BTreeMap::new();

    for (phrase, weight) in phrases {
        let Some((head, tail)) = associated_tail(&phrase, weight) else {
            continue;
        };
        accepted += 1;
        by_head
            .entry(head)
            .or_default()
            .entry(tail)
            .and_modify(|current_weight| *current_weight = current_weight.max(weight))
            .or_insert(weight);
    }

    let mut records = Vec::new();
    let mut tail_count = 0;
    for (head, tails) in by_head {
        let mut tails = tails.into_iter().collect::<Vec<_>>();
        tails.sort_by(|(left_tail, left_weight), (right_tail, right_weight)| {
            right_weight
                .total_cmp(left_weight)
                .then_with(|| left_tail.cmp(right_tail))
        });
        tails.truncate(MAX_TAILS_PER_HEAD);

        let data = tails
            .into_iter()
            .map(|(tail, _)| tail)
            .collect::<Vec<_>>()
            .join(",");
        if data.is_empty() {
            continue;
        }
        tail_count += data.split(',').count();
        records.push(KeyValueRecord {
            key: head,
            value: data,
        });
    }

    let sha256 = records_sha256(&records);
    BuildResult {
        records,
        seen,
        skipped: seen.saturating_sub(accepted),
        tail_count,
        sha256,
    }
}

fn associated_tail(phrase: &str, weight: f64) -> Option<(String, String)> {
    if weight <= MIN_PHRASE_WEIGHT || BANWORDS.iter().any(|word| phrase.contains(word)) {
        return None;
    }

    let chars = phrase.chars().collect::<Vec<_>>();
    if chars.len() <= 1 || chars.len() > MAX_PHRASE_CODEPOINTS {
        return None;
    }

    let head = chars[0];
    if !is_runtime_head(head) {
        return None;
    }

    let tail = chars[1..].iter().collect::<String>();
    if tail.contains(',') || tail.chars().any(char::is_control) {
        return None;
    }

    Some((head.to_string(), tail))
}

fn is_runtime_head(character: char) -> bool {
    let codepoint = character as u32;
    (0x3000..=0xff00).contains(&codepoint)
}

fn records_sha256(records: &[KeyValueRecord]) -> String {
    let mut text = String::new();
    for record in records {
        text.push_str(&record.key);
        text.push('\t');
        text.push_str(&record.value);
        text.push('\n');
    }
    sha256_bytes(text.as_bytes())
}

fn require_tail(conn: &Connection, head: &str, tail: &str) -> Result<()> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM associated_phrases
         WHERE headchar = ?1 AND ',' || data || ',' LIKE ?2",
        [head, &format!("%,{tail},%")],
        |row| row.get(0),
    )?;
    if count == 0 {
        bail!("associated_phrases is missing {head} -> {tail}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::build_from_phrase_weights;

    #[test]
    fn builds_headless_tails_sorted_by_weight() {
        let result = build_from_phrase_weights(vec![
            ("我們".to_string(), -0.5),
            ("我的".to_string(), -0.7),
            ("媽的".to_string(), -0.1),
            ("A股".to_string(), -0.2),
            ("我".to_string(), -0.1),
            ("我自己".to_string(), -0.6),
            ("我國".to_string(), -0.3),
        ]);

        let record = result
            .records
            .iter()
            .find(|record| record.key == "我")
            .expect("我 should have associated phrase tails");

        assert_eq!(record.value, "國,們,自己,的");
        assert_eq!(result.tail_count, 4);
        assert_eq!(result.skipped, 3);
    }
}
