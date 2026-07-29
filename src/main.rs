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
        Some("bpmf-to-qstring") => run_bpmf_to_qstring(),
        Some("qstring-to-bpmf") => run_qstring_to_bpmf(),
        Some("help") | Some("--help") | Some("-h") | None => {
            print_help();
            Ok(())
        }
        Some(command) => bail!("unknown command: {command}"),
    }
}

fn print_help() {
    eprintln!(
        "Usage:\n  cargo run --release -- fetch-modern-sources\n  cargo run --release -- prepare-release\n  cargo run --release -- build-bigram-stats --input sentences.txt --output bigrams.tsv --stats bigram-stats.tsv --review bigram-review.tsv --top-n 1000\n  cargo run --release -- build-unigram-candidates --input sentences.txt --output unigram-candidates.tsv\n  cargo run --release -- bpmf-to-qstring < bopomofo-lines.txt\n  cargo run --release -- qstring-to-bpmf < qstring-lines.txt"
    );
}

/// Reverse of [`run_bpmf_to_qstring`]: reads one qstring per line from
/// stdin and prints its space-separated bopomofo, or an empty line if it
/// could not be decoded.
fn run_qstring_to_bpmf() -> Result<()> {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        match phonetics::bpmf_for_qstring(&line) {
            Some(bpmf) => writeln!(out, "{bpmf}")?,
            None => writeln!(out)?,
        }
    }
    Ok(())
}

/// Reads one bopomofo sequence per line from stdin (syllables separated by
/// spaces or commas, e.g. "ㄕㄨ ㄖㄨˋ") and prints `<qstring>\t<syllable-count>`
/// per line using the same conversion the release importers use, or an empty
/// line if the sequence could not be parsed. Used by audit tooling that needs
/// to compare external dictionaries' readings against engine qstrings.
fn run_bpmf_to_qstring() -> Result<()> {
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        match phonetics::qstring_for_bpmf_sequence(&line) {
            Some((qstring, count)) => writeln!(out, "{qstring}\t{count}")?,
            None => writeln!(out)?,
        }
    }
    Ok(())
}
