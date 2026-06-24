use crate::types::KeyValueRecord;
use anyhow::{bail, Context, Result};
use rusqlite::Connection;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn parse_cin(path: &Path) -> Result<(Vec<KeyValueRecord>, usize, usize)> {
    let file = File::open(path).with_context(|| format!("read {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    let mut seen = 0;
    let mut skipped = 0;
    let mut in_chardef = false;
    let mut in_keyname = false;

    for line in reader.lines() {
        let line = line?;
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("%chardef") && lower.ends_with("begin") {
            in_chardef = true;
            in_keyname = false;
            continue;
        }
        if lower.starts_with("%chardef") && lower.ends_with("end") {
            in_chardef = false;
            continue;
        }
        if lower.starts_with("%keyname") && lower.ends_with("begin") {
            in_keyname = true;
            in_chardef = false;
            continue;
        }
        if lower.starts_with("%keyname") && lower.ends_with("end") {
            in_keyname = false;
            continue;
        }

        if in_chardef {
            seen += 1;
            match parse_key_value(trimmed, "") {
                Some(row) => rows.push(row),
                None => skipped += 1,
            }
        } else if in_keyname {
            seen += 1;
            match parse_key_value(trimmed, "__property_keyname-") {
                Some(row) => rows.push(row),
                None => skipped += 1,
            }
        } else if let Some(property) = trimmed.strip_prefix('%') {
            seen += 1;
            match parse_key_value(property, "__property_") {
                Some(row) => rows.push(row),
                None => skipped += 1,
            }
        }
    }

    Ok((rows, seen, skipped))
}

pub fn validate_runtime_required_data(conn: &Connection) -> Result<()> {
    require_min_rows(conn, "Generic-cj-cin", 70_000)?;
    require_min_rows(conn, "Generic-simplex-cin", 55_000)?;
    require_min_rows(conn, "Punctuations-cj-halfwidth-cin", 30)?;
    require_min_rows(conn, "Punctuations-cj-mixedwidth-cin", 30)?;
    require_min_rows(conn, "BopomofoCorrection-bopomofo-correction-cin", 100)?;

    require_pair(conn, "Generic-cj-cin", "__property_cname", "倉頡（大字集）")?;
    require_pair(
        conn,
        "Generic-simplex-cin",
        "__property_cname",
        "簡易（大字集）",
    )?;
    require_pair(
        conn,
        "BopomofoCorrection-bopomofo-correction-cin",
        "__property_ename",
        "Zhuyinwen Reverse Lookup",
    )?;
    require_pair(conn, "Generic-cj-cin", ",", "，")?;
    require_pair(conn, "Generic-simplex-cin", ",", "，")?;
    require_pair(conn, "Punctuations-cj-halfwidth-cin", ",", ",")?;
    require_pair(conn, "Punctuations-cj-mixedwidth-cin", ",", "，")?;
    require_pair(
        conn,
        "BopomofoCorrection-bopomofo-correction-cin",
        "ㄅ",
        "不",
    )?;
    require_pair(
        conn,
        "BopomofoCorrection-bopomofo-correction-cin",
        "ㄏ",
        "呵",
    )?;
    require_pair(
        conn,
        "BopomofoCorrection-bopomofo-correction-cin",
        "ㄏㄏ",
        "呵呵",
    )?;

    Ok(())
}

fn require_min_rows(conn: &Connection, table_name: &str, min_rows: i64) -> Result<()> {
    let count: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM {}", quote_identifier(table_name)),
        [],
        |row| row.get(0),
    )?;
    if count < min_rows {
        bail!("{table_name} has {count} rows, expected at least {min_rows}");
    }
    Ok(())
}

fn require_pair(conn: &Connection, table_name: &str, key: &str, value: &str) -> Result<()> {
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM {} WHERE key = ?1 AND value = ?2",
            quote_identifier(table_name)
        ),
        [key, value],
        |row| row.get(0),
    )?;
    if count == 0 {
        bail!("{table_name} is missing {key} -> {value}");
    }
    Ok(())
}

fn parse_key_value(line: &str, prefix: &str) -> Option<KeyValueRecord> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let split_at = line.find(char::is_whitespace);
    let (key, value) = match split_at {
        Some(index) => (&line[..index], line[index..].trim()),
        None => (line, ""),
    };
    if key.is_empty() {
        return None;
    }

    Some(KeyValueRecord {
        key: format!("{prefix}{key}"),
        value: value.to_string(),
    })
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::parse_key_value;

    #[test]
    fn parses_module_cin_header_and_rows() {
        let property = parse_key_value("ename CangJie (Large Character Set)", "__property_")
            .expect("property row");
        assert_eq!(property.key, "__property_ename");
        assert_eq!(property.value, "CangJie (Large Character Set)");

        let keyname = parse_key_value("a 日", "__property_keyname-").expect("keyname row");
        assert_eq!(keyname.key, "__property_keyname-a");
        assert_eq!(keyname.value, "日");

        let chardef = parse_key_value("ㄅ 不", "").expect("chardef row");
        assert_eq!(chardef.key, "ㄅ");
        assert_eq!(chardef.value, "不");
    }
}
