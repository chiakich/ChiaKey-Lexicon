//! `build-unigram-candidates`: finds runs of adjacent tokens whose joined
//! phrase is missing from the lexicon, as raw material for new entries.

use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use super::args::{DocumentBoundary, UnigramCandidateArgs};
use super::lexicon::Lexicon;
use super::text::{contains_excluded_particle, han_sentences, tokenize_sentence};
use super::{flush_doc_counts, Occurrences};

#[derive(Hash, Eq, PartialEq, Clone)]
pub(super) struct UnigramCandidateKey {
    phrase: String,
    qstring: String,
    tokens: Vec<String>,
}

pub(super) fn count(
    path: &Path,
    lexicon: &Lexicon,
    args: &UnigramCandidateArgs,
) -> Result<HashMap<UnigramCandidateKey, Occurrences>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut counts = HashMap::<UnigramCandidateKey, Occurrences>::new();
    let mut seen_in_doc = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        if matches!(args.document_boundary, DocumentBoundary::BlankLine) && line.trim().is_empty() {
            flush_doc_counts(&mut counts, &mut seen_in_doc);
            continue;
        }

        count_line(&line, lexicon, args, &mut counts, &mut seen_in_doc);

        if matches!(args.document_boundary, DocumentBoundary::Line) {
            flush_doc_counts(&mut counts, &mut seen_in_doc);
        }
    }
    flush_doc_counts(&mut counts, &mut seen_in_doc);

    Ok(counts)
}

fn count_line(
    line: &str,
    lexicon: &Lexicon,
    args: &UnigramCandidateArgs,
    counts: &mut HashMap<UnigramCandidateKey, Occurrences>,
    seen_in_doc: &mut HashSet<UnigramCandidateKey>,
) {
    for sentence in han_sentences(line) {
        let tokens = tokenize_sentence(&sentence, lexicon);
        for start in 0..tokens.len() {
            let max_end = (start + args.max_tokens).min(tokens.len());
            for end in (start + args.min_tokens)..=max_end {
                let token_slice = &tokens[start..end];
                if token_slice
                    .iter()
                    .any(|token| contains_excluded_particle(token))
                {
                    continue;
                }

                let phrase = token_slice.concat();
                let codepoints = phrase.chars().count();
                if codepoints > args.max_candidate_codepoints {
                    continue;
                }
                if token_slice.len() >= 3
                    && args.max_multi_token_codepoints > 0
                    && codepoints > args.max_multi_token_codepoints
                {
                    continue;
                }
                if lexicon.by_phrase.contains_key(&phrase) {
                    continue;
                }
                let Some(qstring) = qstring_for_tokens(token_slice, lexicon) else {
                    continue;
                };
                let key = UnigramCandidateKey {
                    phrase,
                    qstring,
                    tokens: token_slice.to_vec(),
                };
                let entry = counts.entry(key.clone()).or_default();
                entry.count += 1;
                seen_in_doc.insert(key);
            }
        }
    }
}

/// Concatenates each token's best-weight qstring. The reading is a proposal:
/// the corpus carries no pronunciation, so 破音字 must be reviewed by hand.
fn qstring_for_tokens(tokens: &[String], lexicon: &Lexicon) -> Option<String> {
    let mut qstring = String::new();
    for token in tokens {
        qstring.push_str(&lexicon.by_phrase.get(token)?.qstring);
    }
    Some(qstring)
}

pub(super) fn write_outputs(
    args: &UnigramCandidateArgs,
    counts: &HashMap<UnigramCandidateKey, Occurrences>,
) -> Result<()> {
    let mut rows = counts
        .iter()
        .filter(|(_, count)| count.count >= args.min_count && count.doc_count >= args.min_doc_count)
        .collect::<Vec<_>>();
    rows.sort_by(|(left_key, left_count), (right_key, right_count)| {
        right_count
            .count
            .cmp(&left_count.count)
            .then_with(|| right_count.doc_count.cmp(&left_count.doc_count))
            .then_with(|| left_key.phrase.cmp(&right_key.phrase))
            .then_with(|| left_key.tokens.cmp(&right_key.tokens))
    });

    if let Some(parent) = args.output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
    }

    let output =
        File::create(&args.output).with_context(|| format!("create {}", args.output.display()))?;
    let mut output = BufWriter::new(output);

    writeln!(
        output,
        "qstring\tphrase\tweight\tsource\ttags\tcount\tdoc_count\ttoken_count\ttokens"
    )?;

    for (key, count) in &rows {
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            key.qstring,
            key.phrase,
            args.weight,
            args.source,
            args.tags,
            count.count,
            count.doc_count,
            key.tokens.len(),
            key.tokens.join(" ")
        )?;
    }

    eprintln!(
        "unigram candidate stats: candidates={} emitted={} min_count={} min_doc_count={} min_tokens={} max_tokens={}",
        counts.len(),
        rows.len(),
        args.min_count,
        args.min_doc_count,
        args.min_tokens,
        args.max_tokens
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Default args for counting tests; output paths are unused because the
    /// tests drive `count_line` directly.
    fn test_args(
        min_tokens: usize,
        max_tokens: usize,
        max_candidate_codepoints: usize,
        max_multi_token_codepoints: usize,
    ) -> UnigramCandidateArgs {
        UnigramCandidateArgs {
            input: PathBuf::new(),
            output: PathBuf::new(),
            lexicon: PathBuf::new(),
            max_lexicon_phrase_codepoints: 7,
            min_count: 1,
            min_doc_count: 1,
            min_tokens,
            max_tokens,
            max_candidate_codepoints,
            max_multi_token_codepoints,
            weight: -2.4,
            source: "test".to_string(),
            tags: "test".to_string(),
            document_boundary: DocumentBoundary::Line,
        }
    }

    #[test]
    fn counts_missing_joined_unigram_candidates() {
        let lexicon = Lexicon::for_tests(
            &[
                ("塞克", "a", 0.0),
                ("斯", "b", 0.0),
                ("在", "c", 0.0),
                ("美國", "d", 0.0),
            ],
            4,
        );
        let args = test_args(2, 2, 4, 0);
        let mut counts = HashMap::new();
        let mut seen_in_doc = HashSet::new();

        count_line("塞克斯", &lexicon, &args, &mut counts, &mut seen_in_doc);
        count_line("在美國", &lexicon, &args, &mut counts, &mut seen_in_doc);
        flush_doc_counts(&mut counts, &mut seen_in_doc);

        assert_eq!(counts.len(), 1);
        let (key, count) = counts.iter().next().unwrap();
        assert_eq!(key.phrase, "塞克斯");
        assert_eq!(key.qstring, "ab");
        assert_eq!(key.tokens, vec!["塞克".to_string(), "斯".to_string()]);
        assert_eq!(count.count, 1);
        assert_eq!(count.doc_count, 1);
    }

    #[test]
    fn can_limit_long_multi_token_candidates() {
        let lexicon = Lexicon::for_tests(
            &[
                ("鬼", "a", 0.0),
                ("滅", "b", 0.0),
                ("刃", "c", 0.0),
                ("布林", "d", 0.0),
                ("什", "e", 0.0),
                ("維克", "f", 0.0),
                ("專屬", "g", 0.0),
                ("福利", "h", 0.0),
                ("與", "i", 0.0),
                ("優惠", "j", 0.0),
            ],
            4,
        );
        let args = test_args(3, 4, 7, 4);
        let mut counts = HashMap::new();
        let mut seen_in_doc = HashSet::new();

        count_line("鬼滅刃", &lexicon, &args, &mut counts, &mut seen_in_doc);
        count_line("布林什維克", &lexicon, &args, &mut counts, &mut seen_in_doc);
        count_line(
            "專屬福利與優惠",
            &lexicon,
            &args,
            &mut counts,
            &mut seen_in_doc,
        );
        flush_doc_counts(&mut counts, &mut seen_in_doc);

        assert_eq!(counts.len(), 1);
        assert_eq!(counts.keys().next().unwrap().phrase, "鬼滅刃");
    }
}
