use crate::config::PREPOPULATED_SERVICE_SOURCE_ID;
use anyhow::{bail, Context, Result};
use chrono::DateTime;
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

const MIN_PUNCTUATION_LIST_ROWS: i64 = 400;
const MIN_MOZC_EMOTICON_ROWS: usize = 300;
const MAX_BUTTON_CATEGORY_ITEMS: usize = 100;
const REQUIRED_SUPPLEMENTAL_SYMBOLS: &[&str] = &["€", "₿", "①", "↔", "∑", "✓", "♪", "〒"];
const REQUIRED_SUPPLEMENTAL_CATEGORIES: &[&str] = &[
    "補充標點",
    "貨幣與標記",
    "數字序號",
    "補充箭頭",
    "補充數學",
    "勾叉與星號",
    "花色與音樂",
    "單位符號",
];
const REQUIRED_MOZC_EMOTICONS: &[&str] = &["＼(^o^)／", "(T_T)", "m(_ _)m", "(´・ω・｀)"];

#[derive(Clone, Copy)]
struct SupplementalCategory {
    en: &'static str,
    zh_tw: &'static str,
}

const SUPPLEMENTAL_CATEGORIES: &[SupplementalCategory] = &[
    SupplementalCategory {
        en: "Supplemental Punctuation",
        zh_tw: "補充標點",
    },
    SupplementalCategory {
        en: "Currency and Marks",
        zh_tw: "貨幣與標記",
    },
    SupplementalCategory {
        en: "Numbers and Ordinals",
        zh_tw: "數字序號",
    },
    SupplementalCategory {
        en: "Supplemental Arrows",
        zh_tw: "補充箭頭",
    },
    SupplementalCategory {
        en: "Supplemental Math",
        zh_tw: "補充數學",
    },
    SupplementalCategory {
        en: "Checks, Crosses, and Stars",
        zh_tw: "勾叉與星號",
    },
    SupplementalCategory {
        en: "Card Suits and Music",
        zh_tw: "花色與音樂",
    },
    SupplementalCategory {
        en: "Unit Symbols",
        zh_tw: "單位符號",
    },
];

#[derive(Debug)]
struct SupplementalSymbol {
    symbol: String,
    tags: Vec<String>,
}

pub struct ServiceData {
    pub canned_messages: String,
    pub timestamp: i64,
    pub supplemental_symbol_count: usize,
    pub emoji_message_count: usize,
}

pub fn load(
    canned_messages_path: &Path,
    supplemental_symbols_path: &Path,
    mozc_categorized_path: &Path,
    mozc_emoticon_path: &Path,
    generated_at: &str,
) -> Result<ServiceData> {
    let timestamp = release_timestamp(generated_at)?;
    let canned_messages = fs::read_to_string(canned_messages_path)
        .with_context(|| format!("read {}", canned_messages_path.display()))?;
    let supplemental_symbols = load_supplemental_symbols(supplemental_symbols_path)?;
    let mozc_emoticons = load_mozc_emoticons(mozc_categorized_path, mozc_emoticon_path)?;
    let (canned_messages, supplemental_symbol_count) =
        append_supplemental_symbol_buttons(&canned_messages, &supplemental_symbols)?;
    let (canned_messages, emoji_message_count) =
        replace_emoji_category_with_mozc_messages(&canned_messages, &mozc_emoticons)?;

    Ok(ServiceData {
        canned_messages,
        timestamp,
        supplemental_symbol_count,
        emoji_message_count,
    })
}

pub fn validate_payload(data: &ServiceData) -> Result<()> {
    if data.canned_messages.len() <= 1000 {
        bail!("canned_messages payload is too small");
    }
    if data.timestamp <= 0 {
        bail!("prepopulated service data timestamp must be greater than 0");
    }
    validate_canned_messages_content(&data.canned_messages)?;
    Ok(())
}

pub fn validate_runtime_required_data(conn: &Connection) -> Result<()> {
    let punctuation_list_unigrams: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unigrams WHERE qstring = '_punctuation_list'",
        [],
        |row| row.get(0),
    )?;
    if punctuation_list_unigrams < MIN_PUNCTUATION_LIST_ROWS {
        bail!(
            "missing supplemental runtime punctuation list rows in unigrams: {punctuation_list_unigrams}"
        );
    }

    let punctuation_list_cin: i64 = conn.query_row(
        "SELECT COUNT(*) FROM 'Mandarin-bpmf-cin' WHERE key = '_punctuation_list'",
        [],
        |row| row.get(0),
    )?;
    if punctuation_list_cin < MIN_PUNCTUATION_LIST_ROWS {
        bail!(
            "missing supplemental runtime punctuation list rows in Mandarin-bpmf-cin: {punctuation_list_cin}"
        );
    }

    for symbol in REQUIRED_SUPPLEMENTAL_SYMBOLS {
        require_punctuation_symbol(conn, symbol)?;
    }

    let punctuation_less_than: String = conn.query_row(
        "SELECT current FROM unigrams WHERE qstring = '_punctuation_<'
         ORDER BY probability DESC, current LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    if punctuation_less_than != "，" {
        bail!("_punctuation_< resolves to {punctuation_less_than}, expected ，");
    }

    let punctuation_standard_less_than: String = conn.query_row(
        "SELECT current FROM unigrams WHERE qstring = '_punctuation_Standard_<'
         ORDER BY probability DESC, current LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    if punctuation_standard_less_than != "，" {
        bail!("_punctuation_Standard_< resolves to {punctuation_standard_less_than}, expected ，");
    }

    let bpmf_cname: i64 = conn.query_row(
        "SELECT COUNT(*) FROM 'Mandarin-bpmf-cin'
         WHERE key = '__property_cname' AND value = '注音'",
        [],
        |row| row.get(0),
    )?;
    if bpmf_cname == 0 {
        bail!("missing Mandarin-bpmf-cin __property_cname metadata");
    }

    require_service_row(
        conn,
        "canned_messages",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'canned_messages' AND LENGTH(value) > 1000",
    )?;
    let canned_messages: String = conn.query_row(
        "SELECT value FROM prepopulated_service_data
         WHERE key = 'canned_messages'",
        [],
        |row| row.get(0),
    )?;
    validate_canned_messages_content(&canned_messages)?;
    require_service_row(
        conn,
        "canned_messages_timestamp",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'canned_messages_timestamp' AND CAST(value AS INTEGER) > 0",
    )?;
    forbid_service_row(
        conn,
        "onekey_services",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'onekey_services'",
    )?;
    forbid_service_row(
        conn,
        "onekey_services_timestamp",
        "SELECT COUNT(*) FROM prepopulated_service_data
         WHERE key = 'onekey_services_timestamp'",
    )?;

    Ok(())
}

pub fn source_kind() -> &'static str {
    PREPOPULATED_SERVICE_SOURCE_ID
}

fn release_timestamp(generated_at: &str) -> Result<i64> {
    let timestamp = DateTime::parse_from_rfc3339(generated_at)
        .with_context(|| format!("parse generated_at timestamp {generated_at}"))?
        .timestamp();
    if timestamp <= 0 {
        bail!("generated_at timestamp must be greater than 0");
    }
    Ok(timestamp)
}

fn load_supplemental_symbols(path: &Path) -> Result<Vec<SupplementalSymbol>> {
    let file = fs::File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut seen = HashSet::new();
    let mut symbols = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut columns = line.split('\t');
        let symbol = columns.next().unwrap_or_default().trim();
        if symbol.is_empty() || symbol.chars().any(char::is_control) {
            continue;
        }
        let tags = columns
            .next()
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if seen.insert(symbol.to_string()) {
            symbols.push(SupplementalSymbol {
                symbol: symbol.to_string(),
                tags,
            });
        }
    }

    Ok(symbols)
}

fn load_mozc_emoticons(categorized_path: &Path, emoticon_path: &Path) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut emoticons = Vec::new();

    load_mozc_emoticon_values(categorized_path, &mut seen, &mut emoticons)?;
    load_mozc_emoticon_values(emoticon_path, &mut seen, &mut emoticons)?;

    if emoticons.len() < MIN_MOZC_EMOTICON_ROWS {
        bail!(
            "Mozc emoticon data has too few entries: {}",
            emoticons.len()
        );
    }
    for required in REQUIRED_MOZC_EMOTICONS {
        if !seen.contains(*required) {
            bail!("Mozc emoticon data is missing required sample {required}");
        }
    }

    Ok(emoticons)
}

fn load_mozc_emoticon_values(
    path: &Path,
    seen: &mut HashSet<String>,
    emoticons: &mut Vec<String>,
) -> Result<()> {
    let file = fs::File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        let line = line.trim_end_matches('\r');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let value = line.split('\t').next().unwrap_or_default().trim();
        if value.is_empty() || value.chars().any(char::is_control) || value.chars().count() > 80 {
            continue;
        }

        if seen.insert(value.to_string()) {
            emoticons.push(value.to_string());
        }
    }

    Ok(())
}

fn append_supplemental_symbol_buttons(
    canned_messages: &str,
    symbols: &[SupplementalSymbol],
) -> Result<(String, usize)> {
    let mut categorized_buttons = vec![Vec::<String>::new(); SUPPLEMENTAL_CATEGORIES.len()];
    for symbol in symbols {
        let escaped = xml_escape(&symbol.symbol);
        if !canned_messages.contains(&format!("<string>{escaped}</string>")) {
            let index = supplemental_category_index(&symbol.tags);
            categorized_buttons[index].push(escaped);
        }
    }

    let total = categorized_buttons.iter().map(Vec::len).sum::<usize>();
    if total == 0 {
        return Ok((canned_messages.to_string(), 0));
    }

    let mut categories = String::new();
    for (spec, buttons) in SUPPLEMENTAL_CATEGORIES
        .iter()
        .zip(categorized_buttons.iter())
    {
        if buttons.is_empty() {
            continue;
        }
        categories.push_str(&button_category_xml(*spec, buttons));
    }

    let closing = "\n\t</array>\n</dict>\n</plist>";
    let Some(insert_at) = canned_messages.rfind(closing) else {
        bail!("canned_messages plist does not have the expected CannedMessages closing array");
    };

    let mut output = String::with_capacity(canned_messages.len() + categories.len());
    output.push_str(&canned_messages[..insert_at]);
    output.push_str(&categories);
    output.push_str(&canned_messages[insert_at..]);
    Ok((output, total))
}

fn supplemental_category_index(tags: &[String]) -> usize {
    if has_tag(tags, "unit") {
        7
    } else if has_tag(tags, "card-suit") || has_tag(tags, "music") {
        6
    } else if has_tag(tags, "checkmark")
        || has_tag(tags, "cross")
        || has_tag(tags, "star")
        || has_tag(tags, "asterisk")
        || has_tag(tags, "flower")
    {
        5
    } else if has_tag(tags, "math") {
        4
    } else if has_tag(tags, "arrow") {
        3
    } else if has_tag(tags, "enclosed-number") || has_tag(tags, "roman-numeral") {
        2
    } else if has_tag(tags, "currency") || has_tag(tags, "legal") || has_tag(tags, "cjk-symbol") {
        1
    } else {
        0
    }
}

fn has_tag(tags: &[String], needle: &str) -> bool {
    tags.iter().any(|tag| tag == needle)
}

fn button_category_xml(spec: SupplementalCategory, buttons: &[String]) -> String {
    let mut category = String::from(
        "\n\t\t<dict>\n\
\t\t\t<key>Name</key>\n\
\t\t\t<dict>\n",
    );
    category.push_str("\t\t\t\t<key>en</key>\n\t\t\t\t<string>");
    category.push_str(&xml_escape(spec.en));
    category.push_str("</string>\n\t\t\t\t<key>zh_TW</key>\n\t\t\t\t<string>");
    category.push_str(&xml_escape(spec.zh_tw));
    category.push_str(
        "</string>\n\
\t\t\t</dict>\n\
\t\t\t<key>IsSymbolButtonList</key>\n\
\t\t\t<string>true</string>\n\
\t\t\t<key>Buttons</key>\n\
\t\t\t<array>\n",
    );
    for symbol in buttons {
        category.push_str("\t\t\t\t<string>");
        category.push_str(symbol);
        category.push_str("</string>\n");
    }
    category.push_str("\t\t\t</array>\n\t\t</dict>\n");
    category
}

fn replace_emoji_category_with_mozc_messages(
    canned_messages: &str,
    emoticons: &[String],
) -> Result<(String, usize)> {
    let messages_marker = "\n\t\t\t<key>Messages</key>\n\t\t\t<array>";
    let Some(messages_at) = canned_messages.rfind(messages_marker) else {
        bail!("canned_messages is missing original emoji Messages category");
    };
    let Some(category_start) = canned_messages[..messages_at].rfind("\n\t\t<dict>") else {
        bail!("emoji canned_messages category has no opening dictionary");
    };

    let category_prefix = &canned_messages[category_start..messages_at];
    if !category_prefix.contains("<string>Emoji (Emoticons)</string>")
        && !category_prefix.contains("<string>顏文字</string>")
    {
        bail!("canned_messages original Messages category is not the emoji category");
    }

    let category_close = "\n\t\t\t</array>\n\t\t</dict>";
    let Some(close_offset) = canned_messages[messages_at..].find(category_close) else {
        bail!("emoji canned_messages category has no closing Messages array");
    };
    let category_end = messages_at + close_offset + category_close.len();
    let replacement = mozc_emoji_category_xml(emoticons);

    let mut output = String::with_capacity(
        canned_messages.len() - (category_end - category_start) + replacement.len(),
    );
    output.push_str(&canned_messages[..category_start]);
    output.push_str(&replacement);
    output.push_str(&canned_messages[category_end..]);
    Ok((output, emoticons.len()))
}

fn mozc_emoji_category_xml(emoticons: &[String]) -> String {
    let mut category = String::from(
        "\n\t\t<dict>\n\
\t\t\t<key>Name</key>\n\
\t\t\t<dict>\n\
\t\t\t\t<key>en</key>\n\
\t\t\t\t<string>Emoji (Emoticons)</string>\n\
\t\t\t\t<key>zh_TW</key>\n\
\t\t\t\t<string>顏文字</string>\n\
\t\t\t</dict>\n\
\t\t\t<key>Messages</key>\n\
\t\t\t<array>\n",
    );
    for emoticon in emoticons {
        category.push_str("\t\t\t\t<string>");
        category.push_str(&xml_escape(emoticon));
        category.push_str("</string>\n");
    }
    category.push_str("\t\t\t</array>\n\t\t</dict>\n");
    category
}

fn validate_canned_messages_content(canned_messages: &str) -> Result<()> {
    if !canned_messages.contains("<key>CannedMessages</key>") {
        bail!("canned_messages is missing CannedMessages root key");
    }

    let categories = canned_message_categories(canned_messages);
    if categories.is_empty() {
        bail!("canned_messages has no categories");
    }

    let mut seen_supplemental_categories = HashSet::new();
    let mut emoji_message_count = None;

    for category in categories {
        let name = category_zh_tw_name(category);
        let is_button_list = category_is_symbol_button_list(category);
        let button_count = count_key_array_strings(category, "Buttons");
        let message_count = count_key_array_strings(category, "Messages");

        if button_count > 0 && message_count > 0 {
            bail!(
                "canned_messages category {} has both Buttons and Messages",
                name.as_deref().unwrap_or("<unknown>")
            );
        }
        if is_button_list && button_count == 0 {
            bail!(
                "button category {} is missing Buttons",
                name.as_deref().unwrap_or("<unknown>")
            );
        }
        if !is_button_list && message_count == 0 {
            bail!(
                "message category {} is missing Messages",
                name.as_deref().unwrap_or("<unknown>")
            );
        }
        if button_count > MAX_BUTTON_CATEGORY_ITEMS {
            bail!(
                "button category {} has too many items: {}",
                name.as_deref().unwrap_or("<unknown>"),
                button_count
            );
        }

        if let Some(name) = name.as_deref() {
            if REQUIRED_SUPPLEMENTAL_CATEGORIES.contains(&name) {
                seen_supplemental_categories.insert(name.to_string());
            }
            if name == "顏文字" {
                if is_button_list || button_count > 0 {
                    bail!("顏文字 category must be a Messages list, not a button list");
                }
                emoji_message_count = Some(message_count);
            }
        }
    }

    for required in REQUIRED_SUPPLEMENTAL_CATEGORIES {
        if !seen_supplemental_categories.contains(*required) {
            bail!("canned_messages is missing supplemental category {required}");
        }
    }

    let Some(emoji_message_count) = emoji_message_count else {
        bail!("canned_messages is missing 顏文字 category");
    };
    if emoji_message_count < MIN_MOZC_EMOTICON_ROWS {
        bail!("顏文字 category has too few messages: {emoji_message_count}");
    }
    for emoticon in REQUIRED_MOZC_EMOTICONS {
        let escaped = xml_escape(emoticon);
        if !canned_messages.contains(&format!("<string>{escaped}</string>")) {
            bail!("canned_messages is missing Mozc emoji sample {emoticon}");
        }
    }
    if canned_messages.contains("<string>(^(00)^) 豬</string>")
        || canned_messages.contains("<string>&lt;(￣ ﹌ ￣)&gt; 生氣")
    {
        bail!("canned_messages still contains original annotated emoji strings");
    }
    Ok(())
}

fn canned_message_categories(canned_messages: &str) -> Vec<&str> {
    let mut categories = Vec::new();
    let mut remaining = canned_messages;
    while let Some(start_offset) = remaining.find("\n\t\t<dict>") {
        remaining = &remaining[start_offset..];
        let Some(end_offset) = remaining.find("\n\t\t</dict>") else {
            break;
        };
        let end = end_offset + "\n\t\t</dict>".len();
        categories.push(&remaining[..end]);
        remaining = &remaining[end..];
    }
    categories
}

fn category_zh_tw_name(category: &str) -> Option<String> {
    let marker = "<key>zh_TW</key>";
    let after_marker = category
        .find(marker)
        .map(|index| &category[index + marker.len()..])?;
    extract_first_string(after_marker).map(xml_unescape)
}

fn category_is_symbol_button_list(category: &str) -> bool {
    let marker = "<key>IsSymbolButtonList</key>";
    let Some(after_marker) = category
        .find(marker)
        .map(|index| &category[index + marker.len()..])
    else {
        return false;
    };
    extract_first_string(after_marker).as_deref() == Some("true")
}

fn count_key_array_strings(category: &str, key: &str) -> usize {
    let marker = format!("<key>{key}</key>");
    let Some(after_key) = category
        .find(&marker)
        .map(|index| &category[index + marker.len()..])
    else {
        return 0;
    };
    let Some(array_start) = after_key.find("<array>") else {
        return 0;
    };
    let after_array = &after_key[array_start + "<array>".len()..];
    let Some(array_end) = after_array.find("</array>") else {
        return 0;
    };
    after_array[..array_end].matches("<string>").count()
}

fn extract_first_string(xml: &str) -> Option<String> {
    let start = xml.find("<string>")? + "<string>".len();
    let end = xml[start..].find("</string>")? + start;
    Some(xml[start..end].to_string())
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_unescape(text: String) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn require_service_row(conn: &Connection, key: &str, sql: &str) -> Result<()> {
    let count: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    if count == 0 {
        bail!("missing prepopulated_service_data/{key}");
    }
    Ok(())
}

fn require_punctuation_symbol(conn: &Connection, symbol: &str) -> Result<()> {
    let unigram_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unigrams
         WHERE qstring = '_punctuation_list' AND current = ?1",
        [symbol],
        |row| row.get(0),
    )?;
    let cin_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM 'Mandarin-bpmf-cin'
         WHERE key = '_punctuation_list' AND value = ?1",
        [symbol],
        |row| row.get(0),
    )?;
    if unigram_count == 0 || cin_count == 0 {
        bail!("missing supplemental punctuation symbol {symbol}");
    }
    Ok(())
}

fn forbid_service_row(conn: &Connection, key: &str, sql: &str) -> Result<()> {
    let count: i64 = conn.query_row(sql, [], |row| row.get(0))?;
    if count > 0 {
        bail!("obsolete prepopulated_service_data/{key} must not be shipped");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_payload, ServiceData};

    #[test]
    fn rejects_empty_service_payloads_and_zero_timestamp() {
        let data = ServiceData {
            canned_messages: String::new(),
            timestamp: 0,
            supplemental_symbol_count: 0,
            emoji_message_count: 0,
        };

        assert!(validate_payload(&data).is_err());
    }

    #[test]
    fn adds_supplemental_buttons_and_replaces_emoji_with_mozc_data() {
        let canned_messages = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>CannedMessages</key>
	<array>
		<dict>
			<key>Name</key>
			<dict>
				<key>en</key>
				<string>Emoji (Emoticons)</string>
				<key>zh_TW</key>
				<string>顏文字</string>
			</dict>
			<key>Messages</key>
			<array>
				<dict>
					<key>Name</key>
					<string>&lt;(￣ ﹌ ￣)&gt; 生氣</string>
					<key>Text</key>
					<string>&lt;(￣ ﹌ ￣)&gt; 生氣</string>
				</dict>
			</array>
		</dict>
	</array>
</dict>
</plist>"#;
        let mozc = vec!["＼(^o^)／".to_string(), "(T_T)".to_string()];

        let symbols = vec![
            super::SupplementalSymbol {
                symbol: "€".to_string(),
                tags: vec!["currency".to_string()],
            },
            super::SupplementalSymbol {
                symbol: "↔".to_string(),
                tags: vec!["arrow".to_string()],
            },
        ];
        let (with_symbols, symbol_count) =
            super::append_supplemental_symbol_buttons(canned_messages, &symbols).unwrap();
        let (with_buttons, emoji_count) =
            super::replace_emoji_category_with_mozc_messages(&with_symbols, &mozc).unwrap();

        assert_eq!(symbol_count, 2);
        assert_eq!(emoji_count, 2);
        assert!(with_buttons.contains("<string>貨幣與標記</string>"));
        assert!(with_buttons.contains("<string>補充箭頭</string>"));
        assert!(with_buttons.contains("<string>true</string>"));
        assert!(with_buttons.contains("<key>Buttons</key>"));
        assert!(with_buttons.contains("<key>Messages</key>"));
        assert!(with_buttons.contains("<string>＼(^o^)／</string>"));
        assert!(!with_buttons.contains("<key>IsSymbolButtonList</key>\n\t\t\t<string>true</string>\n\t\t\t<key>Buttons</key>\n\t\t\t<array>\n\t\t\t\t<string>＼(^o^)／</string>"));
        assert!(!with_buttons.contains("<string>&lt;(￣ ﹌ ￣)&gt; 生氣</string>"));
    }

    #[test]
    fn validates_canned_message_category_shapes() {
        let valid = valid_canned_messages_fixture();
        assert!(super::validate_canned_messages_content(&valid).is_ok());

        let mixed = valid.replace(
            "<string>補充標點-0</string>\n\t\t\t</array>",
            "<string>補充標點-0</string>\n\t\t\t</array>\n\t\t\t<key>Messages</key>\n\t\t\t<array>\n\t\t\t\t<string>bad</string>\n\t\t\t</array>",
        );
        assert!(super::validate_canned_messages_content(&mixed).is_err());

        let emoji_as_buttons = valid.replace(
            "\t\t\t<key>Messages</key>\n\t\t\t<array>\n\t\t\t\t<string>＼(^o^)／</string>",
            "\t\t\t<key>IsSymbolButtonList</key>\n\t\t\t<string>true</string>\n\t\t\t<key>Buttons</key>\n\t\t\t<array>\n\t\t\t\t<string>＼(^o^)／</string>",
        );
        assert!(super::validate_canned_messages_content(&emoji_as_buttons).is_err());

        let oversized = valid.replace(
            "<string>補充箭頭-0</string>",
            &(0..101)
                .map(|index| format!("<string>補充箭頭-{index}</string>"))
                .collect::<Vec<_>>()
                .join("\n\t\t\t\t"),
        );
        assert!(super::validate_canned_messages_content(&oversized).is_err());
    }

    fn valid_canned_messages_fixture() -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<plist version=\"1.0\">\n\
<dict>\n\
\t<key>CannedMessages</key>\n\
\t<array>",
        );
        xml.push_str(&message_category_fixture(
            "顏文字",
            emoji_messages_fixture(),
        ));
        for name in super::REQUIRED_SUPPLEMENTAL_CATEGORIES {
            xml.push_str(&button_category_fixture(name, &[format!("{name}-0")]));
        }
        xml.push_str("\n\t</array>\n</dict>\n</plist>");
        xml
    }

    fn emoji_messages_fixture() -> Vec<String> {
        let mut messages = super::REQUIRED_MOZC_EMOTICONS
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        while messages.len() < super::MIN_MOZC_EMOTICON_ROWS {
            messages.push(format!("顔文字{}", messages.len()));
        }
        messages
    }

    fn message_category_fixture(name: &str, messages: Vec<String>) -> String {
        let mut xml = category_name_fixture(name);
        xml.push_str("\t\t\t<key>Messages</key>\n\t\t\t<array>\n");
        for message in messages {
            xml.push_str("\t\t\t\t<string>");
            xml.push_str(&super::xml_escape(&message));
            xml.push_str("</string>\n");
        }
        xml.push_str("\t\t\t</array>\n\t\t</dict>\n");
        xml
    }

    fn button_category_fixture(name: &str, buttons: &[String]) -> String {
        let mut xml = category_name_fixture(name);
        xml.push_str(
            "\t\t\t<key>IsSymbolButtonList</key>\n\
\t\t\t<string>true</string>\n\
\t\t\t<key>Buttons</key>\n\
\t\t\t<array>\n",
        );
        for button in buttons {
            xml.push_str("\t\t\t\t<string>");
            xml.push_str(&super::xml_escape(button));
            xml.push_str("</string>\n");
        }
        xml.push_str("\t\t\t</array>\n\t\t</dict>\n");
        xml
    }

    fn category_name_fixture(name: &str) -> String {
        format!(
            "\n\t\t<dict>\n\
\t\t\t<key>Name</key>\n\
\t\t\t<dict>\n\
\t\t\t\t<key>zh_TW</key>\n\
\t\t\t\t<string>{}</string>\n\
\t\t\t</dict>\n",
            super::xml_escape(name)
        )
    }
}
