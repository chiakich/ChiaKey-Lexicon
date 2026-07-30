//! Command-line parsing for `build-bigram-stats` and
//! `build-unigram-candidates`.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

/// How the corpus is divided for `doc_count` purposes: one document per line,
/// or one document per blank-line-separated block.
#[derive(Clone, Copy)]
pub(super) enum DocumentBoundary {
    Line,
    BlankLine,
}

pub(super) struct Args {
    pub input: PathBuf,
    pub output: PathBuf,
    pub stats: PathBuf,
    pub lexicon: PathBuf,
    pub min_count: usize,
    pub min_doc_count: usize,
    pub top_n: Option<usize>,
    pub probability: f64,
    pub review: Option<PathBuf>,
    pub review_examples: usize,
    pub max_phrase_codepoints: usize,
    pub document_boundary: DocumentBoundary,
    pub include_redundant: bool,
    pub include_excluded_stats: bool,
}

pub(super) struct UnigramCandidateArgs {
    pub input: PathBuf,
    pub output: PathBuf,
    pub lexicon: PathBuf,
    pub max_lexicon_phrase_codepoints: usize,
    pub min_count: usize,
    pub min_doc_count: usize,
    pub min_tokens: usize,
    pub max_tokens: usize,
    pub max_candidate_codepoints: usize,
    pub max_multi_token_codepoints: usize,
    pub weight: f64,
    pub source: String,
    pub tags: String,
    pub document_boundary: DocumentBoundary,
}

pub(super) fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args> {
    let mut parsed = Args {
        input: PathBuf::new(),
        output: PathBuf::from("bigrams.tsv"),
        stats: PathBuf::from("bigram-stats.tsv"),
        lexicon: PathBuf::from("normalized/smart-mandarin.tsv"),
        min_count: 2,
        min_doc_count: 1,
        top_n: None,
        probability: -0.1,
        review: None,
        review_examples: 2,
        max_phrase_codepoints: 7,
        document_boundary: DocumentBoundary::Line,
        include_redundant: false,
        include_excluded_stats: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => parsed.input = value_path(&arg, &mut args)?,
            "--output" => parsed.output = value_path(&arg, &mut args)?,
            "--stats" => parsed.stats = value_path(&arg, &mut args)?,
            "--lexicon" => parsed.lexicon = value_path(&arg, &mut args)?,
            "--min-count" => parsed.min_count = value_usize(&arg, &mut args)?,
            "--min-doc-count" => parsed.min_doc_count = value_usize(&arg, &mut args)?,
            "--top-n" => {
                let value = value_usize(&arg, &mut args)?;
                if value == 0 {
                    bail!("--top-n must be at least 1");
                }
                parsed.top_n = Some(value);
            }
            "--probability" => parsed.probability = value_f64(&arg, &mut args)?,
            "--review" => parsed.review = Some(value_path(&arg, &mut args)?),
            "--review-examples" => parsed.review_examples = value_usize(&arg, &mut args)?,
            "--max-phrase-codepoints" => {
                parsed.max_phrase_codepoints = value_usize(&arg, &mut args)?
            }
            "--document-boundary" => {
                parsed.document_boundary = parse_document_boundary(&arg, &mut args)?
            }
            "--include-redundant" => parsed.include_redundant = true,
            "--include-excluded-stats" => parsed.include_excluded_stats = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => bail!("unknown build-bigram-stats option: {arg}"),
        }
    }

    if parsed.input.as_os_str().is_empty() {
        bail!("missing required --input");
    }
    Ok(parsed)
}

pub(super) fn parse_unigram_candidate_args(
    mut args: impl Iterator<Item = String>,
) -> Result<UnigramCandidateArgs> {
    let mut parsed = UnigramCandidateArgs {
        input: PathBuf::new(),
        output: PathBuf::from("unigram-candidates.tsv"),
        lexicon: PathBuf::from("normalized/smart-mandarin.tsv"),
        max_lexicon_phrase_codepoints: 7,
        min_count: 5,
        min_doc_count: 3,
        min_tokens: 2,
        max_tokens: 4,
        max_candidate_codepoints: 7,
        max_multi_token_codepoints: 0,
        weight: -2.4,
        source: "corpus-unigram-candidate".to_string(),
        tags: "unigram,candidate,corpus".to_string(),
        document_boundary: DocumentBoundary::Line,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => parsed.input = value_path(&arg, &mut args)?,
            "--output" => parsed.output = value_path(&arg, &mut args)?,
            "--lexicon" => parsed.lexicon = value_path(&arg, &mut args)?,
            "--max-lexicon-phrase-codepoints" => {
                parsed.max_lexicon_phrase_codepoints = value_usize(&arg, &mut args)?
            }
            "--min-count" => parsed.min_count = value_usize(&arg, &mut args)?,
            "--min-doc-count" => parsed.min_doc_count = value_usize(&arg, &mut args)?,
            "--min-tokens" => parsed.min_tokens = value_usize(&arg, &mut args)?,
            "--max-tokens" => parsed.max_tokens = value_usize(&arg, &mut args)?,
            "--max-candidate-codepoints" => {
                parsed.max_candidate_codepoints = value_usize(&arg, &mut args)?
            }
            "--max-multi-token-codepoints" | "--max-three-token-codepoints" => {
                parsed.max_multi_token_codepoints = value_usize(&arg, &mut args)?
            }
            "--weight" => parsed.weight = value_f64(&arg, &mut args)?,
            "--source" => parsed.source = value(&arg, &mut args)?,
            "--tags" => parsed.tags = value(&arg, &mut args)?,
            "--document-boundary" => {
                parsed.document_boundary = parse_document_boundary(&arg, &mut args)?
            }
            "--help" | "-h" => {
                print_unigram_candidate_help();
                std::process::exit(0);
            }
            _ => bail!("unknown build-unigram-candidates option: {arg}"),
        }
    }

    if parsed.input.as_os_str().is_empty() {
        bail!("missing required --input");
    }
    if parsed.max_lexicon_phrase_codepoints == 0 {
        bail!("--max-lexicon-phrase-codepoints must be at least 1");
    }
    if parsed.min_tokens == 0 {
        bail!("--min-tokens must be at least 1");
    }
    if parsed.max_tokens < parsed.min_tokens {
        bail!("--max-tokens must be greater than or equal to --min-tokens");
    }
    Ok(parsed)
}

fn parse_document_boundary(
    arg: &str,
    args: &mut impl Iterator<Item = String>,
) -> Result<DocumentBoundary> {
    match value(arg, args)?.as_str() {
        "line" => Ok(DocumentBoundary::Line),
        "blank-line" => Ok(DocumentBoundary::BlankLine),
        value => bail!("invalid {arg}: {value}; expected line or blank-line"),
    }
}

fn value_path(arg: &str, args: &mut impl Iterator<Item = String>) -> Result<PathBuf> {
    Ok(PathBuf::from(value(arg, args)?))
}

fn value_usize(arg: &str, args: &mut impl Iterator<Item = String>) -> Result<usize> {
    value(arg, args)?
        .parse()
        .with_context(|| format!("parse {arg}"))
}

fn value_f64(arg: &str, args: &mut impl Iterator<Item = String>) -> Result<f64> {
    value(arg, args)?
        .parse()
        .with_context(|| format!("parse {arg}"))
}

fn value(arg: &str, args: &mut impl Iterator<Item = String>) -> Result<String> {
    args.next()
        .with_context(|| format!("missing value for {arg}"))
}

fn print_help() {
    eprintln!(
        "Usage:\n  cargo run --release -- build-bigram-stats \\\n    --input sentences.txt \\\n    --output bigrams.tsv \\\n    --stats bigram-stats.tsv \\\n    [--review bigram-review.tsv] [--review-examples 2] \\\n    [--lexicon normalized/smart-mandarin.tsv] \\\n    [--min-count 2] [--min-doc-count 1] [--top-n 1000] \\\n    [--document-boundary line|blank-line] \\\n    [--include-redundant] [--include-excluded-stats]"
    );
}

fn print_unigram_candidate_help() {
    eprintln!(
        "Usage:\n  cargo run --release -- build-unigram-candidates \\\n    --input sentences.txt \\\n    --output unigram-candidates.tsv \\\n    [--lexicon normalized/smart-mandarin.tsv] \\\n    [--max-lexicon-phrase-codepoints 7] \\\n    [--min-count 5] [--min-doc-count 3] \\\n    [--min-tokens 2] [--max-tokens 4] \\\n    [--max-candidate-codepoints 7] \\\n    [--max-multi-token-codepoints 4] \\\n    [--document-boundary line|blank-line]"
    );
}
