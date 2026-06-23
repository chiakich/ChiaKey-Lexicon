#[derive(Clone, Copy)]
struct Component {
    text: &'static str,
    value: u16,
}

const COMPONENTS: &[Component] = &[
    Component {
        text: "ㄅ",
        value: 0x0001,
    },
    Component {
        text: "ㄆ",
        value: 0x0002,
    },
    Component {
        text: "ㄇ",
        value: 0x0003,
    },
    Component {
        text: "ㄈ",
        value: 0x0004,
    },
    Component {
        text: "ㄉ",
        value: 0x0005,
    },
    Component {
        text: "ㄊ",
        value: 0x0006,
    },
    Component {
        text: "ㄋ",
        value: 0x0007,
    },
    Component {
        text: "ㄌ",
        value: 0x0008,
    },
    Component {
        text: "ㄍ",
        value: 0x0009,
    },
    Component {
        text: "ㄎ",
        value: 0x000a,
    },
    Component {
        text: "ㄏ",
        value: 0x000b,
    },
    Component {
        text: "ㄐ",
        value: 0x000c,
    },
    Component {
        text: "ㄑ",
        value: 0x000d,
    },
    Component {
        text: "ㄒ",
        value: 0x000e,
    },
    Component {
        text: "ㄓ",
        value: 0x000f,
    },
    Component {
        text: "ㄔ",
        value: 0x0010,
    },
    Component {
        text: "ㄕ",
        value: 0x0011,
    },
    Component {
        text: "ㄖ",
        value: 0x0012,
    },
    Component {
        text: "ㄗ",
        value: 0x0013,
    },
    Component {
        text: "ㄘ",
        value: 0x0014,
    },
    Component {
        text: "ㄙ",
        value: 0x0015,
    },
    Component {
        text: "ㄧ",
        value: 0x0020,
    },
    Component {
        text: "ㄨ",
        value: 0x0040,
    },
    Component {
        text: "ㄩ",
        value: 0x0060,
    },
    Component {
        text: "ㄚ",
        value: 0x0080,
    },
    Component {
        text: "ㄛ",
        value: 0x0100,
    },
    Component {
        text: "ㄜ",
        value: 0x0180,
    },
    Component {
        text: "ㄝ",
        value: 0x0200,
    },
    Component {
        text: "ㄞ",
        value: 0x0280,
    },
    Component {
        text: "ㄟ",
        value: 0x0300,
    },
    Component {
        text: "ㄠ",
        value: 0x0380,
    },
    Component {
        text: "ㄡ",
        value: 0x0400,
    },
    Component {
        text: "ㄢ",
        value: 0x0480,
    },
    Component {
        text: "ㄣ",
        value: 0x0500,
    },
    Component {
        text: "ㄤ",
        value: 0x0580,
    },
    Component {
        text: "ㄥ",
        value: 0x0600,
    },
    Component {
        text: "ㄦ",
        value: 0x0680,
    },
    Component {
        text: "ˊ",
        value: 0x0800,
    },
    Component {
        text: "ˇ",
        value: 0x1000,
    },
    Component {
        text: "ˋ",
        value: 0x1800,
    },
    Component {
        text: "˙",
        value: 0x2000,
    },
];

pub fn qstring_for_bpmf_sequence(sequence: &str) -> Option<(String, usize)> {
    let syllables = sequence
        .split(|character: char| character == ',' || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if syllables.is_empty() {
        return None;
    }

    let mut qstring = String::new();
    for syllable in &syllables {
        qstring.push_str(&qstring_for_bpmf_syllable(syllable)?);
    }
    Some((qstring, syllables.len()))
}

pub fn phrase_candidate(text: &str, min_codepoints: usize, max_codepoints: usize) -> bool {
    if text.is_empty()
        || text.contains('\t')
        || text.contains('\n')
        || text.contains("http://")
        || text.contains("https://")
    {
        return false;
    }
    if text.chars().any(is_bopomofo_component) {
        return false;
    }
    let length = text.chars().count();
    length >= min_codepoints && length <= max_codepoints
}

fn qstring_for_bpmf_syllable(syllable: &str) -> Option<String> {
    let mut values = Vec::new();
    for character in syllable.chars() {
        let component = character.to_string();
        if let Some(value) = COMPONENTS
            .iter()
            .find(|item| item.text == component)
            .map(|item| item.value)
        {
            values.push(value);
        }
    }
    if values.is_empty() {
        return None;
    }
    Some(absolute_order_string(&values))
}

fn absolute_order_string(components: &[u16]) -> String {
    let syllable = components
        .iter()
        .fold(0_u16, |acc, component| acc | component);
    let order = (syllable & 0x001f) as u32
        + (((syllable & 0x0060) >> 5) as u32 * 22)
        + (((syllable & 0x0780) >> 7) as u32 * 22 * 4)
        + (((syllable & 0x3800) >> 11) as u32 * 22 * 4 * 14);
    let first = char::from_u32(48 + (order % 79)).expect("valid qstring byte");
    let second = char::from_u32(48 + (order / 79)).expect("valid qstring byte");
    format!("{first}{second}")
}

fn is_bopomofo_component(character: char) -> bool {
    let component = character.to_string();
    COMPONENTS.iter().any(|item| item.text == component)
}

#[cfg(test)]
mod tests {
    use super::qstring_for_bpmf_sequence;

    #[test]
    fn converts_libchewing_zhuyin_to_keykey_qstring() {
        assert_eq!(qstring_for_bpmf_sequence("ㄕㄨ ㄖㄨˋ").unwrap().0, "m0]_");
        assert_eq!(qstring_for_bpmf_sequence("ㄘㄜˋ ㄕˋ").unwrap().0, "Nb0_");
    }

    #[test]
    fn accepts_comma_separated_bopomofo() {
        assert_eq!(qstring_for_bpmf_sequence("ㄕㄨ,ㄖㄨˋ").unwrap().0, "m0]_");
    }

    #[test]
    fn converts_wo3_to_expected_keykey_qstring() {
        assert_eq!(qstring_for_bpmf_sequence("ㄨㄛˇ").unwrap().0, "}Q");
    }

    #[test]
    fn converts_neutral_ge_to_expected_keykey_qstring() {
        assert_eq!(qstring_for_bpmf_sequence("ㄍㄜ˙").unwrap().0, "rq");
    }
}
