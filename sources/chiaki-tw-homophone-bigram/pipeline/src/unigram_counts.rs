//! `unigrams`: corpus word counts under the walker's own segmentation.

use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
    thread,
};

use crate::lexicon::{is_han, load_lexicon, segment, Lexicon};

// ---- corpus unigram counts -------------------------------------------------
// Counts how often each lexicon word survives as a token under the SAME Viterbi
// segmentation the walker uses, so the counts are in the engine's tokenization
// rather than some other segmenter's. Emits one column per corpus file: the
// blend experiments need to leave a domain out, and recounting 1.2 GB per
// subset would be the slow way to get there.

struct FileCounts {
    label: String,
    counts: HashMap<String, u64>,
    tokens: u64,
    oov_tokens: u64,
}

fn count_range(
    path: &str,
    start: u64,
    end: u64,
    lexicon: &Lexicon,
) -> io::Result<(HashMap<String, u64>, u64, u64)> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(start))?;
    let mut reader = BufReader::with_capacity(1 << 20, file);
    let mut pos = start;
    // A chunk boundary lands mid-line; the thread owning the previous chunk
    // reads that line to its end, so this one drops the partial head.
    if start > 0 {
        let mut skip = Vec::new();
        pos += reader.read_until(b'\n', &mut skip)? as u64;
    }
    let mut counts = HashMap::<String, u64>::new();
    let mut tokens = 0u64;
    let mut oov = 0u64;
    let mut line = Vec::new();
    let mut run: Vec<char> = Vec::new();
    while pos < end {
        line.clear();
        let read = reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            break;
        }
        pos += read as u64;
        let text = String::from_utf8_lossy(&line);
        run.clear();
        for c in text.chars().chain(std::iter::once('\n')) {
            if is_han(c) {
                run.push(c);
                continue;
            }
            if run.is_empty() {
                continue;
            }
            for word in segment(&run, lexicon) {
                tokens += 1;
                if lexicon.words.contains_key(&word) {
                    *counts.entry(word).or_insert(0) += 1;
                } else {
                    oov += 1;
                }
            }
            run.clear();
        }
    }
    Ok((counts, tokens, oov))
}

fn count_file(path: &str, lexicon: &Arc<Lexicon>, threads: usize) -> io::Result<FileCounts> {
    let len = File::open(path)?.metadata()?.len();
    let threads = threads.max(1);
    let chunk = (len / threads as u64).max(1);
    let mut handles = Vec::new();
    for i in 0..threads {
        let start = chunk * i as u64;
        if start >= len && i > 0 {
            break;
        }
        let end = if i + 1 == threads { len } else { chunk * (i + 1) as u64 };
        let lexicon = Arc::clone(lexicon);
        let path = path.to_string();
        handles.push(thread::spawn(move || count_range(&path, start, end, &lexicon)));
    }
    let mut counts = HashMap::<String, u64>::new();
    let mut tokens = 0u64;
    let mut oov = 0u64;
    for handle in handles {
        let (part, part_tokens, part_oov) = handle.join().expect("count thread panicked")?;
        tokens += part_tokens;
        oov += part_oov;
        if counts.is_empty() {
            counts = part;
        } else {
            for (word, n) in part {
                *counts.entry(word).or_insert(0) += n;
            }
        }
    }
    let label = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());
    Ok(FileCounts { label, counts, tokens, oov_tokens: oov })
}

pub fn unigrams(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut lexicon_path = None;
    let mut out_path = None;
    let mut threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let mut length_bonus: Option<f64> = None;
    let mut corpora = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--lexicon" => {
                lexicon_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--out" => {
                out_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--length-bonus" => {
                length_bonus = args.get(i + 1).and_then(|v| v.parse().ok());
                i += 2;
            }
            "--threads" => {
                threads = args.get(i + 1).and_then(|t| t.parse().ok()).unwrap_or(threads);
                i += 2;
            }
            other => {
                corpora.push(other.to_string());
                i += 1;
            }
        }
    }
    let lexicon_path = lexicon_path.ok_or("unigrams: --lexicon is required")?;
    let out_path = out_path.ok_or("unigrams: --out is required")?;
    if corpora.is_empty() {
        return Err("unigrams: at least one corpus file is required".into());
    }

    let mut loaded = load_lexicon(&lexicon_path)?;
    if let Some(bonus) = length_bonus {
        loaded.length_bonus = bonus;
    }
    let lexicon = Arc::new(loaded);
    eprintln!(
        "lexicon: {} words, max {} chars, length bonus {}",
        lexicon.words.len(),
        lexicon.max_len,
        lexicon.length_bonus
    );

    let mut per_file = Vec::new();
    for path in &corpora {
        let started = std::time::Instant::now();
        let result = count_file(path, &lexicon, threads)?;
        eprintln!(
            "{}: {} tokens ({} out-of-lexicon), {} distinct, {:.1}s",
            result.label,
            result.tokens,
            result.oov_tokens,
            result.counts.len(),
            started.elapsed().as_secs_f64()
        );
        per_file.push(result);
    }

    let mut totals = HashMap::<String, u64>::new();
    for file in &per_file {
        for (word, n) in &file.counts {
            *totals.entry(word.clone()).or_insert(0) += n;
        }
    }
    let mut rows: Vec<(&String, u64)> = totals.iter().map(|(w, n)| (w, *n)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let mut writer = BufWriter::new(File::create(&out_path)?);
    write!(writer, "word\ttotal")?;
    for file in &per_file {
        write!(writer, "\t{}", file.label)?;
    }
    writeln!(writer)?;
    for (word, total) in &rows {
        write!(writer, "{}\t{}", word, total)?;
        for file in &per_file {
            write!(writer, "\t{}", file.counts.get(*word).copied().unwrap_or(0))?;
        }
        writeln!(writer)?;
    }
    writer.flush()?;

    let attested = rows.len();
    eprintln!(
        "wrote {} -- {}/{} lexicon words attested ({:.1}%)",
        out_path,
        attested,
        lexicon.words.len(),
        100.0 * attested as f64 / lexicon.words.len() as f64
    );
    Ok(())
}

