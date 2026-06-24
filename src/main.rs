mod associated_phrases;
mod bpmf_ext;
mod config;
mod db;
mod fetch;
mod files;
mod importers;
mod manifest;
mod module_cin;
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
        "Usage:\n  cargo run --release -- fetch-modern-sources\n  cargo run --release -- prepare-release"
    );
}
