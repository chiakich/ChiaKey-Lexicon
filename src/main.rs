mod associated_phrases;
mod bigram;
mod bpmf_ext;
mod config;
mod db;
mod fetch;
mod files;
mod importers;
mod manifest;
mod module_cin;
mod opencc;
mod paths;
mod phonetics;
mod prepopulated;
mod punctuations;
mod release;
mod types;

use anyhow::{bail, Result};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("build-bigram-stats") => bigram::run(args),
        Some("build-unigram-candidates") => bigram::run_unigram_candidates(args),
        Some("fetch-modern-sources") => fetch::run(),
        Some("prepare-release") => release::run(),
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => bail!("unknown command: {command}"),
    }
}

fn print_help() {
    eprintln!(
        "Usage:\n  cargo run --release -- fetch-modern-sources\n  cargo run --release -- prepare-release\n  cargo run --release -- build-bigram-stats --input sentences.txt --output bigrams.tsv --stats bigram-stats.tsv --review bigram-review.tsv --top-n 1000\n  cargo run --release -- build-unigram-candidates --input sentences.txt --output unigram-candidates.tsv"
    );
}
