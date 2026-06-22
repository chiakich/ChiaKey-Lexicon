use crate::config::{Config, LIBCHEWING_SOURCE_ID, OVERLAY_SOURCE_ID, RIME_ESSAY_SOURCE_ID};
use crate::phonetics::{phrase_candidate, qstring_for_bpmf_sequence};
use crate::types::{LibchewingFile, LibchewingWeightMode, SourceRecord};
use anyhow::{Context, Result};
use csv::StringRecord;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub fn libchewing_max_score(paths: &[PathBuf]) -> Result<i64> {
    let mut max_score = 1;
    for path in paths {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .comment(Some(b'#'))
            .from_path(path)
            .with_context(|| format!("read {}", path.display()))?;
        for row in reader.records() {
            let row = row?;
            if let Some(score) = row.get(1).and_then(parse_i64) {
                max_score = max_score.max(score);
            }
        }
    }
    Ok(max_score)
}

pub fn parse_libchewing_csv(
    entry: &LibchewingFile,
    max_score: i64,
    existing_exact_keys: Option<&HashSet<(String, String)>>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let mut seen = 0;
    let mut skipped = 0;
    let mut records = Vec::new();
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .comment(Some(b'#'))
        .from_path(&entry.path)
        .with_context(|| format!("read {}", entry.path.display()))?;

    for row in reader.records() {
        seen += 1;
        let row = row?;
        match parse_libchewing_row(&row, entry, max_score, existing_exact_keys) {
            Some(record) => records.push(record),
            None => skipped += 1,
        }
    }

    Ok((dedupe_records(records), seen, skipped))
}

pub fn parse_rime_essay(
    path: &Path,
    cfg: &Config,
    char_readings: &HashMap<String, String>,
    existing_phrases: &HashSet<String>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut raw_rows: Vec<(String, i64, String)> = Vec::new();
    let mut seen = 0;
    let mut skipped = 0;
    let mut max_score = 1;

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        seen += 1;
        let Some((phrase, score_text)) = line.split_once('\t') else {
            skipped += 1;
            continue;
        };
        let Some(score) = parse_i64(score_text) else {
            skipped += 1;
            continue;
        };
        if score < cfg.rime_essay_min_score
            || !phrase_candidate(phrase, 2, cfg.max_phrase_codepoints)
            || existing_phrases.contains(phrase)
        {
            skipped += 1;
            continue;
        }

        let mut qstring = String::new();
        let mut ok = true;
        for character in phrase.chars() {
            let key = character.to_string();
            match char_readings.get(&key) {
                Some(reading) => qstring.push_str(reading),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            skipped += 1;
            continue;
        }

        max_score = max_score.max(score);
        raw_rows.push((phrase.to_string(), score, qstring));
    }

    let records = raw_rows
        .into_iter()
        .map(|(phrase, score, qstring)| SourceRecord {
            qstring,
            phrase,
            weight: rime_weight(score, max_score),
            source_id: RIME_ESSAY_SOURCE_ID,
            tags: format!("unigram,{RIME_ESSAY_SOURCE_ID},supplemental"),
        })
        .collect::<Vec<_>>();

    Ok((dedupe_records(records), seen, skipped))
}

pub fn parse_overlay(path: &Path, cfg: &Config) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut seen = 0;
    let mut skipped = 0;
    let mut records = Vec::new();

    for (line_number, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        seen += 1;
        let parts = line.splitn(3, '\t').collect::<Vec<_>>();
        if parts.len() < 3 || !phrase_candidate(parts[0], 1, cfg.max_phrase_codepoints) {
            skipped += 1;
            continue;
        }
        let weight: f64 = parts[1].parse().with_context(|| {
            format!(
                "invalid overlay weight {}:{}",
                path.display(),
                line_number + 1
            )
        })?;
        records.push(SourceRecord {
            qstring: String::new(),
            phrase: parts[0].to_string(),
            weight,
            source_id: OVERLAY_SOURCE_ID,
            tags: format!("unigram,{}", parts[2]),
        });
    }

    Ok((records, seen, skipped))
}

pub fn infer_overlay_qstrings(
    records: Vec<SourceRecord>,
    char_readings: &HashMap<String, String>,
) -> (Vec<SourceRecord>, usize) {
    let mut skipped = 0;
    let mut inferred = Vec::new();
    for mut record in records {
        let mut qstring = String::new();
        let mut ok = true;
        for character in record.phrase.chars() {
            let key = character.to_string();
            match char_readings.get(&key) {
                Some(reading) => qstring.push_str(reading),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            record.qstring = qstring;
            inferred.push(record);
        } else {
            skipped += 1;
        }
    }
    (dedupe_records(inferred), skipped)
}

pub fn dedupe_records(records: Vec<SourceRecord>) -> Vec<SourceRecord> {
    let mut map: HashMap<(String, String), SourceRecord> = HashMap::new();
    for record in records {
        let key = (record.qstring.clone(), record.phrase.clone());
        match map.get(&key) {
            Some(existing) if existing.weight >= record.weight => {}
            _ => {
                map.insert(key, record);
            }
        }
    }
    map.into_values().collect()
}

pub fn format_weight(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        let text = format!("{value:.6}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn parse_libchewing_row(
    row: &StringRecord,
    entry: &LibchewingFile,
    max_score: i64,
    existing_exact_keys: Option<&HashSet<(String, String)>>,
) -> Option<SourceRecord> {
    let phrase = row.get(0)?.to_string();
    let score = parse_i64(row.get(1)?)?;
    let reading = row.get(2)?;
    let (qstring, syllable_count) = qstring_for_bpmf_sequence(reading)?;
    if !phrase_candidate(&phrase, entry.min_codepoints, entry.max_codepoints) {
        return None;
    }
    if syllable_count != phrase.chars().count() {
        return None;
    }
    if existing_exact_keys.is_some_and(|keys| keys.contains(&(qstring.clone(), phrase.clone()))) {
        return None;
    }

    let weight = match entry.weight_mode {
        LibchewingWeightMode::Frequency => libchewing_weight(score, max_score),
        LibchewingWeightMode::CharacterFrequency => libchewing_character_weight(score, max_score),
        LibchewingWeightMode::CharacterFallback => -3.2,
    };
    let tags = format!(
        "unigram,{LIBCHEWING_SOURCE_ID},{}",
        entry.kind.trim_start_matches("libchewing-")
    );
    Some(SourceRecord {
        qstring,
        phrase,
        weight,
        source_id: LIBCHEWING_SOURCE_ID,
        tags,
    })
}

fn libchewing_weight(score: i64, max_score: i64) -> f64 {
    if score <= 0 {
        return -2.8;
    }
    let ratio = ((score + 1) as f64).ln() / ((max_score + 1) as f64).ln();
    round6(-0.25 - (2.35 * (1.0 - ratio)))
}

fn libchewing_character_weight(score: i64, max_score: i64) -> f64 {
    if score <= 0 {
        return -3.2;
    }
    let ratio = ((score + 1) as f64).ln() / ((max_score + 1) as f64).ln();
    round6(-0.35 - (2.85 * (1.0 - ratio)))
}

fn rime_weight(score: i64, max_score: i64) -> f64 {
    let ratio = ((score + 1) as f64).ln() / ((max_score + 1) as f64).ln();
    round6(-1.35 - (1.85 * (1.0 - ratio)))
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn parse_i64(value: impl AsRef<str>) -> Option<i64> {
    value.as_ref().trim().parse().ok()
}
