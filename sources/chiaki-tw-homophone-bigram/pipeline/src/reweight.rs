//! `reweight`: blends lexicon weights towards a corpus estimate.

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
};

// ---- corpus reweighting ----------------------------------------------------
// Blends the lexicon weight towards a corpus estimate. Three properties matter
// and each is a guard against a way this goes wrong:
//
//   * rows come in and go out in their original order, so the walker's tie
//     break (stable_sort keeps physical row order) is untouched and alpha=0
//     reproduces the input DB exactly;
//   * the corpus scale is squeezed to the lexicon's spread before blending --
//     raw log10 frequency is 1.7x wider and would silently rescale what
//     Node::lengthPrior()'s +1.0 buys;
//   * a word moves at most `delta`, because the counts come from a Viterbi
//     segmentation driven by these very weights. Segmentation debris that
//     already wins (說沒, 超超) scores high and would otherwise be promoted
//     into scoring higher still.
//
// A word's delta is computed from its highest-weighted row and applied to all
// of its rows, so the split between a polyphone's readings survives: the corpus
// count is per word and cannot tell 的/ㄉㄜ˙ from 的/ㄉㄧˋ apart.

pub fn reweight(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let mut rows_path = None;
    let mut counts_path = None;
    let mut out_path = None;
    let mut alpha = 0.0f64;
    let mut delta = f64::INFINITY;
    let mut skip_columns: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--rows" => {
                rows_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--counts" => {
                counts_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--out" => {
                out_path = args.get(i + 1).cloned();
                i += 2;
            }
            "--alpha" => {
                alpha = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(alpha);
                i += 2;
            }
            "--delta" => {
                delta = args.get(i + 1).and_then(|v| v.parse().ok()).unwrap_or(delta);
                i += 2;
            }
            // leave-one-domain-out: drop a counts column before summing
            "--hold-out" => {
                if let Some(v) = args.get(i + 1) {
                    skip_columns.push(v.clone());
                }
                i += 2;
            }
            other => return Err(format!("reweight: unknown argument {other}").into()),
        }
    }
    let rows_path = rows_path.ok_or("reweight: --rows is required")?;
    let counts_path = counts_path.ok_or("reweight: --counts is required")?;
    let out_path = out_path.ok_or("reweight: --out is required")?;

    // counts
    let mut counts = HashMap::<String, u64>::new();
    let mut total: u64 = 0;
    {
        let mut reader = BufReader::new(File::open(&counts_path)?);
        let mut header = String::new();
        reader.read_line(&mut header)?;
        let header: Vec<&str> = header.trim_end().split('\t').collect();
        let mut keep = Vec::new();
        for (idx, name) in header.iter().enumerate().skip(2) {
            if skip_columns.iter().any(|s| s == name) {
                eprintln!("holding out column {name}");
            } else {
                keep.push(idx);
            }
        }
        if keep.len() + 2 != header.len() && skip_columns.is_empty() {
            return Err("reweight: counts file has no per-corpus columns".into());
        }
        for line in reader.lines() {
            let line = line?;
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 2 {
                continue;
            }
            let n: u64 = keep
                .iter()
                .filter_map(|&idx| fields.get(idx))
                .filter_map(|v| v.parse::<u64>().ok())
                .sum();
            if n == 0 {
                continue;
            }
            total += n;
            counts.insert(fields[0].to_string(), n);
        }
    }
    eprintln!("counts: {} words, {} tokens", counts.len(), total);

    // rows, in input order
    struct Row {
        rowid: String,
        qstring: String,
        current: String,
        probability: f64,
        backoff: String,
    }
    let mut rows = Vec::new();
    for line in BufReader::new(File::open(&rows_path)?).lines() {
        let line = line?;
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 5 {
            continue;
        }
        rows.push(Row {
            rowid: f[0].to_string(),
            qstring: f[1].to_string(),
            current: f[2].to_string(),
            probability: f[3].parse().unwrap_or(0.0),
            backoff: f[4].to_string(),
        });
    }
    eprintln!("rows: {}", rows.len());

    // one anchor weight per word: the highest-scoring row, matching how the
    // corpus counter collapsed readings when it built its lexicon
    let mut anchor = HashMap::<&str, f64>::new();
    for row in &rows {
        anchor
            .entry(row.current.as_str())
            .and_modify(|w| {
                if row.probability > *w {
                    *w = row.probability;
                }
            })
            .or_insert(row.probability);
    }

    // squeeze the corpus scale onto the lexicon's, over the attested overlap
    let pairs: Vec<(f64, f64)> = anchor
        .iter()
        .filter_map(|(word, w)| {
            counts
                .get(*word)
                .map(|n| (*w, (*n as f64 / total as f64).log10()))
        })
        .collect();
    if pairs.is_empty() {
        return Err("reweight: no lexicon word is attested in the counts".into());
    }
    let n = pairs.len() as f64;
    let mean_l = pairs.iter().map(|p| p.0).sum::<f64>() / n;
    let mean_c = pairs.iter().map(|p| p.1).sum::<f64>() / n;
    let sd = |it: &dyn Fn(&(f64, f64)) -> f64, mean: f64| {
        (pairs.iter().map(|p| (it(p) - mean).powi(2)).sum::<f64>() / n).sqrt()
    };
    let sd_l = sd(&|p: &(f64, f64)| p.0, mean_l);
    let sd_c = sd(&|p: &(f64, f64)| p.1, mean_c);
    eprintln!(
        "overlap {n:.0} words; lexicon mean {mean_l:.3} sd {sd_l:.3}; corpus mean {mean_c:.3} sd {sd_c:.3}"
    );

    let mut moved = 0usize;
    let mut clamped = 0usize;
    let mut shifts = HashMap::<&str, f64>::new();
    for (word, old) in &anchor {
        let count = match counts.get(*word) {
            Some(c) => *c,
            None => continue,
        };
        let corpus = (count as f64 / total as f64).log10();
        let target = mean_l + (corpus - mean_c) / sd_c * sd_l;
        let blended = (1.0 - alpha) * old + alpha * target;
        let mut shift = blended - old;
        if shift.abs() > delta {
            shift = delta * shift.signum();
            clamped += 1;
        }
        if shift != 0.0 {
            moved += 1;
            shifts.insert(*word, shift);
        }
    }
    eprintln!("words moved {moved}, of which clamped {clamped}");

    let mut writer = BufWriter::new(File::create(&out_path)?);
    for row in &rows {
        let shift = shifts.get(row.current.as_str()).copied().unwrap_or(0.0);
        writeln!(
            writer,
            "{}\t{}\t{}\t{:.6}\t{}",
            row.rowid,
            row.qstring,
            row.current,
            row.probability + shift,
            row.backoff
        )?;
    }
    writer.flush()?;
    eprintln!("wrote {out_path}");
    Ok(())
}

