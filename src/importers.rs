use crate::config::{
    Config, CHIAKEY_AUTO_HOTWORDS_SOURCE_ID, CHIAKI_SYNTHETIC_SOURCE_ID,
    CHIAKI_WEB_OVERLAY_SOURCE_ID, LIBCHEWING_SOURCE_ID, OPENCC_VARIANT_SOURCE_ID,
    OVERLAY_SOURCE_ID, RIME_ESSAY_SOURCE_ID,
};
use crate::opencc;
use crate::phonetics::{phrase_candidate, qstring_for_bpmf_sequence};
use crate::types::{
    BigramRecord, ConversionRule, LibchewingFile, LibchewingWeightMode, SourceRecord,
    VariantDemotionRecord,
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
const RIME_OVERLAP_RERANK_MAX_BOOST: f64 = 0.35;
const RIME_OVERLAP_RERANK_STRONG_GROUP_THRESHOLD: f64 = -0.75;
const RIME_SPLIT_RERANK_MARGIN: f64 = 0.01;
const RIME_SPLIT_RERANK_MAX_WEIGHT: f64 = RIME_OVERLAP_RERANK_STRONG_GROUP_THRESHOLD;
const RIME_EXISTING_RERANK_MAX_SPLIT_BOOST: f64 = 0.05;
const SINGLE_CHAR_HOMOPHONE_RERANK_MARGIN: f64 = 0.01;
const SINGLE_CHAR_HOMOPHONE_RERANK_MAX_WEIGHT_GAP: f64 = 0.25;
const OPENCC_VARIANT_DEMOTION_MARGIN: f64 = 0.01;

#[derive(Clone, Copy)]
struct RimeScore {
    score: i64,
    converted: bool,
}

pub struct RimeNormalization<'a> {
    conversion_rules: &'a [ConversionRule],
    opencc: Option<OpenccNormalization<'a>>,
}

struct OpenccNormalization<'a> {
    binary: &'a Path,
    config: &'a Path,
}

pub struct NormalizedRimeEssay {
    entries: Vec<RimeEssayEntry>,
    seen: usize,
    skipped: usize,
}

struct RimeEssayEntry {
    phrase: String,
    score: i64,
    conversion_tags: Vec<String>,
}

impl<'a> RimeNormalization<'a> {
    #[cfg(test)]
    pub fn without_opencc(conversion_rules: &'a [ConversionRule]) -> Self {
        Self {
            conversion_rules,
            opencc: None,
        }
    }

    pub fn with_opencc(
        conversion_rules: &'a [ConversionRule],
        binary: &'a Path,
        config: &'a Path,
    ) -> Self {
        Self {
            conversion_rules,
            opencc: Some(OpenccNormalization { binary, config }),
        }
    }
}

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

#[cfg(test)]
pub fn parse_rime_essay(
    path: &Path,
    cfg: &Config,
    char_readings: &HashMap<String, String>,
    existing_phrases: &HashSet<String>,
    existing_qstring_weights: &HashMap<String, f64>,
    normalization: &RimeNormalization<'_>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let normalized = read_normalized_rime_essay(path, normalization)?;
    parse_normalized_rime_essay(
        &normalized,
        cfg,
        char_readings,
        existing_phrases,
        existing_qstring_weights,
    )
}

pub fn parse_normalized_rime_essay(
    normalized: &NormalizedRimeEssay,
    cfg: &Config,
    char_readings: &HashMap<String, String>,
    existing_phrases: &HashSet<String>,
    existing_qstring_weights: &HashMap<String, f64>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let mut raw_rows: Vec<(String, i64, String, usize, Vec<String>)> = Vec::new();
    let seen = normalized.seen;
    let mut skipped = normalized.skipped;
    let mut max_score = 1;

    for entry in &normalized.entries {
        let phrase = entry.phrase.clone();
        let score = entry.score;
        if score < cfg.rime_essay_min_score
            || !phrase_candidate(&phrase, 2, cfg.max_phrase_codepoints)
            || existing_phrases.contains(&phrase)
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
        raw_rows.push((
            phrase,
            score,
            qstring,
            syllable_count,
            entry.conversion_tags.clone(),
        ));
    }

    let records = raw_rows
        .into_iter()
        .map(
            |(phrase, score, qstring, syllable_count, conversion_tags)| {
                let base_weight = rime_weight(score, max_score);
                let weight = rime_split_rerank_weight(
                    base_weight,
                    &qstring,
                    syllable_count,
                    existing_qstring_weights,
                );
                let split_tag = if weight > base_weight {
                    ",split-rerank"
                } else {
                    ""
                };
                let conversion_tag = if conversion_tags.is_empty() {
                    String::new()
                } else {
                    format!(",conversion-fix,{}", conversion_tags.join(","))
                };
                let tags = format!(
                    "unigram,{RIME_ESSAY_SOURCE_ID},supplemental{split_tag}{conversion_tag}"
                );
                SourceRecord {
                    qstring,
                    phrase,
                    weight,
                    source_id: RIME_ESSAY_SOURCE_ID,
                    tags,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok((dedupe_records(records), seen, skipped))
}

// Single-character homophones share a reading group but libchewing's per-character
// frequency is syllable-flattened (all ㄐㄧㄣˋ chars ~17804), so the most common
// character often is not the top candidate. Re-rank within each reading group using
// rime-essay single-char frequency, promoting the essay winner to the group top when
// its frequency advantage over the current top clears min_ratio. Raise-only.
pub fn parse_single_char_homophone_reranks(
    path: &Path,
    existing_records: &[(String, String, f64)],
    min_ratio: f64,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut essay_freq: HashMap<String, i64> = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((phrase, score_text)) = line.split_once('\t') else {
            continue;
        };
        if phrase.chars().count() != 1 {
            continue;
        }
        if let Some(score) = parse_i64(score_text) {
            essay_freq.insert(phrase.to_string(), score);
        }
    }

    let mut groups: HashMap<String, HashMap<String, f64>> = HashMap::new();
    for (qstring, phrase, weight) in existing_records {
        if phrase.chars().count() != 1 || qstring.chars().count() != 2 {
            continue;
        }
        groups
            .entry(qstring.clone())
            .or_default()
            .entry(phrase.clone())
            .and_modify(|best| {
                if *weight > *best {
                    *best = *weight;
                }
            })
            .or_insert(*weight);
    }

    let mut seen = 0;
    let mut skipped = 0;
    let mut records = Vec::new();
    for (qstring, chars) in &groups {
        if chars.len() < 2 {
            continue;
        }
        seen += 1;
        let Some((winner, winner_freq)) = chars
            .keys()
            .filter_map(|character| essay_freq.get(character).map(|freq| (character, *freq)))
            .max_by_key(|(_, freq)| *freq)
        else {
            skipped += 1;
            continue;
        };
        let winner_weight = chars.get(winner).copied().unwrap_or(f64::NEG_INFINITY);
        let strongest_competitor = chars
            .iter()
            .filter(|(character, _weight)| *character != winner)
            .max_by(|left, right| {
                left.1
                    .partial_cmp(right.1)
                    .unwrap()
                    .then_with(|| left.0.cmp(right.0))
            });
        let Some((competitor, competitor_weight)) = strongest_competitor else {
            skipped += 1;
            continue;
        };
        if winner_weight > *competitor_weight {
            skipped += 1;
            continue;
        };
        if *competitor_weight - winner_weight > SINGLE_CHAR_HOMOPHONE_RERANK_MAX_WEIGHT_GAP {
            skipped += 1;
            continue;
        }
        let Some(competitor_freq) = essay_freq.get(competitor).copied() else {
            skipped += 1;
            continue;
        };
        if (winner_freq as f64) < min_ratio * (competitor_freq as f64) {
            skipped += 1;
            continue;
        }
        records.push(SourceRecord {
            qstring: qstring.clone(),
            phrase: winner.clone(),
            weight: round6(competitor_weight + SINGLE_CHAR_HOMOPHONE_RERANK_MARGIN),
            source_id: RIME_ESSAY_SOURCE_ID,
            tags: format!("unigram,{RIME_ESSAY_SOURCE_ID},homophone-rerank"),
        });
    }

    Ok((dedupe_records(records), seen, skipped))
}

#[cfg(test)]
pub fn parse_rime_overlap_reranks(
    path: &Path,
    cfg: &Config,
    existing_records: &[(String, String, f64)],
    normalization: &RimeNormalization<'_>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let normalized = read_normalized_rime_essay(path, normalization)?;
    parse_normalized_rime_overlap_reranks(&normalized, cfg, existing_records)
}

pub fn parse_normalized_rime_overlap_reranks(
    normalized: &NormalizedRimeEssay,
    cfg: &Config,
    existing_records: &[(String, String, f64)],
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let mut rime_scores: HashMap<String, RimeScore> = HashMap::new();
    let seen = normalized.seen;
    let mut skipped = normalized.skipped;

    for entry in &normalized.entries {
        let phrase = entry.phrase.clone();
        let score = entry.score;
        if score < cfg.rime_essay_min_score
            || !phrase_candidate(&phrase, 2, cfg.max_phrase_codepoints)
        {
            skipped += 1;
            continue;
        }
        rime_scores
            .entry(phrase)
            .and_modify(|existing| {
                if score > existing.score {
                    *existing = RimeScore {
                        score,
                        converted: !entry.conversion_tags.is_empty(),
                    };
                } else if score == existing.score {
                    existing.converted |= !entry.conversion_tags.is_empty();
                }
            })
            .or_insert(RimeScore {
                score,
                converted: !entry.conversion_tags.is_empty(),
            });
    }

    let mut qstring_groups: HashMap<String, Vec<(String, f64, RimeScore)>> = HashMap::new();
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
        group.sort_by(|left, right| {
            left.2
                .score
                .cmp(&right.2.score)
                .then_with(|| left.0.cmp(&right.0))
        });

        let mut floor = f64::NEG_INFINITY;
        for (phrase, current_weight, score) in group {
            let minimum_weight = if floor.is_finite() {
                floor + RIME_OVERLAP_RERANK_MARGIN
            } else {
                current_weight
            };
            let proposed_weight = current_weight
                .max(minimum_weight.min(current_weight + RIME_OVERLAP_RERANK_MAX_BOOST));
            let applied_weight = if proposed_weight > current_weight
                && proposed_weight <= RIME_OVERLAP_RERANK_MAX_WEIGHT
            {
                records.push(SourceRecord {
                    qstring: qstring.clone(),
                    phrase,
                    weight: round6(proposed_weight),
                    source_id: RIME_ESSAY_SOURCE_ID,
                    tags: rime_rerank_tags("overlap-rerank", score.converted),
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

#[cfg(test)]
pub fn parse_rime_existing_phrase_reranks(
    path: &Path,
    cfg: &Config,
    existing_records: &[(String, String, f64)],
    existing_qstring_weights: &HashMap<String, f64>,
    normalization: &RimeNormalization<'_>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let normalized = read_normalized_rime_essay(path, normalization)?;
    parse_normalized_rime_existing_phrase_reranks(
        &normalized,
        cfg,
        existing_records,
        existing_qstring_weights,
    )
}

pub fn parse_normalized_rime_existing_phrase_reranks(
    normalized: &NormalizedRimeEssay,
    cfg: &Config,
    existing_records: &[(String, String, f64)],
    existing_qstring_weights: &HashMap<String, f64>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let mut rime_scores: HashMap<String, RimeScore> = HashMap::new();
    let seen = normalized.seen;
    let mut skipped = normalized.skipped;
    let mut max_score = 1;

    for entry in &normalized.entries {
        let phrase = entry.phrase.clone();
        let score = entry.score;
        if score < cfg.rime_essay_min_score
            || !phrase_candidate(&phrase, 2, cfg.max_phrase_codepoints)
        {
            skipped += 1;
            continue;
        }
        max_score = max_score.max(score);
        rime_scores
            .entry(phrase)
            .and_modify(|existing| {
                if score > existing.score {
                    *existing = RimeScore {
                        score,
                        converted: !entry.conversion_tags.is_empty(),
                    };
                } else if score == existing.score {
                    existing.converted |= !entry.conversion_tags.is_empty();
                }
            })
            .or_insert(RimeScore {
                score,
                converted: !entry.conversion_tags.is_empty(),
            });
    }

    let mut records = Vec::new();
    for (qstring, phrase, current_weight) in existing_records {
        let phrase_len = phrase.chars().count();
        if phrase_len < 2
            || phrase_len > cfg.max_phrase_codepoints
            || qstring.chars().count() != phrase_len * 2
        {
            continue;
        }
        let Some(score) = rime_scores.get(phrase) else {
            continue;
        };
        let base_weight = rime_weight(score.score, max_score);
        let split_weight =
            rime_split_rerank_weight(base_weight, qstring, phrase_len, existing_qstring_weights);
        let proposed_weight =
            split_weight.min(round6(base_weight + RIME_EXISTING_RERANK_MAX_SPLIT_BOOST));
        if proposed_weight > *current_weight {
            records.push(SourceRecord {
                qstring: qstring.clone(),
                phrase: phrase.clone(),
                weight: proposed_weight,
                source_id: RIME_ESSAY_SOURCE_ID,
                tags: rime_rerank_tags("existing-rerank", score.converted),
            });
        }
    }

    Ok((dedupe_records(records), seen, skipped))
}

pub fn parse_conversion_rules(path: &Path) -> Result<(Vec<ConversionRule>, usize, usize)> {
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
        if parts.len() < 3
            || !phrase_candidate(parts[0], 1, usize::MAX)
            || !phrase_candidate(parts[1], 1, usize::MAX)
            || parts[0].chars().count() != parts[1].chars().count()
        {
            skipped += 1;
            continue;
        }
        if parts[2].trim().is_empty() {
            bail!(
                "missing conversion rule tags {}:{}",
                path.display(),
                line_number + 1
            );
        }
        records.push(ConversionRule {
            from: parts[0].to_string(),
            to: parts[1].to_string(),
            tags: parts[2].to_string(),
        });
    }

    Ok((records, seen, skipped))
}

pub fn parse_overlay(path: &Path, cfg: &Config) -> Result<(Vec<SourceRecord>, usize, usize)> {
    parse_overlay_records(path, cfg, OVERLAY_SOURCE_ID)
}

pub fn parse_auto_hotwords_overlay(
    path: &Path,
    cfg: &Config,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    parse_overlay_records(path, cfg, CHIAKEY_AUTO_HOTWORDS_SOURCE_ID)
}

fn parse_overlay_records(
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
            source_id,
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

// Log-prob ceiling for calibrated bigrams; keeps a boosted edge from exceeding ~prob 1.
const BIGRAM_PROB_CEILING: f64 = -0.05;
const BIGRAM_JOINED_PHRASE_MARGIN: f64 = 0.01;
const BIGRAM_JOINED_PHRASE_MAX_WEIGHT: f64 = RIME_SPLIT_RERANK_MAX_WEIGHT;
const BIGRAM_JOINED_PHRASE_MAX_PREVIOUS_GAP: f64 = SINGLE_CHAR_HOMOPHONE_RERANK_MAX_WEIGHT_GAP;

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

// Re-anchor a source's bigram log-probs to the unigram the walker compares against:
//   stored = min( unigram(current) + boost + (raw - raw_max_of_source), ceiling )
// boost is how much the source's strongest collocation beats its unigram; the
// (raw - raw_max) term preserves the source's own confidence ranking, so weaker
// pairs fall below the unigram and stay inert. boost == 0 is raw passthrough.
pub fn calibrate_bigram_boost(
    mut records: Vec<BigramRecord>,
    boost: f64,
    unigram_by_current: &HashMap<String, f64>,
) -> Vec<BigramRecord> {
    if boost == 0.0 || records.is_empty() {
        return records;
    }
    let raw_max = records
        .iter()
        .map(|record| record.probability)
        .fold(f64::NEG_INFINITY, f64::max);
    for record in &mut records {
        // No unigram for current (e.g. boundary bigrams): leave raw, it is the only
        // candidate at that node anyway.
        if let Some(unigram) = unigram_by_current.get(&record.current) {
            record.probability =
                (unigram + boost + (record.probability - raw_max)).min(BIGRAM_PROB_CEILING);
        }
    }
    records
}

pub fn joined_phrase_records_from_bigrams(
    records: &[BigramRecord],
    existing_phrases: &HashSet<String>,
    qstring_weights: &HashMap<String, f64>,
    phrase_weights: &HashMap<String, f64>,
    max_phrase_codepoints: usize,
    source_id: &'static str,
) -> Vec<SourceRecord> {
    let mut joined_records = Vec::new();

    for record in records {
        if record.previous.is_empty() || record.current.is_empty() {
            continue;
        }
        let Some((previous_qstring, current_qstring)) = record.qstring.split_once(' ') else {
            continue;
        };
        if record.previous.chars().count() != 1 || record.current.chars().count() < 2 {
            continue;
        }
        let Some(previous_weight) = phrase_weights.get(&record.previous) else {
            continue;
        };
        let Some(previous_qstring_weight) = qstring_weights.get(previous_qstring) else {
            continue;
        };
        let previous_gap = previous_qstring_weight - previous_weight;
        if previous_gap <= 0.0 || previous_gap > BIGRAM_JOINED_PHRASE_MAX_PREVIOUS_GAP {
            continue;
        }

        let phrase = format!("{}{}", record.previous, record.current);
        if existing_phrases.contains(&phrase)
            || !phrase_candidate(&phrase, 2, max_phrase_codepoints)
        {
            continue;
        }

        let qstring = format!("{previous_qstring}{current_qstring}");
        if qstring.chars().count() != phrase.chars().count() * 2 {
            continue;
        }

        let Some(current_weight) = qstring_weights.get(current_qstring) else {
            continue;
        };

        let weight = round6(
            (previous_qstring_weight + current_weight + BIGRAM_JOINED_PHRASE_MARGIN)
                .min(BIGRAM_JOINED_PHRASE_MAX_WEIGHT),
        );
        joined_records.push(SourceRecord {
            qstring,
            phrase,
            weight,
            source_id,
            tags: format!("unigram,{source_id},bigram-joined-phrase"),
        });
    }

    dedupe_records(joined_records)
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

#[cfg(test)]
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

pub fn generate_opencc_variant_demotions(
    unigram_rows: &[(String, String, f64)],
    opencc_binary: &Path,
    opencc_config: &Path,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let mut phrases = unigram_rows
        .iter()
        .map(|(_qstring, phrase, _weight)| phrase.clone())
        .collect::<Vec<_>>();
    phrases.sort();
    phrases.dedup();

    let converted = opencc::convert_lines(opencc_binary, opencc_config, &phrases)?;
    let converted_by_phrase = phrases
        .into_iter()
        .zip(converted)
        .filter(|(phrase, converted)| phrase != converted)
        .collect::<HashMap<_, _>>();

    let mut best_weight_by_key: HashMap<(String, String), f64> = HashMap::new();
    for (qstring, phrase, weight) in unigram_rows {
        best_weight_by_key
            .entry((qstring.clone(), phrase.clone()))
            .and_modify(|existing| *existing = existing.max(*weight))
            .or_insert(*weight);
    }

    let mut records_by_key: HashMap<(String, String), SourceRecord> = HashMap::new();
    let mut skipped = 0;
    for ((qstring, phrase), weight) in &best_weight_by_key {
        let Some(counterpart) = converted_by_phrase.get(phrase) else {
            skipped += 1;
            continue;
        };
        let Some(counterpart_weight) =
            best_weight_by_key.get(&(qstring.clone(), counterpart.clone()))
        else {
            skipped += 1;
            continue;
        };
        let max_weight = round6(counterpart_weight - OPENCC_VARIANT_DEMOTION_MARGIN);
        if *weight <= max_weight {
            skipped += 1;
            continue;
        }
        records_by_key.insert(
            (qstring.clone(), phrase.clone()),
            SourceRecord {
                qstring: qstring.clone(),
                phrase: phrase.clone(),
                weight: max_weight,
                source_id: OPENCC_VARIANT_SOURCE_ID,
                tags: format!(
                    "unigram,{OPENCC_VARIANT_SOURCE_ID},opencc-t2tw-counterpart,traditional-preference,{counterpart}"
                ),
            },
        );
    }

    let mut records = records_by_key.into_values().collect::<Vec<_>>();
    records.sort_by(|left, right| {
        left.qstring
            .cmp(&right.qstring)
            .then_with(|| left.phrase.cmp(&right.phrase))
    });
    Ok((records, best_weight_by_key.len(), skipped))
}

// Same shape as parse_variant_demotions but for multi-character fragment caps
// (variant demotions are single-character only). Reuses VariantDemotionRecord
// and db::apply_variant_demotions for the phrase-level weight cap.
pub fn parse_fragment_demotions(path: &Path) -> Result<(Vec<VariantDemotionRecord>, usize, usize)> {
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
        if parts.len() < 3 || !phrase_candidate(parts[0], 2, 8) {
            skipped += 1;
            continue;
        }
        let max_weight: f64 = parts[1].parse().with_context(|| {
            format!(
                "invalid fragment demotion weight {}:{}",
                path.display(),
                line_number + 1
            )
        })?;
        if !max_weight.is_finite() {
            bail!(
                "invalid non-finite fragment demotion weight {}:{}",
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

pub fn read_normalized_rime_essay(
    path: &Path,
    normalization: &RimeNormalization<'_>,
) -> Result<NormalizedRimeEssay> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
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
        rows.push((phrase.to_string(), score));
    }

    let phrases = rows
        .iter()
        .map(|(phrase, _score)| phrase.clone())
        .collect::<Vec<_>>();
    let opencc_phrases = match &normalization.opencc {
        Some(opencc_config) => {
            opencc::convert_lines(opencc_config.binary, opencc_config.config, &phrases)?
        }
        None => phrases.clone(),
    };

    let entries = rows
        .into_iter()
        .zip(opencc_phrases)
        .map(|((original_phrase, score), opencc_phrase)| {
            let mut tags = Vec::new();
            if original_phrase != opencc_phrase {
                tags.push("opencc-t2tw,modern-zh-tw,variant-normalization".to_string());
            }
            let (phrase, mut override_tags) =
                apply_conversion_rules(&opencc_phrase, normalization.conversion_rules);
            tags.append(&mut override_tags);
            RimeEssayEntry {
                phrase,
                score,
                conversion_tags: tags,
            }
        })
        .collect();

    Ok(NormalizedRimeEssay {
        entries,
        seen,
        skipped,
    })
}

fn apply_conversion_rules(
    phrase: &str,
    conversion_rules: &[ConversionRule],
) -> (String, Vec<String>) {
    let mut converted = phrase.to_string();
    let mut tags = Vec::new();
    for rule in conversion_rules {
        if converted.contains(&rule.from) {
            converted = converted.replace(&rule.from, &rule.to);
            tags.push(rule.tags.clone());
        }
    }
    (converted, tags)
}

fn rime_rerank_tags(kind: &str, converted: bool) -> String {
    if converted {
        format!("unigram,{RIME_ESSAY_SOURCE_ID},{kind},conversion-fix")
    } else {
        format!("unigram,{RIME_ESSAY_SOURCE_ID},{kind}")
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
        calibrate_bigram_boost, joined_phrase_records_from_bigrams, libchewing_character_weight,
        libchewing_weight, parse_bigram_overlay, parse_conversion_rules, parse_explicit_overlay,
        parse_fragment_demotions, parse_rime_essay, parse_rime_existing_phrase_reranks,
        parse_rime_overlap_reranks, parse_single_char_homophone_reranks, parse_variant_demotions,
        phrase_evidence_character_records, round6, RimeNormalization,
        LIBCHEWING_PHRASE_SEGMENT_BONUS, LIBCHEWING_PHRASE_SEGMENT_BONUS_THRESHOLD,
        RIME_OVERLAP_RERANK_MARGIN, SINGLE_CHAR_HOMOPHONE_RERANK_MARGIN,
    };
    use crate::config::{Config, CHIAKI_WEB_OVERLAY_SOURCE_ID};
    use crate::types::ConversionRule;
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
    fn calibrate_bigram_boost_anchors_to_unigram_and_preserves_ranking() {
        use crate::types::BigramRecord;
        let bigram = |current: &str, probability: f64| BigramRecord {
            qstring: "x x".to_string(),
            previous: "前".to_string(),
            current: current.to_string(),
            probability,
        };
        // raw_max = -1.0 (the 強 row). unigram(強)=-1.5, unigram(弱)=-1.2.
        let records = vec![bigram("強", -1.0), bigram("弱", -3.0), bigram("無", -1.0)];
        let mut unigrams = HashMap::new();
        unigrams.insert("強".to_string(), -1.5);
        unigrams.insert("弱".to_string(), -1.2);
        // 無 deliberately has no unigram.

        let out = calibrate_bigram_boost(records.clone(), 1.0, &unigrams);
        let by = |c: &str| out.iter().find(|r| r.current == c).unwrap().probability;
        // 強: unigram(-1.5) + boost(1.0) + (raw-raw_max = 0) = -0.5
        assert!((by("強") - (-0.5)).abs() < 1e-9);
        // 弱: unigram(-1.2) + 1.0 + (-3.0 - -1.0 = -2.0) = -2.2 (stays below its unigram -> inert)
        assert!((by("弱") - (-2.2)).abs() < 1e-9);
        assert!(
            by("弱") < -1.2,
            "weaker collocation must fall below its unigram"
        );
        // 無: no unigram -> left as raw
        assert!((by("無") - (-1.0)).abs() < 1e-9);

        // boost == 0 is raw passthrough.
        let passthrough = calibrate_bigram_boost(records, 0.0, &unigrams);
        assert_eq!(
            passthrough
                .iter()
                .find(|r| r.current == "強")
                .unwrap()
                .probability,
            -1.0
        );
    }

    #[test]
    fn infers_joined_phrase_unigrams_from_bigrams_above_best_split() {
        use crate::types::BigramRecord;

        let records = vec![BigramRecord {
            qstring: "p= ;:^l".to_string(),
            previous: "清".to_string(),
            current: "乾淨".to_string(),
            probability: -1.12,
        }];
        let existing_phrases = HashSet::new();
        let qstring_weights = HashMap::from([
            ("p=".to_string(), -1.163153),
            (";:^l".to_string(), -0.807221),
        ]);
        let phrase_weights = HashMap::from([("清".to_string(), -1.163174)]);

        let joined = joined_phrase_records_from_bigrams(
            &records,
            &existing_phrases,
            &qstring_weights,
            &phrase_weights,
            7,
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
        );

        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].qstring, "p=;:^l");
        assert_eq!(joined[0].phrase, "清乾淨");
        assert_eq!(joined[0].weight, -1.960374);
        assert_eq!(
            joined[0].tags,
            "unigram,chiaki-web-overlay,bigram-joined-phrase"
        );
    }

    #[test]
    fn skips_joined_phrase_unigrams_when_phrase_already_exists() {
        use crate::types::BigramRecord;

        let records = vec![BigramRecord {
            qstring: "M1 ;:^l".to_string(),
            previous: "擦".to_string(),
            current: "乾淨".to_string(),
            probability: -0.1,
        }];
        let existing_phrases = HashSet::from(["擦乾淨".to_string()]);
        let qstring_weights =
            HashMap::from([("M1".to_string(), -1.1), (";:^l".to_string(), -0.807221)]);
        let phrase_weights = HashMap::from([("擦".to_string(), -1.11)]);

        let joined = joined_phrase_records_from_bigrams(
            &records,
            &existing_phrases,
            &qstring_weights,
            &phrase_weights,
            7,
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
        );

        assert!(joined.is_empty());
    }

    #[test]
    fn skips_joined_phrase_unigrams_for_contextual_multiword_prefixes() {
        use crate::types::BigramRecord;

        let records = vec![BigramRecord {
            qstring: "CE^: p=".to_string(),
            previous: "臺灣".to_string(),
            current: "清".to_string(),
            probability: -1.09,
        }];
        let existing_phrases = HashSet::new();
        let qstring_weights =
            HashMap::from([("CE^:".to_string(), -1.0), ("p=".to_string(), -1.163153)]);
        let phrase_weights = HashMap::from([("清".to_string(), -1.163174)]);

        let joined = joined_phrase_records_from_bigrams(
            &records,
            &existing_phrases,
            &qstring_weights,
            &phrase_weights,
            7,
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
        );

        assert!(joined.is_empty());
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
    fn parses_multi_char_fragment_demotion_rows() {
        // Unlike variant demotions (single-char only), fragment caps are 2+ chars.
        let path = temp_file(
            "fragment-demotions",
            "# phrase\tmax_weight\ttags\n會比\t-1.80706\tchiakey-fragment-denylist,fragment-demote\n單\t-2.0\tchiakey-fragment-denylist,fragment-demote\n",
        );

        let (records, seen, skipped) = parse_fragment_demotions(&path).unwrap();

        assert_eq!(seen, 2);
        assert_eq!(skipped, 1, "single-character rows are rejected");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].phrase, "會比");
        assert_eq!(records[0].max_weight, -1.80706);
        assert_eq!(
            records[0].tags,
            "unigram,chiakey-fragment-denylist,fragment-demote"
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
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) = parse_rime_essay(
            &path,
            &cfg,
            &char_readings,
            &existing_phrases,
            &existing_qstring_weights,
            &normalization,
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
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, _seen, _skipped) = parse_rime_essay(
            &path,
            &cfg,
            &char_readings,
            &existing_phrases,
            &existing_qstring_weights,
            &normalization,
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
    fn applies_conversion_rules_to_rime_supplemental_phrases() {
        let path = temp_file("rime-conversion-supplemental", "喫壞\t539\n因爲\t474154\n");
        let cfg = test_config();
        let char_readings = HashMap::from([
            ("吃".to_string(), "@0".to_string()),
            ("壞".to_string(), "4e".to_string()),
            ("因".to_string(), "Q;".to_string()),
            ("爲".to_string(), "2f".to_string()),
        ]);
        let existing_phrases = HashSet::new();
        let existing_qstring_weights = HashMap::new();
        let conversion_rules = vec![ConversionRule {
            from: "喫".to_string(),
            to: "吃".to_string(),
            tags: "rime-conversion,modern-zh-tw".to_string(),
        }];
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) = parse_rime_essay(
            &path,
            &cfg,
            &char_readings,
            &existing_phrases,
            &existing_qstring_weights,
            &normalization,
        )
        .unwrap();

        assert_eq!(seen, 2);
        assert_eq!(skipped, 0);
        let converted = records
            .iter()
            .find(|record| record.phrase == "吃壞")
            .expect("喫壞 should import as 吃壞");
        assert_eq!(converted.qstring, "@04e");
        assert!(records.iter().all(|record| record.phrase != "喫壞"));
        assert_eq!(
            converted.tags,
            "unigram,rime-essay,supplemental,conversion-fix,rime-conversion,modern-zh-tw"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn applies_conversion_rules_to_rime_existing_reranks() {
        let path = temp_file(
            "rime-conversion-existing-rerank",
            "喫壞\t539\n就是\t664963\n",
        );
        let cfg = test_config();
        let existing = vec![("@04e".to_string(), "吃壞".to_string(), -3.2)];
        let existing_qstring_weights =
            HashMap::from([("@0".to_string(), -1.254153), ("4e".to_string(), -1.530062)]);
        let conversion_rules = vec![ConversionRule {
            from: "喫".to_string(),
            to: "吃".to_string(),
            tags: "rime-conversion,modern-zh-tw".to_string(),
        }];
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) = parse_rime_existing_phrase_reranks(
            &path,
            &cfg,
            &existing,
            &existing_qstring_weights,
            &normalization,
        )
        .unwrap();

        assert_eq!(seen, 2);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "@04e");
        assert_eq!(records[0].phrase, "吃壞");
        assert_eq!(
            records[0].tags,
            "unigram,rime-essay,existing-rerank,conversion-fix"
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
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) =
            parse_rime_overlap_reranks(&path, &cfg, &existing, &normalization).unwrap();

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
    fn parses_conversion_rule_rows() {
        let path = temp_file(
            "rime-conversion-rules",
            "# from\tto\ttags\n喫\t吃\trime-conversion,modern-zh-tw\n羣\t群\trime-conversion,modern-zh-tw\n裏面\t裡面\trime-conversion,modern-zh-tw,phrase-preference\n",
        );

        let (records, seen, skipped) = parse_conversion_rules(&path).unwrap();

        assert_eq!(seen, 3);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].from, "喫");
        assert_eq!(records[0].to, "吃");
        assert_eq!(records[0].tags, "rime-conversion,modern-zh-tw");
        assert_eq!(records[1].from, "羣");
        assert_eq!(records[1].to, "群");
        assert_eq!(records[1].tags, "rime-conversion,modern-zh-tw");
        assert_eq!(records[2].from, "裏面");
        assert_eq!(records[2].to, "裡面");
        assert_eq!(
            records[2].tags,
            "rime-conversion,modern-zh-tw,phrase-preference"
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn limits_overlap_rerank_boost_for_far_weaker_candidates() {
        let path = temp_file("rime-overlap-boost-limit", "回文\t171\n迴文\t525\n");
        let cfg = test_config();
        let existing = vec![
            ("}FGK".to_string(), "回文".to_string(), -1.062714),
            ("}FGK".to_string(), "迴文".to_string(), -2.302193),
        ];
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) =
            parse_rime_overlap_reranks(&path, &cfg, &existing, &normalization).unwrap();

        assert_eq!(seen, 2);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "}FGK");
        assert_eq!(records[0].phrase, "迴文");
        assert_eq!(records[0].weight, -1.952193);
        assert!(records[0].weight < -1.062714);
        assert_eq!(records[0].tags, "unigram,rime-essay,overlap-rerank");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn does_not_overtake_when_overlap_rerank_would_need_too_much_boost() {
        let path = temp_file("rime-overlap-close-limit", "現成\t1560\n縣城\t3617\n");
        let cfg = test_config();
        let existing = vec![
            ("Ei=M".to_string(), "現成".to_string(), -1.128131),
            ("Ei=M".to_string(), "縣城".to_string(), -1.483416),
        ];
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) =
            parse_rime_overlap_reranks(&path, &cfg, &existing, &normalization).unwrap();

        assert_eq!(seen, 2);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "Ei=M");
        assert_eq!(records[0].phrase, "縣城");
        assert_eq!(records[0].weight, -1.133416);
        assert!(records[0].weight < -1.128131);
        assert_eq!(records[0].tags, "unigram,rime-essay,overlap-rerank");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn reranks_existing_weak_phrases_with_rime_scores() {
        let path = temp_file(
            "rime-existing-rerank",
            "網友\t16320\n回文\t171\n就是\t664963\n",
        );
        let cfg = test_config();
        let existing = vec![
            ("0\\NX".to_string(), "網友".to_string(), -2.3),
            ("}FGK".to_string(), "回文".to_string(), -1.062714),
        ];
        let existing_qstring_weights = HashMap::from([
            ("0\\".to_string(), -1.289465),
            ("NX".to_string(), -0.570839),
        ]);
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) = parse_rime_existing_phrase_reranks(
            &path,
            &cfg,
            &existing,
            &existing_qstring_weights,
            &normalization,
        )
        .unwrap();

        assert_eq!(seen, 3);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "0\\NX");
        assert_eq!(records[0].phrase, "網友");
        assert_eq!(records[0].weight, -1.850304);
        assert_eq!(records[0].tags, "unigram,rime-essay,existing-rerank");

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
        let conversion_rules = Vec::new();
        let normalization = RimeNormalization::without_opencc(&conversion_rules);

        let (records, seen, skipped) =
            parse_rime_overlap_reranks(&path, &cfg, &existing, &normalization).unwrap();

        assert_eq!(seen, 6);
        assert_eq!(skipped, 0);
        assert!(records.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn reranks_single_char_homophones_when_rime_winner_is_confident() {
        let path = temp_file(
            "single-char-homophone-rerank",
            "進\t25597\n近\t15920\n盡\t5462\n勁\t3590\n禁\t2336\n浸\t1290\n",
        );
        let existing = vec![
            ("Lj".to_string(), "進".to_string(), -1.053670),
            ("Lj".to_string(), "勁".to_string(), -1.053657),
            ("Lj".to_string(), "近".to_string(), -1.053680),
            ("Df".to_string(), "的".to_string(), -0.1),
        ];

        let (records, seen, skipped) =
            parse_single_char_homophone_reranks(&path, &existing, 5.0).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "Lj");
        assert_eq!(records[0].phrase, "進");
        assert_eq!(
            records[0].weight,
            round6(-1.053657 + SINGLE_CHAR_HOMOPHONE_RERANK_MARGIN)
        );
        assert_eq!(records[0].tags, "unigram,rime-essay,homophone-rerank");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_single_char_homophones_when_rime_advantage_is_too_small() {
        let path = temp_file(
            "single-char-homophone-rerank-ratio",
            "進\t17900\n勁\t3590\n",
        );
        let existing = vec![
            ("Lj".to_string(), "進".to_string(), -1.053670),
            ("Lj".to_string(), "勁".to_string(), -1.053657),
        ];

        let (records, seen, skipped) =
            parse_single_char_homophone_reranks(&path, &existing, 5.0).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 1);
        assert!(records.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn reranks_tied_single_char_homophones_with_rime_frequency() {
        let path = temp_file(
            "single-char-homophone-rerank-tie",
            "行\t39910\n型\t15224\n形\t9000\n刑\t3000\n",
        );
        let existing = vec![
            ("QM".to_string(), "型".to_string(), -0.902038),
            ("QM".to_string(), "行".to_string(), -0.902038),
            ("QM".to_string(), "形".to_string(), -0.902045),
            ("QM".to_string(), "刑".to_string(), -0.902045),
        ];

        let (records, seen, skipped) =
            parse_single_char_homophone_reranks(&path, &existing, 2.5).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 0);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "QM");
        assert_eq!(records[0].phrase, "行");
        assert_eq!(
            records[0].weight,
            round6(-0.902038 + SINGLE_CHAR_HOMOPHONE_RERANK_MARGIN)
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_single_char_homophones_when_winner_has_weak_reading_evidence() {
        let path = temp_file(
            "single-char-homophone-rerank-reading-gap",
            "啊\t84662\n喔\t6253\n呵\t42\n",
        );
        let existing = vec![
            ("B2".to_string(), "啊".to_string(), -3.094452),
            ("B2".to_string(), "喔".to_string(), -1.109748),
            ("B2".to_string(), "呵".to_string(), -3.094452),
        ];

        let (records, seen, skipped) =
            parse_single_char_homophone_reranks(&path, &existing, 2.5).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 1);
        assert!(records.is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn keeps_single_char_homophones_when_current_top_lacks_rime_frequency() {
        let path = temp_file(
            "single-char-homophone-rerank-missing-top",
            "進\t25597\n近\t15920\n",
        );
        let existing = vec![
            ("Lj".to_string(), "進".to_string(), -1.053670),
            ("Lj".to_string(), "勁".to_string(), -1.053657),
        ];

        let (records, seen, skipped) =
            parse_single_char_homophone_reranks(&path, &existing, 5.0).unwrap();

        assert_eq!(seen, 1);
        assert_eq!(skipped, 1);
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
            opencc_binary: PathBuf::from("opencc"),
            opencc_t2tw_config: PathBuf::from("t2tw.json"),
            synthetic_bigram_boost: 0.0,
            commonvoice_bigram_boost: 0.0,
            homophone_rerank_min_ratio: 5.0,
            dist_dir: PathBuf::new(),
            normalized_path: PathBuf::new(),
            manifest_path: PathBuf::new(),
        }
    }
}

fn parse_i64(value: impl AsRef<str>) -> Option<i64> {
    value.as_ref().trim().parse().ok()
}
