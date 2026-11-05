//! Lexicon loading and the Viterbi segmentation the walker uses.
//!
//! Shared by every subcommand that has to turn running text into the same word
//! boundaries the engine would produce.

use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader},
};

pub const LENGTH_BONUS: f64 = 1.0;
pub const UNKNOWN_CHAR_PENALTY: f64 = -9.0;
pub const MAX_WORD_CHARS: usize = 8;

pub struct Lexicon {
    // phrase -> best (highest) weight across readings
    pub words: HashMap<String, f64>,
    pub max_len: usize,
    // per extra character; LENGTH_BONUS matches the engine. Overriding it is a
    // cheap way to ask whether a result depends on this particular segmenter:
    // move it and the word boundaries move with it.
    pub length_bonus: f64,
}

pub fn load_lexicon(path: &str) -> io::Result<Lexicon> {
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
    Ok(Lexicon { words, max_len, length_bonus: LENGTH_BONUS })
}

pub fn is_han(c: char) -> bool {
    matches!(c as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

// Viterbi max-score segmentation over one Han-only run.
//
// The length bonus is per EXTRA character, matching Node::lengthPrior() in the
// engine. A bonus per character would sum to the run length whatever the
// segmentation and so express no preference at all -- which is what this used
// to do, leaving the extracted word boundaries misaligned with the walker's.
pub fn segment(run: &[char], lexicon: &Lexicon) -> Vec<String> {
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
                Some(w) => w + lexicon.length_bonus * (len - 1) as f64,
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

