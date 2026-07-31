//! Corpus counting for the two lexicon-building subcommands.
//!
//! Both commands share the same front half — parse args, load the lexicon,
//! walk the corpus splitting it into Han-only sentences, tokenize each
//! sentence against the lexicon — and diverge only in what they tally and
//! emit. That shared half lives in [`args`], [`lexicon`] and [`text`]; the
//! per-command halves live in [`bigrams`] and [`candidates`].

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

mod args;
mod bigrams;
mod candidates;
mod lexicon;
mod review;
mod text;

/// Per-key corpus tallies shared by both counters. `examples` is only filled
/// for bigrams, and only when `--review` asks for sample sentences.
#[derive(Default)]
struct Occurrences {
    count: usize,
    doc_count: usize,
    examples: Vec<String>,
}

pub fn run(args: impl Iterator<Item = String>) -> Result<()> {
    let args = args::parse_args(args)?;
    let lexicon = lexicon::load(&args.lexicon, args.max_phrase_codepoints)?;
    let example_limit = args
        .review
        .as_ref()
        .map(|_| args.review_examples)
        .unwrap_or(0);
    let counts = bigrams::count(&args.input, &lexicon, args.document_boundary, example_limit)?;
    bigrams::write_outputs(&args, &lexicon, &counts)
}

pub fn run_unigram_candidates(args: impl Iterator<Item = String>) -> Result<()> {
    let args = args::parse_unigram_candidate_args(args)?;
    let lexicon = lexicon::load(&args.lexicon, args.max_lexicon_phrase_codepoints)?;
    let counts = candidates::count(&args.input, &lexicon, &args)?;
    candidates::write_outputs(&args, &counts)
}

/// Both counters record "seen at least once in this document" in a set, then
/// fold that set into `doc_count` at each document boundary. Only the key type
/// differs between them.
fn flush_doc_counts<K: Eq + Hash>(
    counts: &mut HashMap<K, Occurrences>,
    seen_in_doc: &mut HashSet<K>,
) {
    for key in seen_in_doc.drain() {
        counts.entry(key).or_default().doc_count += 1;
    }
}
