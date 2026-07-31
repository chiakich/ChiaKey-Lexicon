//! `build-bigram-stats`: counts adjacent token pairs and emits the bigram
//! overlay plus its stats and review sidecars.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use super::args::{Args, DocumentBoundary};
use super::lexicon::Lexicon;
use super::review;
use super::text::{contains_excluded_particle, han_sentences, tokenize_sentence};
use super::{flush_doc_counts, Occurrences};

#[derive(Hash, Eq, PartialEq, Clone)]
pub(super) struct BigramKey {
    previous: String,
    current: String,
}

pub(super) fn count(
    path: &Path,
    lexicon: &Lexicon,
    document_boundary: DocumentBoundary,
    max_examples_per_candidate: usize,
) -> Result<HashMap<BigramKey, Occurrences>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut counts = HashMap::<BigramKey, Occurrences>::new();
    let mut seen_in_doc = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        if matches!(document_boundary, DocumentBoundary::BlankLine) && line.trim().is_empty() {
            flush_doc_counts(&mut counts, &mut seen_in_doc);
            continue;
        }

        count_line(
            &line,
            lexicon,
            &mut counts,
            &mut seen_in_doc,
            max_examples_per_candidate,
        );

        if matches!(document_boundary, DocumentBoundary::Line) {
            flush_doc_counts(&mut counts, &mut seen_in_doc);
        }
    }
    flush_doc_counts(&mut counts, &mut seen_in_doc);

    Ok(counts)
}

fn count_line(
    line: &str,
    lexicon: &Lexicon,
    counts: &mut HashMap<BigramKey, Occurrences>,
    seen_in_doc: &mut HashSet<BigramKey>,
    max_examples_per_candidate: usize,
) {
    for sentence in han_sentences(line) {
        let tokens = tokenize_sentence(&sentence, lexicon);
        for pair in tokens.windows(2) {
            if pair[0] == pair[1] {
                continue;
            }
            let key = BigramKey {
                previous: pair[0].clone(),
                current: pair[1].clone(),
            };
            let entry = counts.entry(key.clone()).or_default();
            entry.count += 1;
            if max_examples_per_candidate > 0
                && entry.examples.len() < max_examples_per_candidate
                && !entry.examples.iter().any(|example| example == &sentence)
            {
                entry.examples.push(sentence.clone());
            }
            seen_in_doc.insert(key);
        }
    }
}

pub(super) fn write_outputs(
    args: &Args,
    lexicon: &Lexicon,
    counts: &HashMap<BigramKey, Occurrences>,
) -> Result<()> {
    let mut rows = counts.iter().collect::<Vec<_>>();
    rows.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .count
            .cmp(&left_count.count)
            .then_with(|| right_count.doc_count.cmp(&left_count.doc_count))
            .then_with(|| left_key.previous.cmp(&right_key.previous))
            .then_with(|| left_key.current.cmp(&right_key.current))
    });

    let output =
        File::create(&args.output).with_context(|| format!("create {}", args.output.display()))?;
    let stats =
        File::create(&args.stats).with_context(|| format!("create {}", args.stats.display()))?;
    let mut output = BufWriter::new(output);
    let mut stats = BufWriter::new(stats);

    writeln!(
        stats,
        "previous\tcurrent\tcount\tdoc_count\tselected\tredundant\texcluded_particle\texcluded_single_char_pair\texcluded_joined_unigram\tprevious_rank\tcurrent_rank\tprevious_qstring\tcurrent_qstring"
    )?;
    writeln!(output, "# qstring\tprevious\tcurrent\tprobability")?;

    let mut emitted = 0_usize;
    let mut redundant = 0_usize;
    let mut excluded_particle = 0_usize;
    let mut excluded_single_char_pair = 0_usize;
    let mut excluded_joined_unigram = 0_usize;
    let mut review_rows = Vec::new();
    for (key, count) in rows {
        let Some(previous) = lexicon.by_phrase.get(&key.previous) else {
            continue;
        };
        let Some(current) = lexicon.by_phrase.get(&key.current) else {
            continue;
        };
        let previous_rank = lexicon.rank(&key.previous, previous);
        let current_rank = lexicon.rank(&key.current, current);
        let is_redundant = is_redundant_pair(previous_rank, current_rank);
        if is_redundant {
            redundant += 1;
        }
        let has_excluded_particle =
            contains_excluded_particle(&key.previous) || contains_excluded_particle(&key.current);
        if has_excluded_particle {
            excluded_particle += 1;
        }
        let is_single_char_pair = is_single_char_pair(&key.previous, &key.current);
        if is_single_char_pair {
            excluded_single_char_pair += 1;
        }
        let has_joined_unigram = lexicon.has_joined_unigram(&key.previous, &key.current);
        if has_joined_unigram {
            excluded_joined_unigram += 1;
        }

        // Single-character pairs are no longer excluded: that guard dated from when
        // bigrams were weak enough that a mis-picked single-char reading could hijack
        // a position. Reading selection and the variant demotion policy now cover it,
        // and held-out measurement shows the pairs carry substantial benefit. The
        // stats column is kept so the effect stays observable.
        let is_eligible = count.count >= args.min_count
            && count.doc_count >= args.min_doc_count
            && (!is_redundant || args.include_redundant)
            && !has_excluded_particle
            && !has_joined_unigram;
        let within_top_n = args.top_n.map(|limit| emitted < limit).unwrap_or(true);
        let should_emit = is_eligible && within_top_n;

        if is_eligible || args.include_excluded_stats {
            writeln!(
                stats,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                key.previous,
                key.current,
                count.count,
                count.doc_count,
                should_emit,
                is_redundant,
                has_excluded_particle,
                is_single_char_pair,
                has_joined_unigram,
                previous_rank,
                current_rank,
                previous.qstring,
                current.qstring
            )?;
        }

        if !should_emit {
            continue;
        }

        writeln!(
            output,
            "{} {}\t{}\t{}\t{}",
            previous.qstring, current.qstring, key.previous, key.current, args.probability
        )?;
        review_rows.push(review::ReviewRow {
            previous: key.previous.clone(),
            current: key.current.clone(),
            count: count.count,
            doc_count: count.doc_count,
            previous_qstring: previous.qstring.clone(),
            current_qstring: current.qstring.clone(),
            previous_rank,
            current_rank,
            probability: args.probability,
            examples: count.examples.clone(),
        });
        emitted += 1;
    }

    if let Some(review_path) = &args.review {
        review::write_review(review_path, &review_rows)?;
    }

    eprintln!(
        "bigram stats: pairs={} redundant={} excluded_particle={} excluded_single_char_pair={} excluded_joined_unigram={} emitted={} min_count={} min_doc_count={} top_n={}",
        counts.len(),
        redundant,
        excluded_particle,
        excluded_single_char_pair,
        excluded_joined_unigram,
        emitted,
        args.min_count,
        args.min_doc_count,
        args.top_n
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unlimited".to_string())
    );

    Ok(())
}

/// A pair where both sides are already the top candidate for their qstring
/// teaches the engine nothing it would not already pick.
fn is_redundant_pair(previous_rank: usize, current_rank: usize) -> bool {
    previous_rank == 1 && current_rank == 1
}

fn is_single_char_pair(previous: &str, current: &str) -> bool {
    previous.chars().count() == 1 && current.chars().count() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_limited_review_examples_for_bigram_counts() {
        let lexicon = Lexicon::for_tests(
            &[("台北", "a", 0.0), ("捷運", "b", 0.0), ("方便", "c", 0.0)],
            2,
        );
        let mut counts = HashMap::new();
        let mut seen_in_doc = HashSet::new();

        count_line("台北捷運方便", &lexicon, &mut counts, &mut seen_in_doc, 1);
        count_line("台北捷運", &lexicon, &mut counts, &mut seen_in_doc, 1);

        let count = counts
            .get(&BigramKey {
                previous: "台北".to_string(),
                current: "捷運".to_string(),
            })
            .unwrap();
        assert_eq!(count.count, 2);
        assert_eq!(count.examples, vec!["台北捷運方便".to_string()]);
    }

    #[test]
    fn detects_single_character_pairs() {
        assert!(is_single_char_pair("台", "積"));
        assert!(!is_single_char_pair("台灣", "人"));
        assert!(!is_single_char_pair("我", "覺得"));
    }

    #[test]
    fn treats_top_ranked_pairs_as_redundant() {
        assert!(is_redundant_pair(1, 1));
        assert!(!is_redundant_pair(2, 1));
    }
}
