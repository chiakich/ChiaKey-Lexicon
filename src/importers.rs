use crate::config::{
    Config, CHIAKI_SYNTHETIC_SOURCE_ID, CHIAKI_WEB_OVERLAY_SOURCE_ID, LIBCHEWING_SOURCE_ID,
    OVERLAY_SOURCE_ID, RIME_ESSAY_SOURCE_ID,
};
use crate::phonetics::{phrase_candidate, qstring_for_bpmf_sequence};
use crate::types::{
    BigramRecord, LibchewingFile, LibchewingWeightMode, SourceRecord, VariantDemotionRecord,
};
use anyhow::{bail, Context, Result};
use csv::StringRecord;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const LIBCHEWING_PHRASE_SEGMENT_BONUS: f64 = 0.5;
const LIBCHEWING_PHRASE_SEGMENT_BONUS_THRESHOLD: f64 = -0.75;
pub const LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_MIN_PHRASE_WEIGHT: f64 = -1.0;
const LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_MIN_SUPPORT: usize = 3;
const LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_CURRENT_THRESHOLD: f64 = -2.4;
const LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_PENALTY: f64 = 1.0;
const LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_MAX_WEIGHT: f64 = -1.35;
const RIME_OVERLAP_RERANK_MARGIN: f64 = 0.01;
const RIME_OVERLAP_RERANK_MAX_WEIGHT: f64 = -0.5;
const RIME_OVERLAP_RERANK_STRONG_GROUP_THRESHOLD: f64 = -0.75;
const RIME_SPLIT_RERANK_MARGIN: f64 = 0.01;
const RIME_SPLIT_RERANK_MAX_WEIGHT: f64 = RIME_OVERLAP_RERANK_STRONG_GROUP_THRESHOLD;

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
    existing_qstring_weights: &HashMap<String, f64>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut raw_rows: Vec<(String, i64, String, usize)> = Vec::new();
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
        let syllable_count = phrase.chars().count();
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
        raw_rows.push((phrase.to_string(), score, qstring, syllable_count));
    }

    let records = raw_rows
        .into_iter()
        .map(|(phrase, score, qstring, syllable_count)| {
            let base_weight = rime_weight(score, max_score);
            let weight = rime_split_rerank_weight(
                base_weight,
                &qstring,
                syllable_count,
                existing_qstring_weights,
            );
            let tags = if weight > base_weight {
                format!("unigram,{RIME_ESSAY_SOURCE_ID},supplemental,split-rerank")
            } else {
                format!("unigram,{RIME_ESSAY_SOURCE_ID},supplemental")
            };
            SourceRecord {
                qstring,
                phrase,
                weight,
                source_id: RIME_ESSAY_SOURCE_ID,
                tags,
            }
        })
        .collect::<Vec<_>>();

    Ok((dedupe_records(records), seen, skipped))
}

pub fn parse_rime_overlap_reranks(
    path: &Path,
    cfg: &Config,
    existing_records: &[(String, String, f64)],
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rime_scores: HashMap<String, i64> = HashMap::new();
    let mut seen = 0;
    let mut skipped = 0;

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
        {
            skipped += 1;
            continue;
        }
        rime_scores
            .entry(phrase.to_string())
            .and_modify(|existing| *existing = (*existing).max(score))
            .or_insert(score);
    }

    let mut qstring_groups: HashMap<String, Vec<(String, f64, i64)>> = HashMap::new();
    for (qstring, phrase, current_weight) in existing_records {
        let phrase_len = phrase.chars().count();
        if phrase_len < 2
            || phrase_len > cfg.max_phrase_codepoints
            || qstring.chars().count() != phrase_len * 2
        {
            continue;
        }
        if let Some(score) = rime_scores.get(phrase) {
            qstring_groups.entry(qstring.clone()).or_default().push((
                phrase.clone(),
                *current_weight,
                *score,
            ));
        }
    }

    let mut records = Vec::new();
    for (qstring, mut group) in qstring_groups {
        if group.len() < 2 {
            continue;
        }
        if group.iter().any(|(_phrase, current_weight, _score)| {
            *current_weight >= RIME_OVERLAP_RERANK_STRONG_GROUP_THRESHOLD
        }) {
            continue;
        }
        group.sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));

        let mut floor = f64::NEG_INFINITY;
        for (phrase, current_weight, _score) in group {
            let minimum_weight = if floor.is_finite() {
                floor + RIME_OVERLAP_RERANK_MARGIN
            } else {
                current_weight
            };
            let proposed_weight = current_weight.max(minimum_weight);
            let applied_weight = if proposed_weight > current_weight
                && proposed_weight <= RIME_OVERLAP_RERANK_MAX_WEIGHT
            {
                records.push(SourceRecord {
                    qstring: qstring.clone(),
                    phrase,
                    weight: round6(proposed_weight),
                    source_id: RIME_ESSAY_SOURCE_ID,
                    tags: format!("unigram,{RIME_ESSAY_SOURCE_ID},overlap-rerank"),
                });
                proposed_weight
            } else {
                current_weight
            };
            floor = floor.max(applied_weight);
        }
    }

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

pub fn parse_explicit_overlay(
    path: &Path,
    cfg: &Config,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    parse_explicit_records(path, cfg, OVERLAY_SOURCE_ID)
}

pub fn parse_chiaki_web_overlay(
    path: &Path,
    cfg: &Config,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    parse_explicit_records(path, cfg, CHIAKI_WEB_OVERLAY_SOURCE_ID)
}

pub fn parse_chiaki_synthetic_overlay(
    path: &Path,
    cfg: &Config,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    parse_explicit_records(path, cfg, CHIAKI_SYNTHETIC_SOURCE_ID)
}

pub fn parse_bigram_overlay(
    path: &Path,
    cfg: &Config,
) -> Result<(Vec<BigramRecord>, usize, usize)> {
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
        let parts = line.splitn(4, '\t').collect::<Vec<_>>();
        if parts.len() < 4
            || parts[0].is_empty()
            || (parts[1].is_empty() && parts[2].is_empty())
            || (!parts[1].is_empty() && !phrase_candidate(parts[1], 1, cfg.max_phrase_codepoints))
            || (!parts[2].is_empty() && !phrase_candidate(parts[2], 1, cfg.max_phrase_codepoints))
        {
            skipped += 1;
            continue;
        }
        let probability: f64 = parts[3].parse().with_context(|| {
            format!(
                "invalid bigram probability {}:{}",
                path.display(),
                line_number + 1
            )
        })?;
        if !probability.is_finite() {
            bail!(
                "invalid non-finite bigram probability {}:{}",
                path.display(),
                line_number + 1
            );
        }
        records.push(BigramRecord {
            qstring: parts[0].to_string(),
            previous: parts[1].to_string(),
            current: parts[2].to_string(),
            probability,
        });
    }

    Ok((dedupe_bigram_records(records), seen, skipped))
}

fn parse_explicit_records(
    path: &Path,
    cfg: &Config,
    source_id: &'static str,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
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
        let parts = line.splitn(4, '\t').collect::<Vec<_>>();
        if parts.len() < 4
            || parts[0].is_empty()
            || !phrase_candidate(parts[1], 1, cfg.max_phrase_codepoints)
        {
            skipped += 1;
            continue;
        }
        let weight: f64 = parts[2].parse().with_context(|| {
            format!(
                "invalid explicit overlay weight {}:{}",
                path.display(),
                line_number + 1
            )
        })?;
        records.push(SourceRecord {
            qstring: parts[0].to_string(),
            phrase: parts[1].to_string(),
            weight,
            source_id,
            tags: format!("unigram,{}", parts[3]),
        });
    }

    Ok((dedupe_records(records), seen, skipped))
}

pub fn phrase_evidence_character_records(
    evidence: &[(String, String, f64, f64, usize)],
) -> Vec<SourceRecord> {
    let records = evidence
        .iter()
        .filter_map(
            |(qstring, phrase, current_weight, best_phrase_weight, support_count)| {
                if *support_count < LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_MIN_SUPPORT
                    || *current_weight > LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_CURRENT_THRESHOLD
                {
                    return None;
                }

                let proposed_weight = (*best_phrase_weight
                    - LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_PENALTY)
                    .min(LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_MAX_WEIGHT);
                if proposed_weight <= *current_weight {
                    return None;
                }

                Some(SourceRecord {
                    qstring: qstring.clone(),
                    phrase: phrase.clone(),
                    weight: round6(proposed_weight),
                    source_id: LIBCHEWING_SOURCE_ID,
                    tags: format!("unigram,{LIBCHEWING_SOURCE_ID},character-phrase-evidence"),
                })
            },
        )
        .collect::<Vec<_>>();
    dedupe_records(records)
}

pub fn parse_variant_demotions(path: &Path) -> Result<(Vec<VariantDemotionRecord>, usize, usize)> {
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
        if parts.len() < 3 || !phrase_candidate(parts[0], 1, 1) {
            skipped += 1;
            continue;
        }
        let max_weight: f64 = parts[1].parse().with_context(|| {
            format!(
                "invalid variant demotion weight {}:{}",
                path.display(),
                line_number + 1
            )
        })?;
        if !max_weight.is_finite() {
            bail!(
                "invalid non-finite variant demotion weight {}:{}",
                path.display(),
                line_number + 1
            );
        }
        records.push(VariantDemotionRecord {
            phrase: parts[0].to_string(),
            max_weight,
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

pub fn dedupe_bigram_records(records: Vec<BigramRecord>) -> Vec<BigramRecord> {
    let mut map: HashMap<(String, String, String), BigramRecord> = HashMap::new();
    for record in records {
        let key = (
            record.qstring.clone(),
            record.previous.clone(),
            record.current.clone(),
        );
        match map.get(&key) {
            Some(existing) if existing.probability >= record.probability => {}
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
        LibchewingWeightMode::Frequency => libchewing_weight(score, max_score, syllable_count),
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

fn libchewing_weight(score: i64, max_score: i64, syllable_count: usize) -> f64 {
    let base = if score <= 0 {
        -2.8
    } else {
        let ratio = ((score + 1) as f64).ln() / ((max_score + 1) as f64).ln();
        -0.25 - (2.35 * (1.0 - ratio))
    };
    let segment_bonus = if syllable_count > 1 && base < LIBCHEWING_PHRASE_SEGMENT_BONUS_THRESHOLD {
        LIBCHEWING_PHRASE_SEGMENT_BONUS
    } else {
        0.0
    };
    round6(base + segment_bonus)
}

fn libchewing_character_weight(score: i64, max_score: i64) -> f64 {
    if score <= 0 {
        return -3.25;
    }
    let ratio = ((score + 1) as f64).ln() / ((max_score + 1) as f64).ln();
    // Keep character-frequency rows useful for same-reading character order,
    // while giving explicit phrase rows a small edge over character splits.
    round6(-0.40 - (2.85 * (1.0 - ratio)))
}

fn rime_weight(score: i64, max_score: i64) -> f64 {
    let ratio = ((score + 1) as f64).ln() / ((max_score + 1) as f64).ln();
    round6(-1.35 - (1.85 * (1.0 - ratio)))
}

fn rime_split_rerank_weight(
    base_weight: f64,
    qstring: &str,
    syllable_count: usize,
    existing_qstring_weights: &HashMap<String, f64>,
) -> f64 {
    let mut best_split = f64::NEG_INFINITY;
    for split_syllable in 1..syllable_count {
        let split_at = split_syllable * 2;
        if split_at >= qstring.len() {
            continue;
        }
        let (prefix, suffix) = qstring.split_at(split_at);
        let Some(prefix_weight) = existing_qstring_weights.get(prefix) else {
            continue;
        };
        let Some(suffix_weight) = existing_qstring_weights.get(suffix) else {
            continue;
        };
        best_split = best_split.max(prefix_weight + suffix_weight);
    }

    if best_split.is_finite() && best_split + RIME_SPLIT_RERANK_MARGIN > base_weight {
        round6((best_split + RIME_SPLIT_RERANK_MARGIN).min(RIME_SPLIT_RERANK_MAX_WEIGHT))
    } else {
        base_weight
    }
}

fn round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::{
        libchewing_character_weight, libchewing_weight, parse_bigram_overlay,
        parse_explicit_overlay, parse_rime_essay, parse_rime_overlap_reranks,
        parse_variant_demotions, phrase_evidence_character_records,
        LIBCHEWING_PHRASE_SEGMENT_BONUS, LIBCHEWING_PHRASE_SEGMENT_BONUS_THRESHOLD,
        RIME_OVERLAP_RERANK_MARGIN,
    };
    use crate::config::Config;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn parses_explicit_overlay_rows() {
        let path = temp_file(
            "explicit-overlay",
            "# qstring\tphrase\tweight\ttags\nrq\t個\t-2.9\tmanual,neutral-tone\n",
        );
        let cfg = test_config();

        let (records, seen, skipped) = parse_explicit_overlay(&path, &cfg).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "rq");
        assert_eq!(records[0].phrase, "個");
        assert_eq!(records[0].weight, -2.9);
        assert_eq!(records[0].tags, "unigram,manual,neutral-tone");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parses_bigram_overlay_rows() {
        let path = temp_file(
            "bigram-overlay",
            "# qstring\tprevious\tcurrent\tprobability\nrq t4\t個\t人\t-0.1\n",
        );
        let cfg = test_config();

        let (mut records, seen, skipped) = parse_bigram_overlay(&path, &cfg).unwrap();
        records.sort_by(|left, right| left.qstring.cmp(&right.qstring));

        assert_eq!(seen, 1);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "rq t4");
        assert_eq!(records[0].previous, "個");
        assert_eq!(records[0].current, "人");
        assert_eq!(records[0].probability, -0.1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parses_bigram_overlay_boundary_rows() {
        let path = temp_file(
            "bigram-boundary-overlay",
            "# qstring\tprevious\tcurrent\tprobability\n! rq\t\t個\t-0.3\nrq $\t個\t\t-0.4\n! $\t\t\t-0.5\n",
        );
        let cfg = test_config();

        let (mut records, seen, skipped) = parse_bigram_overlay(&path, &cfg).unwrap();
        records.sort_by(|left, right| left.qstring.cmp(&right.qstring));

        assert_eq!(seen, 3);
        assert_eq!(skipped, 1);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].qstring, "! rq");
        assert_eq!(records[0].previous, "");
        assert_eq!(records[0].current, "個");
        assert_eq!(records[0].probability, -0.3);
        assert_eq!(records[1].qstring, "rq $");
        assert_eq!(records[1].previous, "個");
        assert_eq!(records[1].current, "");
        assert_eq!(records[1].probability, -0.4);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn parses_variant_demotion_rows() {
        let path = temp_file(
            "variant-demotions",
            "# phrase\tmax_weight\ttags\n个\t-3.6\topencc-variant-policy,simplified-form\n",
        );

        let (records, seen, skipped) = parse_variant_demotions(&path).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].phrase, "个");
        assert_eq!(records[0].max_weight, -3.6);
        assert_eq!(
            records[0].tags,
            "unigram,opencc-variant-policy,simplified-form"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn calibrates_known_phrases_above_character_splits() {
        let max_score = 327_781;
        let place_name = libchewing_weight(507, max_score, 2);
        let ordinal = libchewing_character_weight(59_239, max_score);
        let name = libchewing_character_weight(73_301, max_score);
        let foundation = libchewing_weight(74, max_score, 2);
        let machine = libchewing_character_weight(30_641, max_score);
        let weight = libchewing_weight(1, max_score, 2);
        let whole = libchewing_character_weight(35_212, max_score);
        let middle = libchewing_character_weight(14_865, max_score);

        assert!(
            place_name > ordinal + name,
            "place-name phrase should outrank the ordinal+name character split"
        );
        assert!(
            foundation > ordinal + machine,
            "foundation phrase should outrank the ordinal+machine character split"
        );
        assert!(
            weight > whole + middle,
            "weight phrase should outrank the whole+middle character split"
        );
    }

    #[test]
    fn applies_phrase_segment_bonus_only_to_multi_syllable_rows() {
        let max_score = 327_781;
        let single = libchewing_weight(507, max_score, 1);
        let phrase = libchewing_weight(507, max_score, 2);

        assert_eq!(phrase, single + LIBCHEWING_PHRASE_SEGMENT_BONUS);
    }

    #[test]
    fn keeps_already_strong_phrases_on_original_scale() {
        let max_score = 327_781;
        let single = libchewing_weight(max_score, max_score, 1);
        let phrase = libchewing_weight(max_score, max_score, 2);

        assert_eq!(phrase, single);
        assert!(phrase > LIBCHEWING_PHRASE_SEGMENT_BONUS_THRESHOLD);
    }

    #[test]
    fn promotes_weak_character_reading_with_strong_phrase_evidence() {
        let evidence = vec![
            (
                "\\_".to_string(),
                "數".to_string(),
                -3.094452,
                -0.409824,
                14,
            ),
            ("\\_".to_string(), "数".to_string(), -3.094452, -3.094452, 0),
            ("m0".to_string(), "書".to_string(), -0.929843, -0.5, 6),
        ];

        let records = phrase_evidence_character_records(&evidence);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "\\_");
        assert_eq!(records[0].phrase, "數");
        assert_eq!(records[0].weight, -1.409824);
        assert_eq!(
            records[0].tags,
            "unigram,libchewing-data,character-phrase-evidence"
        );
    }

    #[test]
    fn reranks_rime_supplemental_phrase_above_existing_split_path() {
        let path = temp_file("rime-split-rerank", "趁現在\t280\n因爲\t474154\n");
        let cfg = test_config();
        let char_readings = HashMap::from([
            ("趁".to_string(), ":j".to_string()),
            ("現".to_string(), "Ei".to_string()),
            ("在".to_string(), "_d".to_string()),
            ("因".to_string(), "Q;".to_string()),
            ("爲".to_string(), "2f".to_string()),
        ]);
        let existing_phrases = HashSet::new();
        let existing_qstring_weights = HashMap::from([
            (":j".to_string(), -1.698951),
            ("Ei_d".to_string(), -0.265419),
        ]);

        let (records, seen, skipped) = parse_rime_essay(
            &path,
            &cfg,
            &char_readings,
            &existing_phrases,
            &existing_qstring_weights,
        )
        .unwrap();
        let take_now = records
            .iter()
            .find(|record| record.phrase == "趁現在")
            .expect("趁現在 should be imported");

        assert_eq!(seen, 2);
        assert_eq!(skipped, 0);
        assert!(take_now.weight > -1.698951 + -0.265419);
        assert_eq!(take_now.weight, -1.95437);
        assert_eq!(
            take_now.tags,
            "unigram,rime-essay,supplemental,split-rerank"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn lets_rime_split_rerank_escape_the_old_supplemental_floor() {
        let path = temp_file("rime-split-rerank-cap", "統計系統\t46\n因爲\t474154\n");
        let cfg = test_config();
        let char_readings = HashMap::from([
            ("統".to_string(), "?]".to_string()),
            ("計".to_string(), "A_".to_string()),
            ("系".to_string(), "C_".to_string()),
            ("因".to_string(), "Q;".to_string()),
            ("爲".to_string(), "2f".to_string()),
        ]);
        let existing_phrases = HashSet::new();
        let existing_qstring_weights = HashMap::from([
            ("?]A_".to_string(), -0.341542),
            ("C_?]".to_string(), -0.465907),
        ]);

        let (records, _seen, _skipped) = parse_rime_essay(
            &path,
            &cfg,
            &char_readings,
            &existing_phrases,
            &existing_qstring_weights,
        )
        .unwrap();
        let statistics_system = records
            .iter()
            .find(|record| record.phrase == "統計系統")
            .expect("統計系統 should be imported");

        assert_eq!(statistics_system.weight, -0.797449);
        assert!(statistics_system.weight > -1.35);
        assert_eq!(
            statistics_system.tags,
            "unigram,rime-essay,supplemental,split-rerank"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn reranks_existing_same_qstring_candidates_with_rime_scores() {
        let path = temp_file("rime-overlap", "賄選\t531\n會選\t662\n選成\t429\n");
        let cfg = test_config();
        let existing = vec![
            ("=fBZ".to_string(), "賄選".to_string(), -0.885961),
            ("=fBZ".to_string(), "會選".to_string(), -1.215681),
            ("BZ=M".to_string(), "選成".to_string(), -1.640198),
        ];

        let (records, seen, skipped) = parse_rime_overlap_reranks(&path, &cfg, &existing).unwrap();

        assert_eq!(seen, 3);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "=fBZ");
        assert_eq!(records[0].phrase, "會選");
        assert_eq!(records[0].weight, -0.885961 + RIME_OVERLAP_RERANK_MARGIN);
        assert_eq!(records[0].tags, "unigram,rime-essay,overlap-rerank");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_same_qstring_groups_with_strong_candidates_on_libchewing_order() {
        let path = temp_file(
            "rime-overlap-strong",
            "市立\t618\n視力\t3937\n示例\t3448\n事例\t1210\n勢利\t752\n勢力\t10221\n",
        );
        let cfg = test_config();
        let existing = vec![
            ("0_=_".to_string(), "市立".to_string(), -0.549893),
            ("0_=_".to_string(), "視力".to_string(), -0.549936),
            ("0_=_".to_string(), "勢利".to_string(), -0.549936),
            ("0_=_".to_string(), "勢力".to_string(), -0.549936),
            ("0_=_".to_string(), "示例".to_string(), -1.306103),
            ("0_=_".to_string(), "事例".to_string(), -1.306103),
        ];

        let (records, seen, skipped) = parse_rime_overlap_reranks(&path, &cfg, &existing).unwrap();

        assert_eq!(seen, 6);
        assert_eq!(skipped, 0);
        assert!(records.is_empty());

        let _ = fs::remove_file(path);
    }

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("chiakey-lexicon-{name}-{}.tsv", std::process::id()));
        fs::write(&path, content).unwrap();
        path
    }

    fn test_config() -> Config {
        Config {
            root: PathBuf::new(),
            boneyard_db: PathBuf::new(),
            release_version: "test".to_string(),
            language_model_version: "test".to_string(),
            minimum_app_version: "test".to_string(),
            generated_at: "2026-06-23T00:00:00Z".to_string(),
            release_base_url: "https://example.invalid".to_string(),
            max_phrase_codepoints: 7,
            rime_essay_min_score: 40,
            dist_dir: PathBuf::new(),
            normalized_path: PathBuf::new(),
            manifest_path: PathBuf::new(),
        }
    }
}

fn parse_i64(value: impl AsRef<str>) -> Option<i64> {
    value.as_ref().trim().parse().ok()
}
