//! Loading the normalized lexicon that both counters tokenize against.

use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Clone)]
pub(super) struct LexiconEntry {
    pub qstring: String,
    pub weight: f64,
}

pub(super) struct Lexicon {
    /// Best (highest-weight) entry per phrase; drives tokenization.
    pub by_phrase: HashMap<String, LexiconEntry>,
    /// 1-based position of a phrase among the candidates sharing its qstring,
    /// used to tell "already the top pick" pairs from ones a bigram can move.
    pub rank_by_qstring_phrase: HashMap<(String, String), usize>,
    pub max_phrase_codepoints: usize,
}

pub(super) fn load(path: &Path, max_phrase_codepoints: usize) -> Result<Lexicon> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut by_phrase: HashMap<String, LexiconEntry> = HashMap::new();
    let mut by_qstring: HashMap<String, Vec<(String, f64)>> = HashMap::new();

    for line in reader.lines() {
        let line = line?;
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() < 3 {
            continue;
        }
        let qstring = parts[0].to_string();
        if is_special_qstring(&qstring) {
            continue;
        }
        let phrase = parts[1].to_string();
        let Ok(weight) = parts[2].parse::<f64>() else {
            continue;
        };
        let codepoints = phrase.chars().count();
        if phrase.is_empty() || codepoints > max_phrase_codepoints || phrase.contains('_') {
            continue;
        }

        by_qstring
            .entry(qstring.clone())
            .or_default()
            .push((phrase.clone(), weight));

        match by_phrase.get(&phrase) {
            Some(existing) if existing.weight >= weight => {}
            _ => {
                by_phrase.insert(phrase, LexiconEntry { qstring, weight });
            }
        }
    }

    let mut rank_by_qstring_phrase = HashMap::new();
    for (qstring, mut entries) in by_qstring {
        entries.sort_by(compare_unigram);
        for (index, (phrase, _weight)) in entries.into_iter().enumerate() {
            rank_by_qstring_phrase.insert((qstring.clone(), phrase), index + 1);
        }
    }

    Ok(Lexicon {
        by_phrase,
        rank_by_qstring_phrase,
        max_phrase_codepoints,
    })
}

impl Lexicon {
    pub(super) fn rank(&self, phrase: &str, entry: &LexiconEntry) -> usize {
        self.rank_by_qstring_phrase
            .get(&(entry.qstring.clone(), phrase.to_string()))
            .copied()
            .unwrap_or(1)
    }

    /// True when the two phrases joined together are themselves a lexicon
    /// entry, i.e. the tokenizer split a word the lexicon already knows whole.
    pub(super) fn has_joined_unigram(&self, previous: &str, current: &str) -> bool {
        let mut joined = String::with_capacity(previous.len() + current.len());
        joined.push_str(previous);
        joined.push_str(current);
        self.by_phrase.contains_key(&joined)
    }
}

fn compare_unigram(a: &(String, f64), b: &(String, f64)) -> Ordering {
    b.1.partial_cmp(&a.1)
        .unwrap_or(Ordering::Equal)
        .then_with(|| a.0.cmp(&b.0))
}

fn is_special_qstring(qstring: &str) -> bool {
    qstring.starts_with("_punctuation") || qstring.starts_with("_ctrl")
}

#[cfg(test)]
impl Lexicon {
    /// Builds a rank-less lexicon from `(phrase, qstring, weight)` triples, for
    /// tests that only exercise tokenization or phrase lookup.
    pub(super) fn for_tests(entries: &[(&str, &str, f64)], max_phrase_codepoints: usize) -> Self {
        Lexicon {
            by_phrase: entries
                .iter()
                .map(|(phrase, qstring, weight)| {
                    (
                        phrase.to_string(),
                        LexiconEntry {
                            qstring: qstring.to_string(),
                            weight: *weight,
                        },
                    )
                })
                .collect(),
            rank_by_qstring_phrase: HashMap::new(),
            max_phrase_codepoints,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes `contents` to a uniquely-named temp file, runs `load` on it, and
    /// cleans up.
    fn load_from(label: &str, contents: &str) -> Lexicon {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "chiakey-bigram-{label}-{}.tsv",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, contents).unwrap();
        let lexicon = load(&path, 7).unwrap();
        std::fs::remove_file(&path).unwrap();
        lexicon
    }

    #[test]
    fn skips_special_qstrings_when_loading_lexicon() {
        let lexicon = load_from(
            "special",
            "_punctuation_list\t十\t0.0\ttest\n_5\t拍\t-0.7\ttest\np?\t十\t-0.7\ttest\n",
        );

        assert_eq!(lexicon.by_phrase.get("十").unwrap().qstring, "p?");
        assert_eq!(lexicon.by_phrase.get("拍").unwrap().qstring, "_5");
    }

    #[test]
    fn ranks_homophones_by_descending_weight() {
        let lexicon = load_from(
            "rank",
            "a\t台北\t-0.1\ttest\n\
             a\t抬北\t-0.8\ttest\n\
             b\t捷運\t-0.2\ttest\n\
             b\t接運\t-0.9\ttest\n",
        );
        let taipei = lexicon.by_phrase.get("台北").unwrap();
        let typo = lexicon.by_phrase.get("抬北").unwrap();
        let mrt = lexicon.by_phrase.get("捷運").unwrap();

        assert_eq!(lexicon.rank("台北", taipei), 1);
        assert_eq!(lexicon.rank("抬北", typo), 2);
        assert_eq!(lexicon.rank("捷運", mrt), 1);
    }

    #[test]
    fn detects_joined_unigram_pairs() {
        let lexicon = Lexicon::for_tests(
            &[
                ("下", "L`", -1.0),
                ("意識", "5_0_", -1.0),
                ("下意識", "L`5_0_", -2.0),
            ],
            4,
        );

        assert!(lexicon.has_joined_unigram("下", "意識"));
        assert!(!lexicon.has_joined_unigram("意識", "下"));
    }
}
