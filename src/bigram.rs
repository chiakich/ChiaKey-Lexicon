use anyhow::{bail, Context, Result};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

#[derive(Clone)]
struct LexiconEntry {
    qstring: String,
    weight: f64,
}

struct Lexicon {
    by_phrase: HashMap<String, LexiconEntry>,
    first_by_qstring: HashMap<String, String>,
    max_phrase_codepoints: usize,
}

#[derive(Default)]
struct BigramCount {
    count: usize,
    doc_count: usize,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct BigramKey {
    previous: String,
    current: String,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct UnigramCandidateKey {
    phrase: String,
    qstring: String,
    tokens: Vec<String>,
}

struct Args {
    input: PathBuf,
    output: PathBuf,
    stats: PathBuf,
    lexicon: PathBuf,
    min_count: usize,
    min_doc_count: usize,
    probability: f64,
    max_phrase_codepoints: usize,
    document_boundary: DocumentBoundary,
    include_redundant: bool,
    include_excluded_stats: bool,
}

struct UnigramCandidateArgs {
    input: PathBuf,
    output: PathBuf,
    lexicon: PathBuf,
    max_lexicon_phrase_codepoints: usize,
    min_count: usize,
    min_doc_count: usize,
    min_tokens: usize,
    max_tokens: usize,
    max_candidate_codepoints: usize,
    max_multi_token_codepoints: usize,
    weight: f64,
    source: String,
    tags: String,
    document_boundary: DocumentBoundary,
}

#[derive(Clone, Copy)]
enum DocumentBoundary {
    Line,
    BlankLine,
}

pub fn run(args: impl Iterator<Item = String>) -> Result<()> {
    let args = parse_args(args)?;
    let lexicon = load_lexicon(&args.lexicon, args.max_phrase_codepoints)?;
    let counts = count_bigrams(&args.input, &lexicon, args.document_boundary)?;
    write_outputs(&args, &lexicon, &counts)
}

pub fn run_unigram_candidates(args: impl Iterator<Item = String>) -> Result<()> {
    let args = parse_unigram_candidate_args(args)?;
    let lexicon = load_lexicon(&args.lexicon, args.max_lexicon_phrase_codepoints)?;
    let counts = count_unigram_candidates(&args.input, &lexicon, &args)?;
    write_unigram_candidate_outputs(&args, &counts)
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args> {
    let mut parsed = Args {
        input: PathBuf::new(),
        output: PathBuf::from("bigrams.tsv"),
        stats: PathBuf::from("bigram-stats.tsv"),
        lexicon: PathBuf::from("normalized/smart-mandarin.tsv"),
        min_count: 2,
        min_doc_count: 1,
        probability: -0.1,
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
            "--probability" => parsed.probability = value_f64(&arg, &mut args)?,
            "--max-phrase-codepoints" => {
                parsed.max_phrase_codepoints = value_usize(&arg, &mut args)?
            }
            "--document-boundary" => {
                parsed.document_boundary = match value(&arg, &mut args)?.as_str() {
                    "line" => DocumentBoundary::Line,
                    "blank-line" => DocumentBoundary::BlankLine,
                    value => {
                        bail!("invalid --document-boundary: {value}; expected line or blank-line")
                    }
                }
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

fn parse_unigram_candidate_args(
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
        "Usage:\n  cargo run --release -- build-bigram-stats \\\n    --input sentences.txt \\\n    --output bigrams.tsv \\\n    --stats bigram-stats.tsv \\\n    [--lexicon normalized/smart-mandarin.tsv] \\\n    [--min-count 2] [--min-doc-count 1] \\\n    [--document-boundary line|blank-line] \\\n    [--include-redundant] [--include-excluded-stats]"
    );
}

fn print_unigram_candidate_help() {
    eprintln!(
        "Usage:\n  cargo run --release -- build-unigram-candidates \\\n    --input sentences.txt \\\n    --output unigram-candidates.tsv \\\n    [--lexicon normalized/smart-mandarin.tsv] \\\n    [--max-lexicon-phrase-codepoints 7] \\\n    [--min-count 5] [--min-doc-count 3] \\\n    [--min-tokens 2] [--max-tokens 4] \\\n    [--max-candidate-codepoints 7] \\\n    [--max-multi-token-codepoints 4] \\\n    [--document-boundary line|blank-line]"
    );
}

fn load_lexicon(path: &PathBuf, max_phrase_codepoints: usize) -> Result<Lexicon> {
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

    let first_by_qstring = by_qstring
        .into_iter()
        .filter_map(|(qstring, mut entries)| {
            entries.sort_by(|a, b| compare_unigram(a, b));
            entries
                .into_iter()
                .next()
                .map(|(phrase, _)| (qstring, phrase))
        })
        .collect::<HashMap<_, _>>();

    Ok(Lexicon {
        by_phrase,
        first_by_qstring,
        max_phrase_codepoints,
    })
}

fn compare_unigram(a: &(String, f64), b: &(String, f64)) -> Ordering {
    b.1.partial_cmp(&a.1)
        .unwrap_or(Ordering::Equal)
        .then_with(|| a.0.cmp(&b.0))
}

fn is_special_qstring(qstring: &str) -> bool {
    qstring.starts_with("_punctuation") || qstring.starts_with("_ctrl")
}

fn count_bigrams(
    path: &PathBuf,
    lexicon: &Lexicon,
    document_boundary: DocumentBoundary,
) -> Result<HashMap<BigramKey, BigramCount>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut counts = HashMap::<BigramKey, BigramCount>::new();
    let mut seen_in_doc = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        if matches!(document_boundary, DocumentBoundary::BlankLine) && line.trim().is_empty() {
            flush_doc_counts(&mut counts, &mut seen_in_doc);
            continue;
        }

        count_line_bigrams(&line, lexicon, &mut counts, &mut seen_in_doc);

        if matches!(document_boundary, DocumentBoundary::Line) {
            flush_doc_counts(&mut counts, &mut seen_in_doc);
        }
    }
    flush_doc_counts(&mut counts, &mut seen_in_doc);

    Ok(counts)
}

fn count_unigram_candidates(
    path: &PathBuf,
    lexicon: &Lexicon,
    args: &UnigramCandidateArgs,
) -> Result<HashMap<UnigramCandidateKey, BigramCount>> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut counts = HashMap::<UnigramCandidateKey, BigramCount>::new();
    let mut seen_in_doc = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        if matches!(args.document_boundary, DocumentBoundary::BlankLine) && line.trim().is_empty() {
            flush_candidate_doc_counts(&mut counts, &mut seen_in_doc);
            continue;
        }

        count_line_unigram_candidates(&line, lexicon, args, &mut counts, &mut seen_in_doc);

        if matches!(args.document_boundary, DocumentBoundary::Line) {
            flush_candidate_doc_counts(&mut counts, &mut seen_in_doc);
        }
    }
    flush_candidate_doc_counts(&mut counts, &mut seen_in_doc);

    Ok(counts)
}

fn count_line_bigrams(
    line: &str,
    lexicon: &Lexicon,
    counts: &mut HashMap<BigramKey, BigramCount>,
    seen_in_doc: &mut HashSet<BigramKey>,
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
            seen_in_doc.insert(key);
        }
    }
}

fn count_line_unigram_candidates(
    line: &str,
    lexicon: &Lexicon,
    args: &UnigramCandidateArgs,
    counts: &mut HashMap<UnigramCandidateKey, BigramCount>,
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

fn flush_doc_counts(
    counts: &mut HashMap<BigramKey, BigramCount>,
    seen_in_doc: &mut HashSet<BigramKey>,
) {
    for key in seen_in_doc.drain() {
        counts.entry(key).or_default().doc_count += 1;
    }
}

fn flush_candidate_doc_counts(
    counts: &mut HashMap<UnigramCandidateKey, BigramCount>,
    seen_in_doc: &mut HashSet<UnigramCandidateKey>,
) {
    for key in seen_in_doc.drain() {
        counts.entry(key).or_default().doc_count += 1;
    }
}

fn qstring_for_tokens(tokens: &[String], lexicon: &Lexicon) -> Option<String> {
    let mut qstring = String::new();
    for token in tokens {
        qstring.push_str(&lexicon.by_phrase.get(token)?.qstring);
    }
    Some(qstring)
}

fn han_sentences(line: &str) -> Vec<String> {
    if should_skip_line(line) {
        return Vec::new();
    }

    let mut sentences = Vec::new();
    let mut current = String::new();
    for character in line.chars() {
        if is_han(character) {
            current.push(character);
        } else if !current.is_empty() {
            if current.chars().count() >= 2 {
                sentences.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if current.chars().count() >= 2 {
        sentences.push(current);
    }
    sentences
}

fn should_skip_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty()
        || trimmed.starts_with("作者")
        || trimmed.starts_with("標題")
        || trimmed.starts_with("時間")
        || trimmed.starts_with("看板")
        || trimmed.starts_with("※")
        || trimmed.starts_with("--")
        || trimmed.contains("http://")
        || trimmed.contains("https://")
}

fn tokenize_sentence(sentence: &str, lexicon: &Lexicon) -> Vec<String> {
    let characters = sentence.chars().collect::<Vec<_>>();
    let mut scores = vec![f64::NEG_INFINITY; characters.len() + 1];
    let mut next = vec![None; characters.len() + 1];
    scores[characters.len()] = 0.0;

    for index in (0..characters.len()).rev() {
        let max_len = lexicon.max_phrase_codepoints.min(characters.len() - index);

        for length in 1..=max_len {
            let candidate = characters[index..index + length].iter().collect::<String>();
            let Some(entry) = lexicon.by_phrase.get(&candidate) else {
                continue;
            };
            let score = entry.weight + scores[index + length];
            if score > scores[index]
                || (score == scores[index]
                    && next[index].as_ref().is_some_and(|(_, best)| length > *best))
            {
                scores[index] = score;
                next[index] = Some((candidate, length));
            }
        }
    }

    let mut tokens = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        if let Some((token, length)) = &next[index] {
            tokens.push(token.clone());
            index += length;
        } else {
            index += 1;
        }
    }

    tokens
}

fn write_outputs(
    args: &Args,
    lexicon: &Lexicon,
    counts: &HashMap<BigramKey, BigramCount>,
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
        "previous\tcurrent\tcount\tdoc_count\tredundant\texcluded_particle\texcluded_single_char_pair\texcluded_joined_unigram\tprevious_qstring\tcurrent_qstring"
    )?;

    let mut emitted = 0_usize;
    let mut redundant = 0_usize;
    let mut excluded_particle = 0_usize;
    let mut excluded_single_char_pair = 0_usize;
    let mut excluded_joined_unigram = 0_usize;
    for (key, count) in rows {
        let Some(previous) = lexicon.by_phrase.get(&key.previous) else {
            continue;
        };
        let Some(current) = lexicon.by_phrase.get(&key.current) else {
            continue;
        };
        let is_redundant =
            is_redundant_pair(lexicon, &key.previous, previous, &key.current, current);
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
        let has_joined_unigram = has_joined_unigram(lexicon, &key.previous, &key.current);
        if has_joined_unigram {
            excluded_joined_unigram += 1;
        }

        let should_emit = count.count >= args.min_count
            && count.doc_count >= args.min_doc_count
            && (!is_redundant || args.include_redundant)
            && !has_excluded_particle
            && !is_single_char_pair
            && !has_joined_unigram;

        if should_emit || args.include_excluded_stats {
            writeln!(
                stats,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                key.previous,
                key.current,
                count.count,
                count.doc_count,
                is_redundant,
                has_excluded_particle,
                is_single_char_pair,
                has_joined_unigram,
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
        emitted += 1;
    }

    eprintln!(
        "bigram stats: pairs={} redundant={} excluded_particle={} excluded_single_char_pair={} excluded_joined_unigram={} emitted={} min_count={} min_doc_count={}",
        counts.len(),
        redundant,
        excluded_particle,
        excluded_single_char_pair,
        excluded_joined_unigram,
        emitted,
        args.min_count,
        args.min_doc_count
    );

    Ok(())
}

fn write_unigram_candidate_outputs(
    args: &UnigramCandidateArgs,
    counts: &HashMap<UnigramCandidateKey, BigramCount>,
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

fn is_redundant_pair(
    lexicon: &Lexicon,
    previous_phrase: &str,
    previous: &LexiconEntry,
    current_phrase: &str,
    current: &LexiconEntry,
) -> bool {
    lexicon
        .first_by_qstring
        .get(&previous.qstring)
        .is_some_and(|phrase| phrase == previous_phrase)
        && lexicon
            .first_by_qstring
            .get(&current.qstring)
            .is_some_and(|phrase| phrase == current_phrase)
}

fn contains_excluded_particle(phrase: &str) -> bool {
    phrase.contains('的')
        || phrase == "在"
        || phrase == "為"
        || phrase == "個"
        || phrase == "了"
        || phrase == "任"
        || phrase == "地"
}

fn is_single_char_pair(previous: &str, current: &str) -> bool {
    previous.chars().count() == 1 && current.chars().count() == 1
}

fn has_joined_unigram(lexicon: &Lexicon, previous: &str, current: &str) -> bool {
    let mut joined = String::with_capacity(previous.len() + current.len());
    joined.push_str(previous);
    joined.push_str(current);
    lexicon.by_phrase.contains_key(&joined)
}

fn is_han(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_han_sentences_and_skips_metadata() {
        assert_eq!(han_sentences("作者 abc (測試)"), Vec::<String>::new());
        assert_eq!(han_sentences("今天，天氣很好！"), vec!["今天", "天氣很好"]);
    }

    #[test]
    fn tokenizes_with_longest_match() {
        let mut by_phrase = HashMap::new();
        by_phrase.insert(
            "程式".to_string(),
            LexiconEntry {
                qstring: "a".to_string(),
                weight: 0.0,
            },
        );
        by_phrase.insert(
            "語言".to_string(),
            LexiconEntry {
                qstring: "b".to_string(),
                weight: 0.0,
            },
        );
        by_phrase.insert(
            "程式語言".to_string(),
            LexiconEntry {
                qstring: "ab".to_string(),
                weight: 0.0,
            },
        );
        let lexicon = Lexicon {
            by_phrase,
            first_by_qstring: HashMap::new(),
            max_phrase_codepoints: 4,
        };
        assert_eq!(tokenize_sentence("程式語言", &lexicon), vec!["程式語言"]);
    }

    #[test]
    fn tokenizes_with_best_weighted_path() {
        let mut by_phrase = HashMap::new();
        by_phrase.insert(
            "還以".to_string(),
            LexiconEntry {
                qstring: "a".to_string(),
                weight: -2.0,
            },
        );
        by_phrase.insert(
            "還".to_string(),
            LexiconEntry {
                qstring: "b".to_string(),
                weight: -0.5,
            },
        );
        by_phrase.insert(
            "以為".to_string(),
            LexiconEntry {
                qstring: "c".to_string(),
                weight: -0.5,
            },
        );
        by_phrase.insert(
            "為".to_string(),
            LexiconEntry {
                qstring: "d".to_string(),
                weight: -2.0,
            },
        );
        let lexicon = Lexicon {
            by_phrase,
            first_by_qstring: HashMap::new(),
            max_phrase_codepoints: 4,
        };
        assert_eq!(tokenize_sentence("還以為", &lexicon), vec!["還", "以為"]);
    }

    #[test]
    fn skips_special_qstrings_when_loading_lexicon() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "chiakey-bigram-test-{}.tsv",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(
            &path,
            "_punctuation_list\t十\t0.0\ttest\n_5\t拍\t-0.7\ttest\np?\t十\t-0.7\ttest\n",
        )
        .unwrap();

        let lexicon = load_lexicon(&path, 7).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(lexicon.by_phrase.get("十").unwrap().qstring, "p?");
        assert_eq!(lexicon.by_phrase.get("拍").unwrap().qstring, "_5");
    }

    #[test]
    fn detects_excluded_de_particle_inside_bigram_terms() {
        assert!(contains_excluded_particle("的"));
        assert!(contains_excluded_particle("真的"));
        assert!(contains_excluded_particle("在"));
        assert!(contains_excluded_particle("為"));
        assert!(contains_excluded_particle("個"));
        assert!(contains_excluded_particle("了"));
        assert!(contains_excluded_particle("任"));
        assert!(contains_excluded_particle("地"));
        assert!(!contains_excluded_particle("現在"));
        assert!(!contains_excluded_particle("存在"));
        assert!(!contains_excluded_particle("成為"));
        assert!(!contains_excluded_particle("認為"));
        assert!(!contains_excluded_particle("個人"));
        assert!(!contains_excluded_particle("那個"));
        assert!(!contains_excluded_particle("了解"));
        assert!(!contains_excluded_particle("任命"));
        assert!(!contains_excluded_particle("在地"));
        assert!(!contains_excluded_particle("地點"));
        assert!(!contains_excluded_particle("台灣"));
    }

    #[test]
    fn detects_single_character_pairs() {
        assert!(is_single_char_pair("台", "積"));
        assert!(!is_single_char_pair("台灣", "人"));
        assert!(!is_single_char_pair("我", "覺得"));
    }

    #[test]
    fn detects_joined_unigram_pairs() {
        let mut by_phrase = HashMap::new();
        by_phrase.insert(
            "下".to_string(),
            LexiconEntry {
                qstring: "L`".to_string(),
                weight: -1.0,
            },
        );
        by_phrase.insert(
            "意識".to_string(),
            LexiconEntry {
                qstring: "5_0_".to_string(),
                weight: -1.0,
            },
        );
        by_phrase.insert(
            "下意識".to_string(),
            LexiconEntry {
                qstring: "L`5_0_".to_string(),
                weight: -2.0,
            },
        );
        let mut lexicon = Lexicon {
            by_phrase,
            first_by_qstring: HashMap::new(),
            max_phrase_codepoints: 4,
        };
        lexicon.first_by_qstring.insert("L`".into(), "下".into());
        lexicon
            .first_by_qstring
            .insert("5_0_".into(), "意識".into());

        assert!(has_joined_unigram(&lexicon, "下", "意識"));
        assert!(!has_joined_unigram(&lexicon, "意識", "下"));
    }

    #[test]
    fn counts_missing_joined_unigram_candidates() {
        let mut by_phrase = HashMap::new();
        by_phrase.insert(
            "塞克".to_string(),
            LexiconEntry {
                qstring: "a".to_string(),
                weight: 0.0,
            },
        );
        by_phrase.insert(
            "斯".to_string(),
            LexiconEntry {
                qstring: "b".to_string(),
                weight: 0.0,
            },
        );
        by_phrase.insert(
            "在".to_string(),
            LexiconEntry {
                qstring: "c".to_string(),
                weight: 0.0,
            },
        );
        by_phrase.insert(
            "美國".to_string(),
            LexiconEntry {
                qstring: "d".to_string(),
                weight: 0.0,
            },
        );
        let lexicon = Lexicon {
            by_phrase,
            first_by_qstring: HashMap::new(),
            max_phrase_codepoints: 4,
        };
        let args = UnigramCandidateArgs {
            input: PathBuf::new(),
            output: PathBuf::new(),
            lexicon: PathBuf::new(),
            max_lexicon_phrase_codepoints: 7,
            min_count: 1,
            min_doc_count: 1,
            min_tokens: 2,
            max_tokens: 2,
            max_candidate_codepoints: 4,
            max_multi_token_codepoints: 0,
            weight: -2.4,
            source: "test".to_string(),
            tags: "test".to_string(),
            document_boundary: DocumentBoundary::Line,
        };
        let mut counts = HashMap::new();
        let mut seen_in_doc = HashSet::new();

        count_line_unigram_candidates("塞克斯", &lexicon, &args, &mut counts, &mut seen_in_doc);
        count_line_unigram_candidates("在美國", &lexicon, &args, &mut counts, &mut seen_in_doc);
        flush_candidate_doc_counts(&mut counts, &mut seen_in_doc);

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
        let mut by_phrase = HashMap::new();
        for (phrase, qstring) in [
            ("鬼", "a"),
            ("滅", "b"),
            ("刃", "c"),
            ("布林", "d"),
            ("什", "e"),
            ("維克", "f"),
            ("專屬", "g"),
            ("福利", "h"),
            ("與", "i"),
            ("優惠", "j"),
        ] {
            by_phrase.insert(
                phrase.to_string(),
                LexiconEntry {
                    qstring: qstring.to_string(),
                    weight: 0.0,
                },
            );
        }
        let lexicon = Lexicon {
            by_phrase,
            first_by_qstring: HashMap::new(),
            max_phrase_codepoints: 4,
        };
        let args = UnigramCandidateArgs {
            input: PathBuf::new(),
            output: PathBuf::new(),
            lexicon: PathBuf::new(),
            max_lexicon_phrase_codepoints: 7,
            min_count: 1,
            min_doc_count: 1,
            min_tokens: 3,
            max_tokens: 4,
            max_candidate_codepoints: 7,
            max_multi_token_codepoints: 4,
            weight: -2.4,
            source: "test".to_string(),
            tags: "test".to_string(),
            document_boundary: DocumentBoundary::Line,
        };
        let mut counts = HashMap::new();
        let mut seen_in_doc = HashSet::new();

        count_line_unigram_candidates("鬼滅刃", &lexicon, &args, &mut counts, &mut seen_in_doc);
        count_line_unigram_candidates("布林什維克", &lexicon, &args, &mut counts, &mut seen_in_doc);
        count_line_unigram_candidates(
            "專屬福利與優惠",
            &lexicon,
            &args,
            &mut counts,
            &mut seen_in_doc,
        );
        flush_candidate_doc_counts(&mut counts, &mut seen_in_doc);

        assert_eq!(counts.len(), 1);
        assert_eq!(counts.keys().next().unwrap().phrase, "鬼滅刃");
    }
}
