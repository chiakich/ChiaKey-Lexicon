use std::{
    collections::{HashMap, HashSet},
    env,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
};

// Mimics the walker's preference for longer words (see ChiaKey Node.cpp +1.0/syllable).
const LENGTH_BONUS: f64 = 1.0;
const UNKNOWN_CHAR_PENALTY: f64 = -9.0;
const MAX_WORD_CHARS: usize = 8;

struct Lexicon {
    // phrase -> best (highest) weight across readings
    words: HashMap<String, f64>,
    max_len: usize,
}

fn load_lexicon(path: &str) -> io::Result<Lexicon> {
    let reader = BufReader::new(File::open(path)?);
    let mut words = HashMap::<String, f64>::new();
    let mut max_len = 1;
    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let _code = fields.next();
        let phrase = match fields.next() {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };
        let weight: f64 = match fields.next().and_then(|w| w.parse().ok()) {
            Some(w) => w,
            None => continue,
        };
        let chars = phrase.chars().count();
        if chars == 0 || chars > MAX_WORD_CHARS || !phrase.chars().all(is_han) {
            continue;
        }
        max_len = max_len.max(chars);
        words
            .entry(phrase.to_string())
            .and_modify(|w| {
                if weight > *w {
                    *w = weight;
                }
            })
            .or_insert(weight);
    }
    Ok(Lexicon { words, max_len })
}

fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

// Viterbi max-score segmentation over one Han-only run.
//
// The length bonus is per EXTRA character, matching Node::lengthPrior() in the
// engine. A bonus per character would sum to the run length whatever the
// segmentation and so express no preference at all -- which is what this used
// to do, leaving the extracted word boundaries misaligned with the walker's.
fn segment(run: &[char], lexicon: &Lexicon) -> Vec<String> {
    let n = run.len();
    if n == 0 {
        return Vec::new();
    }
    // best[i] = (score up to i, backpointer word length)
    let mut best = vec![(f64::NEG_INFINITY, 0usize); n + 1];
    best[0] = (0.0, 0);
    let mut buffer = String::new();
    for i in 0..n {
        let (base, _) = best[i];
        if base == f64::NEG_INFINITY {
            continue;
        }
        let limit = lexicon.max_len.min(n - i);
        for len in 1..=limit {
            buffer.clear();
            buffer.extend(&run[i..i + len]);
            let score = match lexicon.words.get(buffer.as_str()) {
                Some(w) => w + LENGTH_BONUS * (len - 1) as f64,
                None if len == 1 => UNKNOWN_CHAR_PENALTY,
                None => continue,
            };
            let candidate = base + score;
            if candidate > best[i + len].0 {
                best[i + len] = (candidate, len);
            }
        }
    }
    let mut words = Vec::new();
    let mut pos = n;
    while pos > 0 {
        let len = best[pos].1;
        if len == 0 {
            // unreachable in practice: single chars always score
            break;
        }
        words.push(run[pos - len..pos].iter().collect::<String>());
        pos -= len;
    }
    words.reverse();
    words
}

fn extract(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut out_path = None;
    let mut min_count = 1u64;
    let mut corpora = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = iter.next().cloned(),
            "--out" => out_path = iter.next().cloned(),
            "--min-count" => min_count = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            other => corpora.push(other.to_string()),
        }
    }
    let lexicon = load_lexicon(&lexicon_path.ok_or("--lexicon required")?)?;
    let out_path = out_path.ok_or("--out required")?;
    eprintln!("lexicon words: {}", lexicon.words.len());

    // (prev, cur) -> (count, docfreq)
    let mut pairs = HashMap::<(String, String), (u64, u64)>::new();
    let mut seen_this_doc = HashSet::<(String, String)>::new();
    let mut documents = 0u64;
    let mut tokens = 0u64;
    for corpus in &corpora {
        let reader = BufReader::new(File::open(corpus)?);
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed == "#####" {
                continue;
            }
            documents += 1;
            seen_this_doc.clear();
            // split into Han runs; anything non-Han is a boundary
            let mut run = Vec::<char>::new();
            let flush = |run: &mut Vec<char>,
                             pairs: &mut HashMap<(String, String), (u64, u64)>,
                             seen: &mut HashSet<(String, String)>,
                             tokens: &mut u64| {
                if run.is_empty() {
                    return;
                }
                let words = segment(run, &lexicon);
                *tokens += words.len() as u64;
                for window in words.windows(2) {
                    let key = (window[0].clone(), window[1].clone());
                    let entry = pairs.entry(key.clone()).or_insert((0, 0));
                    entry.0 += 1;
                    if seen.insert(key) {
                        entry.1 += 1;
                    }
                }
                run.clear();
            };
            for c in trimmed.chars() {
                if is_han(c) {
                    run.push(c);
                } else {
                    flush(&mut run, &mut pairs, &mut seen_this_doc, &mut tokens);
                }
            }
            flush(&mut run, &mut pairs, &mut seen_this_doc, &mut tokens);
        }
    }

    let mut rows: Vec<_> = pairs
        .into_iter()
        .filter(|(_, (count, _))| *count >= min_count)
        .collect();
    rows.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(&b.0)));
    let mut out = BufWriter::new(File::create(&out_path)?);
    writeln!(out, "# previous\tcurrent\tcount\tdocfreq")?;
    let mut compound = 0u64;
    for ((prev, cur), (count, docfreq)) in &rows {
        // placement rule: prev+cur forming one lexicon word belongs to unigrams
        let mut joined = prev.clone();
        joined.push_str(cur);
        let is_word = lexicon.words.contains_key(joined.as_str());
        if is_word {
            compound += 1;
        }
        writeln!(
            out,
            "{prev}\t{cur}\t{count}\t{docfreq}{}",
            if is_word { "\tcompound" } else { "" }
        )?;
    }
    eprintln!(
        "documents={documents} tokens={tokens} pairs={} compound_pairs={compound}",
        rows.len()
    );
    Ok(())
}

fn fnv1a(text: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Clone, Copy, PartialEq)]
enum Split {
    Dev,
    Test,
    All,
}

fn pair_split(prev: &str, cur: &str) -> Split {
    if fnv1a(&format!("{prev}\t{cur}")) % 2 == 0 {
        Split::Dev
    } else {
        Split::Test
    }
}

fn coverage(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut target_path = None;
    let mut gaps_path = None;
    let mut split = Split::Dev;
    let mut extracted = Vec::new(); // cols 1,2 (prev, cur)
    let mut overlays = Vec::new(); // cols 2,3 (prev, cur), '#' header
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = iter.next().cloned(),
            "--target" => target_path = iter.next().cloned(),
            "--out-gaps" => gaps_path = iter.next().cloned(),
            "--extracted" => extracted.push(iter.next().cloned().ok_or("missing value")?),
            "--overlay" => overlays.push(iter.next().cloned().ok_or("missing value")?),
            "--split" => {
                split = match iter.next().map(String::as_str) {
                    Some("dev") => Split::Dev,
                    Some("test") => Split::Test,
                    Some("all") => Split::All,
                    other => return Err(format!("bad --split {other:?}").into()),
                }
            }
            other => return Err(format!("unexpected coverage argument: {other}").into()),
        }
    }
    let lexicon = load_lexicon(&lexicon_path.ok_or("--lexicon required")?)?;

    let mut observed = HashSet::<(String, String)>::new();
    for path in &extracted {
        let reader = BufReader::new(File::open(path)?);
        for line in reader.lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            if let (Some(prev), Some(cur)) = (fields.next(), fields.next()) {
                observed.insert((prev.to_string(), cur.to_string()));
            }
        }
    }
    let mut overlay_pairs = HashSet::<(String, String)>::new();
    for path in &overlays {
        let reader = BufReader::new(File::open(path)?);
        for line in reader.lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }
            let mut fields = line.split('\t');
            let _q = fields.next();
            if let (Some(prev), Some(cur)) = (fields.next(), fields.next()) {
                overlay_pairs.insert((prev.to_string(), cur.to_string()));
            }
        }
    }

    // target rows: qstring prev cur prob
    let reader = BufReader::new(File::open(target_path.ok_or("--target required")?)?);
    let mut total = 0u64;
    let mut boundary = 0u64;
    let mut compound = 0u64;
    let mut hit_corpus = 0u64;
    let mut hit_overlay = 0u64;
    let mut hit_any = 0u64;
    let mut weight_total = 0.0f64;
    let mut weight_hit = 0.0f64;
    let mut gaps = Vec::<(String, String, f64)>::new();
    let mut oov = 0u64;
    for line in reader.lines() {
        let line = line?;
        let mut fields = line.split('\t');
        let _q = fields.next();
        let prev = fields.next().unwrap_or("").to_string();
        let cur = fields.next().unwrap_or("").to_string();
        let prob: f64 = fields.next().and_then(|p| p.parse().ok()).unwrap_or(-99.0);
        if prev.is_empty() || cur.is_empty() {
            boundary += 1;
            continue;
        }
        if split != Split::All && pair_split(&prev, &cur) != split {
            continue;
        }
        total += 1;
        let joined = format!("{prev}{cur}");
        if lexicon.words.contains_key(joined.as_str()) {
            compound += 1;
            continue; // handled by unigram path in ChiaKey
        }
        let key = (prev.clone(), cur.clone());
        let weight = prob.exp();
        weight_total += weight;
        let in_corpus = observed.contains(&key);
        let in_overlay = overlay_pairs.contains(&key);
        if in_corpus {
            hit_corpus += 1;
        }
        if in_overlay {
            hit_overlay += 1;
        }
        if in_corpus || in_overlay {
            hit_any += 1;
            weight_hit += weight;
        } else {
            // gap importance: joint proxy P(prev) * P(cur|prev); both words must be
            // in the current lexicon or the pair can never ship in an overlay.
            if let (Some(pw), Some(_)) = (lexicon.words.get(&prev), lexicon.words.get(&cur)) {
                gaps.push((prev.clone(), cur.clone(), pw + prob));
            } else {
                oov += 1;
            }
        }
    }
    let effective = total - compound;
    println!("split_rows\t{total}");
    println!("boundary_rows_skipped\t{boundary}");
    println!("compound_rows_unigram_path\t{compound}");
    println!("effective_pairs\t{effective}");
    println!(
        "recall_corpus\t{:.4}",
        hit_corpus as f64 / effective.max(1) as f64
    );
    println!(
        "recall_overlay\t{:.4}",
        hit_overlay as f64 / effective.max(1) as f64
    );
    println!(
        "recall_combined\t{:.4}",
        hit_any as f64 / effective.max(1) as f64
    );
    println!(
        "weighted_recall_combined\t{:.4}",
        weight_hit / weight_total.max(f64::MIN_POSITIVE)
    );
    println!("gap_oov_skipped\t{oov}");
    if let Some(path) = gaps_path {
        gaps.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = BufWriter::new(File::create(&path)?);
        writeln!(out, "# previous\tcurrent\tjoint_score")?;
        let mut written = std::collections::HashSet::new();
        for (prev, cur, prob) in &gaps {
            if written.insert((prev.clone(), cur.clone())) {
                writeln!(out, "{prev}\t{cur}\t{prob:.4}")?;
            }
        }
        eprintln!("gaps written: {}", gaps.len());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("extract") => extract(&args[1..]),
        Some("coverage") => coverage(&args[1..]),
        Some("collision") => collision(&args[1..]),
        Some("candidates") => candidates(&args[1..]),
        Some("evaluate") => evaluate(&args[1..]),
        Some("emit") => emit(&args[1..]),
        _ => {
            eprintln!(
                "Usage:\n  extract --lexicon L --out OUT [--min-count N] CORPUS...\n  coverage --lexicon L --target T [--extracted F]... [--overlay F]... [--split dev|test|all] [--out-gaps G]"
            );
            Ok(())
        }
    }
}

// ---- collision analysis ----------------------------------------------------
// A bigram only earns its place when `current` loses the unigram race at its own
// code: otherwise the walker already picks it and the row is dead weight.

struct CodeIndex {
    // phrase -> its codes
    phrase_codes: HashMap<String, Vec<String>>,
    // code -> phrases sharing it
    code_phrases: HashMap<String, Vec<String>>,
    // code -> (best weight, count of candidates)
    code_best: HashMap<String, (f64, u64)>,
    // (code, phrase) -> weight
    entry_weight: HashMap<(String, String), f64>,
}

fn load_code_index(path: &str) -> io::Result<CodeIndex> {
    let reader = BufReader::new(File::open(path)?);
    let mut phrase_codes = HashMap::<String, Vec<String>>::new();
    let mut code_best = HashMap::<String, (f64, u64)>::new();
    let mut entry_weight = HashMap::<(String, String), f64>::new();
    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }
        let mut fields = line.split('\t');
        let code = match fields.next() {
            Some(c) if !c.is_empty() => c,
            _ => continue,
        };
        let phrase = match fields.next() {
            Some(p) if !p.is_empty() => p,
            _ => continue,
        };
        let weight: f64 = match fields.next().and_then(|w| w.parse().ok()) {
            Some(w) => w,
            None => continue,
        };
        if !phrase.chars().all(is_han) {
            continue;
        }
        phrase_codes
            .entry(phrase.to_string())
            .or_default()
            .push(code.to_string());
        let slot = code_best.entry(code.to_string()).or_insert((f64::MIN, 0));
        slot.1 += 1;
        if weight > slot.0 {
            slot.0 = weight;
        }
        entry_weight
            .entry((code.to_string(), phrase.to_string()))
            .and_modify(|w| {
                if weight > *w {
                    *w = weight
                }
            })
            .or_insert(weight);
    }
    let mut code_phrases = HashMap::<String, Vec<String>>::new();
    for (code, phrase) in entry_weight.keys() {
        code_phrases
            .entry(code.clone())
            .or_default()
            .push(phrase.clone());
    }
    Ok(CodeIndex {
        phrase_codes,
        code_phrases,
        code_best,
        entry_weight,
    })
}

// Verdict for one candidate `current` word.
enum Verdict {
    NotInDict,
    NoCompetitor,  // unique at its code: unigram already wins
    AlreadyTop,    // has rivals but is the best: unigram already wins
    Contested(f64) // loses by this margin: a bigram can actually change the result
}

fn verdict(index: &CodeIndex, phrase: &str) -> Verdict {
    let Some((code, own)) = primary_code(index, phrase) else {
        return Verdict::NotInDict;
    };
    let Some(&(best, count)) = index.code_best.get(&code) else {
        return Verdict::NotInDict;
    };
    if count <= 1 {
        Verdict::NoCompetitor
    } else if own >= best {
        Verdict::AlreadyTop
    } else {
        Verdict::Contested(best - own)
    }
}

fn collision(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut input = None;
    let mut cur_column = 1usize;
    let mut min_docfreq = 0u64;
    let mut docfreq_column: Option<usize> = None;
    let mut out_path = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = iter.next().cloned(),
            "--input" => input = iter.next().cloned(),
            "--cur-column" => cur_column = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--docfreq-column" => docfreq_column = iter.next().and_then(|v| v.parse().ok()),
            "--min-docfreq" => min_docfreq = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--out" => out_path = iter.next().cloned(),
            other => return Err(format!("unexpected collision argument: {other}").into()),
        }
    }
    let index = load_code_index(&lexicon_path.ok_or("--lexicon required")?)?;
    let reader = BufReader::new(File::open(input.ok_or("--input required")?)?);
    let mut out = match out_path {
        Some(path) => Some(BufWriter::new(File::create(path)?)),
        None => None,
    };
    if let Some(handle) = out.as_mut() {
        writeln!(handle, "# previous\tcurrent\tmargin")?;
    }
    let (mut rows, mut not_in_dict, mut no_rival, mut already_top, mut contested) = (0, 0, 0, 0, 0);
    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() <= cur_column {
            continue;
        }
        if let (Some(column), true) = (docfreq_column, min_docfreq > 0) {
            let value: u64 = fields.get(column).and_then(|v| v.parse().ok()).unwrap_or(0);
            if value < min_docfreq {
                continue;
            }
        }
        let prev = fields.get(cur_column - 1).copied().unwrap_or("");
        let cur = fields[cur_column];
        if cur.is_empty() {
            continue;
        }
        rows += 1;
        match verdict(&index, cur) {
            Verdict::NotInDict => not_in_dict += 1,
            Verdict::NoCompetitor => no_rival += 1,
            Verdict::AlreadyTop => already_top += 1,
            Verdict::Contested(margin) => {
                contested += 1;
                if let Some(handle) = out.as_mut() {
                    writeln!(handle, "{prev}\t{cur}\t{margin:.4}")?;
                }
            }
        }
    }
    println!("rows\t{rows}");
    println!("current_not_in_dict\t{not_in_dict}");
    println!("no_competitor_at_code\t{no_rival}");
    println!("already_top_at_code\t{already_top}");
    println!("contested_useful\t{contested}");
    println!(
        "useful_ratio\t{:.4}",
        contested as f64 / rows.max(1) as f64
    );
    Ok(())
}

// Turn elicitation output ("target<TAB>prev1、prev2、…") into verifiable
// (prev, current, rival) triples, dropping anything the runtime cannot use.
fn candidates(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut out_path = None;
    let mut inputs = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = iter.next().cloned(),
            "--out" => out_path = iter.next().cloned(),
            other => inputs.push(other.to_string()),
        }
    }
    let lexicon_path = lexicon_path.ok_or("--lexicon required")?;
    let index = load_code_index(&lexicon_path)?;
    let lexicon = load_lexicon(&lexicon_path)?;
    let mut out = BufWriter::new(File::create(out_path.ok_or("--out required")?)?);
    writeln!(out, "# previous\tcurrent\trival")?;
    let mut seen = HashSet::<(String, String)>::new();
    let (mut emitted, mut dropped) = (0u64, 0u64);
    for input in &inputs {
        let reader = BufReader::new(File::open(input)?);
        for line in reader.lines() {
            let line = line?;
            let mut fields = line.trim().splitn(2, '\t');
            // the model sometimes merges two targets ("一頁、一葉"); keep the first
            let target = fields
                .next()
                .unwrap_or("")
                .trim()
                .split(['、', ',', '，'])
                .next()
                .unwrap_or("")
                .trim();
            let list = match fields.next() {
                Some(value) => value,
                None => continue,
            };
            if target.is_empty() || !target.chars().all(is_han) {
                continue;
            }
            // rival = the entry that currently beats `target` at its own code
            let rival = match best_rival(&index, target) {
                Some(rival) => rival,
                None => continue,
            };
            for prev in list.split(['、', ',', '，', ' ', '\t']) {
                let prev = prev.trim();
                if prev.is_empty() || !prev.chars().all(is_han) {
                    continue;
                }
                if prev == target || !lexicon.words.contains_key(prev) {
                    dropped += 1;
                    continue;
                }
                // a pair that is itself a word belongs in unigrams, not bigrams
                if lexicon.words.contains_key(&format!("{prev}{target}")) {
                    dropped += 1;
                    continue;
                }
                if !seen.insert((prev.to_string(), target.to_string())) {
                    continue;
                }
                writeln!(out, "{prev}\t{target}\t{rival}")?;
                emitted += 1;
            }
        }
    }
    eprintln!("candidates emitted={emitted} dropped={dropped}");
    Ok(())
}

fn best_rival(index: &CodeIndex, phrase: &str) -> Option<String> {
    let codes = index.phrase_codes.get(phrase)?;
    let mut best: Option<(f64, String)> = None;
    for code in codes {
        let own = *index.entry_weight.get(&(code.clone(), phrase.to_string()))?;
        for ((entry_code, entry_phrase), weight) in &index.entry_weight {
            if entry_code != code || entry_phrase == phrase || *weight <= own {
                continue;
            }
            if best.as_ref().map_or(true, |(w, _)| *weight > *w) {
                best = Some((*weight, entry_phrase.clone()));
            }
        }
    }
    best.map(|(_, phrase)| phrase)
}

// Held-out accuracy on real text: at every position where the walker would pick
// the wrong word (because `current` loses its own qstring), does the overlay fix
// it — and does it break positions that were already right?
fn evaluate(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut overlay = None;
    let mut overlay_cur_column = 1usize;
    let mut corpora = Vec::new();
    let mut boost = 1.5_f64;
    let mut always_wins = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = iter.next().cloned(),
            "--overlay" => overlay = iter.next().cloned(),
            "--overlay-cur-column" => {
                overlay_cur_column = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1)
            }
            // model the release calibration instead of assuming every row wins
            "--boost" => boost = iter.next().and_then(|v| v.parse().ok()).unwrap_or(1.5),
            "--assume-always-wins" => always_wins = true,
            other => corpora.push(other.to_string()),
        }
    }
    let lexicon_path = lexicon_path.ok_or("--lexicon required")?;
    let lexicon = load_lexicon(&lexicon_path)?;
    let index = load_code_index(&lexicon_path)?;

    // A row only fires when the user types the reading its qstring encodes, so key
    // the overlay by (previous, current, current's code) rather than by text alone.
    // Beyond that, the walker reads only the top row of each (qstring, previous)
    // group and only takes it when it outscores the unigram head, so replay the
    // release calibration rather than assuming every row wins.
    let mut raw_rows = Vec::<(String, String, String, String, f64)>::new();
    let mut raw_max = f64::NEG_INFINITY;
    if let Some(path) = &overlay {
        let reader = BufReader::new(File::open(path)?);
        for line in reader.lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() <= overlay_cur_column {
                continue;
            }
            let prev = f[overlay_cur_column - 1];
            let cur = f[overlay_cur_column];
            if prev.is_empty() || cur.is_empty() {
                continue;
            }
            let qstring = if overlay_cur_column >= 2 {
                f[overlay_cur_column - 2].to_string()
            } else {
                String::new()
            };
            let cur_code = qstring
                .split(' ')
                .nth(1)
                .map(str::to_string)
                .or_else(|| primary_code(&index, cur).map(|(code, _)| code));
            let raw: f64 = f
                .get(overlay_cur_column + 1)
                .and_then(|v| v.parse().ok())
                .unwrap_or(-0.2);
            if raw > raw_max {
                raw_max = raw;
            }
            if let Some(code) = cur_code {
                raw_rows.push((qstring, prev.to_string(), cur.to_string(), code, raw));
            }
        }
    }

    // keep only the strongest row per (qstring, previous): the rest are unreachable
    let mut group_best = HashMap::<(String, String), f64>::new();
    for (qstring, prev, _, _, raw) in &raw_rows {
        let key = (qstring.clone(), prev.clone());
        let slot = group_best.entry(key).or_insert(f64::NEG_INFINITY);
        if *raw > *slot {
            *slot = *raw;
        }
    }

    let mut pairs = HashSet::<(String, String, String)>::new();
    let (mut cls_a, mut cls_b, mut cls_c, mut cls_d) = (0u64, 0u64, 0u64, 0u64);
    for (qstring, prev, cur, code, raw) in &raw_rows {
        if group_best
            .get(&(qstring.clone(), prev.clone()))
            .is_some_and(|best| raw < best)
        {
            cls_a += 1;
            continue;
        }
        if always_wins {
            pairs.insert((prev.clone(), cur.clone(), code.clone()));
            continue;
        }
        let unigram = lexicon.words.get(cur).copied().unwrap_or(-4.0);
        let stored = (unigram + boost + (raw - raw_max)).min(-0.05);
        // the unigram head starts at the code's best candidate and can sink to the
        // weakest one as the user learns, so [lo, hi] bounds what the row must beat
        let (hi, _) = match index.code_best.get(code) {
            Some(v) => *v,
            None => continue,
        };
        let lo = index
            .code_phrases
            .get(code)
            .and_then(|phrases| {
                phrases
                    .iter()
                    .filter_map(|p| index.entry_weight.get(&(code.clone(), p.clone())))
                    .copied()
                    .fold(None, |acc: Option<f64>, w| {
                        Some(acc.map_or(w, |a: f64| a.min(w)))
                    })
            })
            .unwrap_or(hi);
        if stored > hi {
            cls_c += 1;
            pairs.insert((prev.clone(), cur.clone(), code.clone()));
        } else if stored <= lo {
            cls_b += 1;
        } else {
            cls_d += 1;
        }
    }
    if overlay.is_some() && !always_wins {
        println!("rows_unreachable_A\t{cls_a}");
        println!("rows_never_wins_B\t{cls_b}");
        println!("rows_always_effective_C\t{cls_c}");
        println!("rows_conditional_D\t{cls_d}");
    }

    let (mut positions, mut contested, mut fixed, mut broken, mut already_ok) = (0u64, 0u64, 0u64, 0u64, 0u64);
    for corpus in &corpora {
        let reader = BufReader::new(File::open(corpus)?);
        for line in reader.lines() {
            let line = line?;
            let mut run = Vec::<char>::new();
            let mut words = Vec::<String>::new();
            for c in line.chars() {
                if is_han(c) {
                    run.push(c);
                } else if !run.is_empty() {
                    words.extend(segment(&run, &lexicon));
                    run.clear();
                }
            }
            if !run.is_empty() {
                words.extend(segment(&run, &lexicon));
            }
            for window in words.windows(2) {
                let (prev, cur) = (&window[0], &window[1]);
                positions += 1;
                match verdict(&index, cur) {
                    Verdict::Contested(_) => {
                        contested += 1;
                        if let Some((code, _)) = primary_code(&index, cur) {
                            if pairs.contains(&(prev.clone(), cur.clone(), code)) {
                                fixed += 1;
                            }
                        }
                    }
                    Verdict::AlreadyTop | Verdict::NoCompetitor => {
                        already_ok += 1;
                        // would the overlay pull this position away from the right word?
                        if let Some((code, _)) = primary_code(&index, cur) {
                            let hijacked = index.code_phrases.get(&code).is_some_and(|phrases| {
                                phrases.iter().any(|phrase| {
                                    phrase != cur
                                        && pairs.contains(&(
                                            prev.clone(),
                                            phrase.clone(),
                                            code.clone(),
                                        ))
                                })
                            });
                            if hijacked {
                                broken += 1;
                            }
                        }
                    }
                    Verdict::NotInDict => {}
                }
            }
        }
    }
    println!("adjacent_positions\t{positions}");
    println!("already_correct\t{already_ok}");
    println!("contested_wrong_without_bigram\t{contested}");
    println!("fixed_by_overlay\t{fixed}");
    println!(
        "fix_rate_of_contested\t{:.4}",
        fixed as f64 / contested.max(1) as f64
    );
    println!("broken_by_overlay\t{broken}");
    println!(
        "net_gain\t{}",
        fixed as i64 - broken as i64
    );
    Ok(())
}

// Emit runtime rows in the repo's overlay format. A row is only safe when both
// sides have an unambiguous reading: `previous` must have exactly one code, and
// `current` is pinned to the code where its rival lives.
fn emit(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut input = None;
    let mut out_path = None;
    let mut all_readings = false;
    let mut all_cur_readings = false;
    let mut rival_evidence = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lexicon" => lexicon_path = iter.next().cloned(),
            "--input" => input = iter.next().cloned(),
            "--out" => out_path = iter.next().cloned(),
            // full (unthresholded) pair table, used to compare a candidate against
            // its homophone rivals after the same previous word
            "--rival-evidence" => rival_evidence = iter.next().cloned(),
            // a polyphonic previous is not wrong, it just needs one row per reading
            "--all-prev-readings" => all_readings = true,
            // opt-in, measured as a regression — see contested_codes
            "--all-cur-readings" => all_cur_readings = true,
            other => return Err(format!("unexpected emit argument: {other}").into()),
        }
    }
    let lexicon_path = lexicon_path.ok_or("--lexicon required")?;
    let index = load_code_index(&lexicon_path)?;
    let lexicon = load_lexicon(&lexicon_path)?;

    // (previous, current) -> doc_count over the whole corpus, before any threshold
    let mut evidence = HashMap::<(String, String), u64>::new();
    if let Some(path) = &rival_evidence {
        let reader = BufReader::new(File::open(path)?);
        for line in reader.lines() {
            let line = line?;
            if line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 4 {
                continue;
            }
            if let Ok(doc) = f[3].parse::<u64>() {
                evidence.insert((f[0].to_string(), f[1].to_string()), doc);
            }
        }
        eprintln!("rival evidence pairs: {}", evidence.len());
    }

    let reader = BufReader::new(File::open(input.ok_or("--input required")?)?);
    let mut rows = Vec::<(String, String, String, f64)>::new();
    let mut lost_to_rival = 0u64;
    let (mut ambiguous_prev, mut not_contested, mut seen_dup) = (0u64, 0u64, 0u64);
    let mut de_particle = 0u64;
    let mut title_surname = 0u64;
    let mut seen = HashSet::<(String, String, String)>::new();
    for line in reader.lines() {
        let line = line?;
        if line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 2 {
            continue;
        }
        let (prev, cur) = (f[0], f[1]);
        let docfreq: f64 = f.get(3).and_then(|v| v.parse().ok()).unwrap_or(1.0);
        // previous must have exactly one reading, or the qstring would be a guess
        let prev_codes = match index.phrase_codes.get(prev) {
            Some(codes) => codes,
            None => continue,
        };
        let mut unique_prev: Vec<&String> = prev_codes.iter().collect();
        unique_prev.sort();
        unique_prev.dedup();
        if unique_prev.len() != 1 && !all_readings {
            ambiguous_prev += 1;
            continue;
        }
        // 的/地/得 are being separated by tone in the IME itself (ㄉㄜ˙ -> 的,
        // ㄉㄜˊ -> 得), so bigram rows for them are dead weight and currently
        // steal positions from the correct particle.
        if matches!(cur, "的" | "地" | "得") {
            de_particle += 1;
            continue;
        }
        // A title followed by a single character is a surname the LY gazette
        // happens to mention often; it disambiguates one legislator's name, not
        // the language, so it is corpus bias rather than a useful collocation.
        if matches!(prev, "立法委員" | "立委") && cur.chars().count() == 1 {
            title_surname += 1;
            continue;
        }
        // current: every code where it actually loses is a collision we can fix, and
        // each gets its own row. The corpus cannot tell readings apart — it only shows
        // characters — so the same doc_count backs every reading of this pair; which
        // reading a row applies to is decided by the code the user types.
        let cur_codes = if all_cur_readings {
            contested_codes(&index, cur)
        } else {
            contested_code(&index, cur).into_iter().collect()
        };
        if cur_codes.is_empty() {
            not_contested += 1;
            continue;
        }
        for cur_code in &cur_codes {
            // the corpus itself must prefer this candidate over every homophone
            // rival after the same previous word, otherwise the row fights the data.
            // Rivals are the other candidates at *this* reading's code, so this has to
            // be judged per reading.
            if !evidence.is_empty() {
                let own = evidence
                    .get(&(prev.to_string(), cur.to_string()))
                    .copied()
                    .unwrap_or(0);
                let beaten = index.code_phrases.get(cur_code).is_some_and(|phrases| {
                    phrases.iter().any(|phrase| {
                        if phrase == cur {
                            return false;
                        }
                        // A rival whose `prev + rival` is itself a dictionary word never
                        // shows up as a pair: segmentation merges it into one token. Its
                        // zero count is an artifact, not evidence of disuse, and the
                        // whole-word path already spells it. Treat it as attested.
                        // Narrow to the demonstrated failure mode: the rival spells the
                        // same word with one character changed (裡/裏, 紀念/記念) and
                        // `prev + rival` is a dictionary compound, so segmentation merged
                        // it away and its zero count is an artifact rather than disuse.
                        let minimal_pair = phrase.chars().count() == cur.chars().count()
                            && phrase
                                .chars()
                                .zip(cur.chars())
                                .filter(|(a, b)| a != b)
                                .count()
                                == 1;
                        if minimal_pair && lexicon.words.contains_key(&format!("{prev}{phrase}")) {
                            return true;
                        }
                        evidence
                            .get(&(prev.to_string(), phrase.clone()))
                            .copied()
                            .unwrap_or(0)
                            > own
                    })
                });
                if beaten {
                    lost_to_rival += 1;
                    continue;
                }
            }
            for prev_code in &unique_prev {
                let qstring = format!("{prev_code} {cur_code}");
                if !seen.insert((qstring.clone(), prev.to_string(), cur.to_string())) {
                    seen_dup += 1;
                    continue;
                }
                rows.push((qstring, prev.to_string(), cur.to_string(), docfreq));
            }
        }
    }

    // `probability` is this source's internal strength order, not a conditional
    // probability: release re-anchors it to the current unigram floor. Same
    // doc-frequency mapping as tw-ly-transcript so the two layers stay comparable.
    const CEIL: f64 = -0.2;
    const SPAN: f64 = 1.5;
    let max_doc = rows.iter().map(|r| r.3).fold(1.0_f64, f64::max);
    let min_doc = rows.iter().map(|r| r.3).fold(f64::MAX, f64::min);
    let denominator = (max_doc / min_doc.max(1.0)).ln();
    let mut out = BufWriter::new(File::create(out_path.ok_or("--out required")?)?);
    writeln!(out, "# qstring\tprevious\tcurrent\tprobability")?;
    let mut emitted: Vec<String> = Vec::with_capacity(rows.len());
    for (qstring, prev, cur, docfreq) in &rows {
        let raw = if denominator <= f64::EPSILON {
            CEIL
        } else {
            CEIL - SPAN * (max_doc / docfreq).ln() / denominator
        };
        emitted.push(format!("{qstring}\t{prev}\t{cur}\t{raw:.6}"));
    }
    emitted.sort();
    for line in &emitted {
        writeln!(out, "{line}")?;
    }
    eprintln!(
        "emitted={} ambiguous_prev_skipped={ambiguous_prev} current_not_contested={not_contested} de_particle_skipped={de_particle} title_surname_skipped={title_surname} lost_to_rival={lost_to_rival} duplicate_keys={seen_dup}",
        emitted.len()
    );
    Ok(())
}

// The word's usual reading: highest own weight across its codes. Ties keep the
// first code in file order, so callers that need every reading must not use this.
fn primary_code(index: &CodeIndex, phrase: &str) -> Option<(String, f64)> {
    let codes = index.phrase_codes.get(phrase)?;
    let mut best: Option<(String, f64)> = None;
    for code in codes {
        let own = *index.entry_weight.get(&(code.clone(), phrase.to_string()))?;
        if best.as_ref().map_or(true, |(_, w)| own > *w) {
            best = Some((code.clone(), own));
        }
    }
    best
}

// The collision a bigram can fix: `phrase` loses at its own usual reading. This is
// the shipping behaviour; see contested_codes for why covering every reading was
// measured and rejected.
fn contested_code(index: &CodeIndex, phrase: &str) -> Option<String> {
    let (code, own) = primary_code(index, phrase)?;
    let (code_best, count) = *index.code_best.get(&code)?;
    if count > 1 && own < code_best {
        Some(code)
    } else {
        None
    }
}

// Every reading where `phrase` loses at its own code, not just the usual one.
// Opt-in via `--all-cur-readings`, and measured to be a large regression — kept so
// the experiment is repeatable, not because it should be on.
//
// Why it fails: a row bound to a secondary reading fires at that reading's node,
// where some *other*, often far commoner word is the head — 粘 at ㄋㄧㄢˊ (`~I`)
// sits under 年 (-0.64 vs -1.81). The corpus cannot say which reading was meant for
// a given occurrence, so the same doc_count backs every reading, and the evidence
// check waves through a row whose support actually came from the ㄓㄢ reading. The
// result is rows that steal positions from common words instead of staying inert:
// held-out `broken_by_overlay` roughly tripled across all three registers while
// `fixed_by_overlay` did not move.
fn contested_codes(index: &CodeIndex, phrase: &str) -> Vec<String> {
    let Some(codes) = index.phrase_codes.get(phrase) else {
        return Vec::new();
    };
    let mut contested = Vec::new();
    let mut seen = HashSet::new();
    for code in codes {
        if !seen.insert(code.as_str()) {
            continue;
        }
        let Some(&own) = index.entry_weight.get(&(code.clone(), phrase.to_string())) else {
            continue;
        };
        let Some(&(code_best, count)) = index.code_best.get(code) else {
            continue;
        };
        if count > 1 && own < code_best {
            contested.push(code.clone());
        }
    }
    contested.sort();
    contested
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_from(rows: &[(&str, &str, f64)]) -> CodeIndex {
        let mut phrase_codes = HashMap::<String, Vec<String>>::new();
        let mut code_best = HashMap::<String, (f64, u64)>::new();
        let mut entry_weight = HashMap::<(String, String), f64>::new();
        for (code, phrase, weight) in rows {
            phrase_codes
                .entry(phrase.to_string())
                .or_default()
                .push(code.to_string());
            let slot = code_best.entry(code.to_string()).or_insert((f64::MIN, 0));
            slot.1 += 1;
            if *weight > slot.0 {
                slot.0 = *weight;
            }
            entry_weight.insert((code.to_string(), phrase.to_string()), *weight);
        }
        let mut code_phrases = HashMap::<String, Vec<String>>::new();
        for (code, phrase) in entry_weight.keys() {
            code_phrases
                .entry(code.clone())
                .or_default()
                .push(phrase.clone());
        }
        CodeIndex {
            phrase_codes,
            code_phrases,
            code_best,
            entry_weight,
        }
    }

    // 粘's real numbers: both readings weigh -1.806039, and it loses at both — to
    // 沾/詹 at ㄓㄢ and to 年 at ㄋㄧㄢˊ. primary_code can only name one of them, and
    // on that exact tie it names whichever came first in the file.
    // The shipping default keeps only the usual reading; --all-cur-readings widens
    // it. Both are covered because the wide form stays available as an experiment.
    #[test]
    fn contested_code_keeps_only_the_usual_reading() {
        let index = index_from(&[
            ("A:", "粘", -1.806039),
            ("A:", "沾", -1.481233),
            ("~I", "粘", -1.806039),
            ("~I", "年", -0.639652),
        ]);
        let picked = contested_code(&index, "粘").unwrap();
        assert!(picked == "A:" || picked == "~I", "picked {picked}");
    }

    #[test]
    fn contested_codes_returns_every_losing_reading() {
        let index = index_from(&[
            ("A:", "粘", -1.806039),
            ("A:", "沾", -1.481233),
            ("A:", "詹", -1.481318),
            ("~I", "粘", -1.806039),
            ("~I", "年", -0.639652),
        ]);
        assert_eq!(contested_codes(&index, "粘"), vec!["A:", "~I"]);
    }

    #[test]
    fn contested_codes_skips_readings_where_the_word_already_wins() {
        let index = index_from(&[
            ("A:", "粘", -1.806039),
            ("A:", "沾", -1.481233),
            // at ~I it outweighs its only rival, so the unigram already wins there
            ("~I", "粘", -1.806039),
            ("~I", "黏", -1.817493),
        ]);
        assert_eq!(contested_codes(&index, "粘"), vec!["A:"]);
    }

    #[test]
    fn contested_codes_skips_readings_with_no_rival() {
        let index = index_from(&[("A:", "粘", -1.806039), ("A:", "沾", -1.481233), ("~I", "粘", -1.806039)]);
        assert_eq!(contested_codes(&index, "粘"), vec!["A:"]);
    }

    #[test]
    fn contested_codes_is_empty_for_unknown_phrase() {
        let index = index_from(&[("A:", "粘", -1.8)]);
        assert!(contested_codes(&index, "沒這個詞").is_empty());
    }
}
