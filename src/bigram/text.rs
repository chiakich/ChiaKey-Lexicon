//! Turning raw corpus lines into lexicon tokens.

use super::lexicon::Lexicon;

/// Splits a line into runs of Han characters at least two codepoints long,
/// dropping metadata lines and anything punctuation- or Latin-separated.
pub(super) fn han_sentences(line: &str) -> Vec<String> {
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

/// Segments a sentence by maximizing the summed lexicon weight of the chosen
/// path, preferring longer tokens when two paths score equally.
pub(super) fn tokenize_sentence(sentence: &str, lexicon: &Lexicon) -> Vec<String> {
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

/// Particles that attach to anything, so a pair containing one carries no
/// collocation signal worth emitting.
pub(super) fn contains_excluded_particle(phrase: &str) -> bool {
    phrase.contains('的')
        || phrase == "在"
        || phrase == "為"
        || phrase == "個"
        || phrase == "了"
        || phrase == "任"
        || phrase == "地"
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
        let lexicon = Lexicon::for_tests(
            &[
                ("程式", "a", 0.0),
                ("語言", "b", 0.0),
                ("程式語言", "ab", 0.0),
            ],
            4,
        );
        assert_eq!(tokenize_sentence("程式語言", &lexicon), vec!["程式語言"]);
    }

    #[test]
    fn tokenizes_with_best_weighted_path() {
        let lexicon = Lexicon::for_tests(
            &[
                ("還以", "a", -2.0),
                ("還", "b", -0.5),
                ("以為", "c", -0.5),
                ("為", "d", -2.0),
            ],
            4,
        );
        assert_eq!(tokenize_sentence("還以為", &lexicon), vec!["還", "以為"]);
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
}
