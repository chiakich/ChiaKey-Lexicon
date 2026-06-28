use crate::config::{PUNCTUATION_SOURCE_ID, SYMBOL_OVERLAY_SOURCE_ID};
use crate::types::SourceRecord;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

const BASE_WEIGHT: f64 = 0.0;
const SYMBOL_OVERLAY_BASE_WEIGHT: f64 = -0.001;
const RANK_STEP: f64 = 0.000001;
const PUNCTUATION_LIST_KEY: &str = "_punctuation_list";

pub fn parse_cin(path: &Path) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut ranks: HashMap<String, usize> = HashMap::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
    let mut records = Vec::new();
    let mut seen = 0;
    let mut skipped = 0;
    let mut in_chardef = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        match line {
            "%chardef  begin" | "%chardef begin" => {
                in_chardef = true;
                continue;
            }
            "%chardef  end" | "%chardef end" => break,
            _ => {}
        }

        if !in_chardef {
            continue;
        }

        seen += 1;
        match parse_chardef_line(line, &mut ranks) {
            Some(record) => {
                let key = (record.qstring.clone(), record.phrase.clone());
                if seen_pairs.insert(key) {
                    records.push(record);
                } else {
                    skipped += 1;
                }
            }
            None => skipped += 1,
        }
    }

    Ok((records, seen, skipped))
}

pub fn parse_symbol_overlay(
    path: &Path,
    existing_exact_keys: &HashSet<(String, String)>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
    let mut records = Vec::new();
    let mut seen = 0;
    let mut skipped = 0;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        seen += 1;
        let parts = line.splitn(2, '\t').collect::<Vec<_>>();
        let symbol = parts[0].trim();
        if symbol.is_empty() || symbol.contains('\n') {
            skipped += 1;
            continue;
        }

        let key = (PUNCTUATION_LIST_KEY.to_string(), symbol.to_string());
        if existing_exact_keys.contains(&key) || !seen_pairs.insert(key) {
            skipped += 1;
            continue;
        }

        let rank = records.len();
        let tag_suffix = parts
            .get(1)
            .map(|tags| tags.trim())
            .filter(|tags| !tags.is_empty())
            .unwrap_or("supplemental-symbol");
        records.push(SourceRecord {
            qstring: PUNCTUATION_LIST_KEY.to_string(),
            phrase: symbol.to_string(),
            weight: SYMBOL_OVERLAY_BASE_WEIGHT - (rank as f64 * RANK_STEP),
            source_id: SYMBOL_OVERLAY_SOURCE_ID,
            tags: format!("unigram,{SYMBOL_OVERLAY_SOURCE_ID},{tag_suffix}"),
        });
    }

    Ok((records, seen, skipped))
}

pub fn parse_symbol_alternatives(
    path: &Path,
    existing_exact_keys: &HashSet<(String, String)>,
) -> Result<(Vec<SourceRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut ranks: HashMap<String, usize> = HashMap::new();
    let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
    let mut records = Vec::new();
    let mut seen = 0;
    let mut skipped = 0;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        seen += 1;
        let parts = line.splitn(3, '\t').collect::<Vec<_>>();
        if parts.len() < 2 {
            skipped += 1;
            continue;
        }

        let qstring = parts[0].trim();
        let symbol = parts[1].trim();
        if qstring.is_empty()
            || symbol.is_empty()
            || qstring.contains('\n')
            || symbol.contains('\n')
        {
            skipped += 1;
            continue;
        }

        let key = (qstring.to_string(), symbol.to_string());
        if existing_exact_keys.contains(&key) || !seen_pairs.insert(key) {
            skipped += 1;
            continue;
        }

        let rank = ranks.entry(qstring.to_string()).or_default();
        let tag_suffix = parts
            .get(2)
            .map(|tags| tags.trim())
            .filter(|tags| !tags.is_empty())
            .unwrap_or("punctuation-alternative");
        records.push(SourceRecord {
            qstring: qstring.to_string(),
            phrase: symbol.to_string(),
            weight: SYMBOL_OVERLAY_BASE_WEIGHT - (*rank as f64 * RANK_STEP),
            source_id: SYMBOL_OVERLAY_SOURCE_ID,
            tags: format!("unigram,{SYMBOL_OVERLAY_SOURCE_ID},{tag_suffix}"),
        });
        *rank += 1;
    }

    Ok((records, seen, skipped))
}

fn parse_chardef_line(line: &str, ranks: &mut HashMap<String, usize>) -> Option<SourceRecord> {
    let split_at = line.find(char::is_whitespace)?;
    let key = line[..split_at].trim();
    let phrase = line[split_at..].trim();
    if key.is_empty()
        || phrase.is_empty()
        || !(key.starts_with("_punctuation_") || key.starts_with("_ctrl_"))
    {
        return None;
    }

    let rank = ranks.entry(key.to_string()).or_default();
    let weight = BASE_WEIGHT - (*rank as f64 * RANK_STEP);
    *rank += 1;

    Some(SourceRecord {
        qstring: key.to_string(),
        phrase: phrase.to_string(),
        weight,
        source_id: PUNCTUATION_SOURCE_ID,
        tags: format!("unigram,{PUNCTUATION_SOURCE_ID},keykey-punctuation"),
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_chardef_line, parse_symbol_alternatives, parse_symbol_overlay};
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn keeps_runtime_punctuation_and_ctrl_keys() {
        let mut ranks = HashMap::new();
        let punctuation = parse_chardef_line("_punctuation_< ，", &mut ranks).unwrap();
        let standard = parse_chardef_line("_punctuation_Standard_< ，", &mut ranks).unwrap();
        let ctrl = parse_chardef_line("_ctrl_, ，", &mut ranks).unwrap();

        assert_eq!(punctuation.qstring, "_punctuation_<");
        assert_eq!(punctuation.phrase, "，");
        assert_eq!(standard.qstring, "_punctuation_Standard_<");
        assert_eq!(standard.phrase, "，");
        assert_eq!(ctrl.qstring, "_ctrl_,");
        assert_eq!(ctrl.phrase, "，");
    }

    #[test]
    fn skips_non_runtime_keys_and_preserves_rank_order() {
        let mut ranks = HashMap::new();
        assert!(parse_chardef_line("a ㄅ", &mut ranks).is_none());

        let first = parse_chardef_line("_punctuation_{ 『", &mut ranks).unwrap();
        let second = parse_chardef_line("_punctuation_{ 《", &mut ranks).unwrap();

        assert!(first.weight > second.weight);
    }

    #[test]
    fn appends_symbol_overlay_without_replacing_existing_punctuation() {
        let path = temp_file(
            "symbols",
            "# symbol<TAB>tags\n€\tcurrency\n，\tduplicate\n€\tduplicate\n",
        );
        let existing_exact_keys =
            HashSet::from([("_punctuation_list".to_string(), "，".to_string())]);

        let (records, seen, skipped) = parse_symbol_overlay(&path, &existing_exact_keys).unwrap();

        assert_eq!(seen, 3);
        assert_eq!(skipped, 2);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "_punctuation_list");
        assert_eq!(records[0].phrase, "€");
        assert_eq!(records[0].tags, "unigram,chiakey-symbols-overlay,currency");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn appends_symbol_alternatives_to_runtime_punctuation_keys() {
        let path = temp_file(
            "symbol-alternatives",
            "# qstring<TAB>symbol<TAB>tags\n_punctuation_[\t『\tquote\n_punctuation_[\t「\tduplicate\n_punctuation_[\t『\tduplicate\n",
        );
        let existing_exact_keys = HashSet::from([("_punctuation_[".to_string(), "「".to_string())]);

        let (records, seen, skipped) =
            parse_symbol_alternatives(&path, &existing_exact_keys).unwrap();

        assert_eq!(seen, 3);
        assert_eq!(skipped, 2);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].qstring, "_punctuation_[");
        assert_eq!(records[0].phrase, "『");
        assert_eq!(records[0].tags, "unigram,chiakey-symbols-overlay,quote");

        let _ = fs::remove_file(path);
    }

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("chiakey-lexicon-{name}-{}.tsv", std::process::id()));
        fs::write(&path, content).unwrap();
        path
    }
}
