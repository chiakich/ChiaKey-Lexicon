use crate::config::{
    Config, BONEYARD_SOURCE_ID, BONEYARD_SOURCE_NAME, BPMF_EXT_SOURCE_ID, BPMF_EXT_SOURCE_NAME,
    CHIAKEY_AUTO_HOTWORDS_SOURCE_ID, CHIAKEY_AUTO_HOTWORDS_SOURCE_NAME, CHIAKI_SYNTHETIC_SOURCE_ID,
    CHIAKI_SYNTHETIC_SOURCE_NAME, CHIAKI_WEB_OVERLAY_SOURCE_ID, CHIAKI_WEB_OVERLAY_SOURCE_NAME,
    DATABASE_SCHEMA_VERSION, FRAGMENT_DENYLIST_SOURCE_ID, FRAGMENT_DENYLIST_SOURCE_NAME,
    LIBCHEWING_SOURCE_ID, LIBCHEWING_SOURCE_NAME, MODULE_CIN_SOURCE_ID, MODULE_CIN_SOURCE_NAME,
    MOZC_EMOTICON_SOURCE_ID, MOZC_EMOTICON_SOURCE_NAME, OPENFORMOSA_COMMON_VOICE_SOURCE_ID,
    OPENFORMOSA_COMMON_VOICE_SOURCE_NAME, OVERLAY_SOURCE_ID, OVERLAY_SOURCE_NAME,
    PREPOPULATED_SERVICE_SOURCE_ID, PREPOPULATED_SERVICE_SOURCE_NAME, PUNCTUATION_SOURCE_ID,
    PUNCTUATION_SOURCE_NAME, RIME_CONVERSION_SOURCE_ID, RIME_CONVERSION_SOURCE_NAME,
    RIME_ESSAY_SOURCE_ID, RIME_ESSAY_SOURCE_NAME, SYMBOL_OVERLAY_SOURCE_ID,
    SYMBOL_OVERLAY_SOURCE_NAME,
};
use crate::db;
use crate::files::{file_info, relative_to, sha256_bytes, sha256_file};
use crate::paths::ReleasePaths;
use crate::types::FileInfo;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub fn release_metadata(
    cfg: &Config,
    paths: &ReleasePaths,
    metadata: &BTreeMap<String, Value>,
    counts: &Value,
    source_rows: &[Value],
    db_info: &FileInfo,
    normalized_info: &FileInfo,
) -> Result<Value> {
    let sources = vec![
        release_source_from_inventory(
            BONEYARD_SOURCE_ID,
            BONEYARD_SOURCE_NAME,
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.boneyard_inventory,
            db::stats_for_source_rows(source_rows, "YahooKeyKey-Source-1.1.2528/"),
        )?,
        release_source_from_inventory(
            PUNCTUATION_SOURCE_ID,
            PUNCTUATION_SOURCE_NAME,
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.punctuation_inventory,
            db::stats_for_source_rows(
                source_rows,
                "sources/keykey-punctuations-cin/vendor/bpmf-punctuations.cin",
            ),
        )?,
        release_source_from_files(
            SYMBOL_OVERLAY_SOURCE_ID,
            SYMBOL_OVERLAY_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.symbol_overlay_source_dir,
            &[
                &paths.symbol_overlay_symbols,
                &paths.symbol_overlay_alternatives,
            ],
            db::stats_for_source_rows(source_rows, "sources/chiakey-symbols-overlay/"),
        )?,
        release_source_from_inventory(
            PREPOPULATED_SERVICE_SOURCE_ID,
            PREPOPULATED_SERVICE_SOURCE_NAME,
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.prepopulated_service_inventory,
            db::stats_for_source_rows(
                source_rows,
                "sources/keykey-prepopulated-service-data/vendor/",
            ),
        )?,
        release_source_from_inventory(
            MOZC_EMOTICON_SOURCE_ID,
            MOZC_EMOTICON_SOURCE_NAME,
            "BSD-3-Clause",
            "Google and Mozc contributors",
            &paths.mozc_emoticon_inventory,
            db::stats_for_source_rows(source_rows, "sources/mozc-emoticon-data/raw/"),
        )?,
        release_source_from_inventory(
            MODULE_CIN_SOURCE_ID,
            MODULE_CIN_SOURCE_NAME,
            "BSD-3-Clause-style / Public Domain source tables",
            "Yahoo! Inc.; OpenVanilla contributors; opendesktop.org.tw CIN contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.module_cin_inventory,
            db::stats_for_source_rows(source_rows, "sources/keykey-module-cin/vendor/"),
        )?,
        release_source_from_inventory(
            LIBCHEWING_SOURCE_ID,
            LIBCHEWING_SOURCE_NAME,
            "LGPL-2.1-or-later",
            "libchewing Core Team",
            &paths.libchewing_inventory,
            db::stats_for_source_rows(source_rows, "sources/libchewing-data/raw/"),
        )?,
        release_source_from_inventory(
            BPMF_EXT_SOURCE_ID,
            BPMF_EXT_SOURCE_NAME,
            "Public Domain",
            "opendesktop.org.tw phone.cin contributors; KeyKey Boneyard maintainers",
            &paths.bpmf_ext_inventory,
            db::stats_for_source_rows(source_rows, "sources/bpmf-ext-cin/vendor/bpmf-ext.cin"),
        )?,
        release_source_from_inventory(
            RIME_ESSAY_SOURCE_ID,
            RIME_ESSAY_SOURCE_NAME,
            "LGPL-3.0",
            "Rime essay contributors",
            &paths.rime_essay_inventory,
            db::stats_for_source_rows(source_rows, "sources/rime-essay/raw/"),
        )?,
        release_source_from_files(
            RIME_CONVERSION_SOURCE_ID,
            RIME_CONVERSION_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.rime_conversion_source_dir,
            &[&paths.rime_conversion_replacements],
            db::stats_for_source_rows(source_rows, "sources/chiakey-rime-conversion-policy/"),
        )?,
        release_source_from_files(
            OVERLAY_SOURCE_ID,
            OVERLAY_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.overlay_source_dir,
            &[&paths.overlay_phrases, &paths.overlay_explicit],
            db::stats_for_source_rows(source_rows, "sources/chiakey-modern-overlay/"),
        )?,
        release_source_from_files(
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
            CHIAKI_WEB_OVERLAY_SOURCE_NAME,
            "CC BY-NC 4.0; commercial use requires permission from Chiaki.C",
            "Chiaki.C",
            &paths.chiaki_web_overlay_source_dir,
            &[&paths.chiaki_web_overlay_explicit, &paths.chiaki_web_overlay_bigrams],
            db::stats_for_source_rows(source_rows, "sources/chiaki-web-overlay/"),
        )?,
        release_source_from_files(
            CHIAKI_SYNTHETIC_SOURCE_ID,
            CHIAKI_SYNTHETIC_SOURCE_NAME,
            "CC BY-NC 4.0; commercial use requires permission from Chiaki.C",
            "Chiaki.C",
            &paths.chiaki_synthetic_source_dir,
            &[&paths.chiaki_synthetic_unigrams, &paths.chiaki_synthetic_bigrams],
            db::stats_for_source_rows(source_rows, "sources/chiaki-synthetic-overlay/"),
        )?,
        release_source_from_files(
            CHIAKEY_AUTO_HOTWORDS_SOURCE_ID,
            CHIAKEY_AUTO_HOTWORDS_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.chiakey_auto_hotwords_source_dir,
            &[&paths.chiakey_auto_hotwords_phrases, &paths.chiakey_auto_hotwords_state],
            db::stats_for_source_rows(source_rows, "sources/chiakey-auto-hotwords-overlay/"),
        )?,
        release_source_from_files(
            OPENFORMOSA_COMMON_VOICE_SOURCE_ID,
            OPENFORMOSA_COMMON_VOICE_SOURCE_NAME,
            "CC0-1.0",
            "OpenFormosa / Mozilla Common Voice contributors",
            &paths.openformosa_common_voice_source_dir,
            &[&paths.openformosa_common_voice_bigrams],
            db::stats_for_source_rows(source_rows, "sources/openformosa-common-voice-25-zh-tw/"),
        )?,
        release_source_from_files(
            FRAGMENT_DENYLIST_SOURCE_ID,
            FRAGMENT_DENYLIST_SOURCE_NAME,
            "Self-authored (MOE revised dict used as offline review tool only)",
            "ChiaKey Lexicon maintainers",
            &paths.fragment_denylist_source_dir,
            &[&paths.fragment_demotions],
            db::stats_for_source_rows(source_rows, "sources/chiakey-fragment-denylist/"),
        )?,
    ];

    Ok(json!({
        "schema": 1,
        "version": cfg.release_version,
        "generated_at": cfg.generated_at,
        "language_model_version": cfg.language_model_version,
        "database_schema_version": DATABASE_SCHEMA_VERSION,
        "database": {
            "filename": paths.db_filename,
            "sha256": db_info.sha256,
            "size": db_info.size,
            "metadata": metadata,
            "counts": counts
        },
        "normalized": {
            "path": "normalized/smart-mandarin.tsv",
            "sha256": normalized_info.sha256,
            "size": normalized_info.size,
            "rows": counts.get("normalized_rows").and_then(Value::as_i64).unwrap_or_default(),
            "format": "reading<TAB>phrase<TAB>weight<TAB>source_id<TAB>tags"
        },
        "sources": sources
    }))
}

pub fn manifest(
    cfg: &Config,
    paths: &ReleasePaths,
    db_info: &FileInfo,
    metadata_info: &FileInfo,
    checksum_info: &FileInfo,
) -> Result<Value> {
    let sources = vec![
        manifest_source_from_inventory(
            BONEYARD_SOURCE_ID,
            BONEYARD_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard",
            "sqlite",
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.boneyard_inventory,
            100,
        )?,
        manifest_source_from_inventory(
            PUNCTUATION_SOURCE_ID,
            PUNCTUATION_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-punctuations.cin",
            "cin",
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.punctuation_inventory,
            120,
        )?,
        manifest_source_from_files(
            SYMBOL_OVERLAY_SOURCE_ID,
            SYMBOL_OVERLAY_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiakey-symbols-overlay",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.symbol_overlay_source_dir,
            &[
                &paths.symbol_overlay_symbols,
                &paths.symbol_overlay_alternatives,
            ],
            125,
        )?,
        manifest_source_from_inventory(
            PREPOPULATED_SERVICE_SOURCE_ID,
            PREPOPULATED_SERVICE_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/Distributions/Takao/OnlineData",
            "plist",
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.prepopulated_service_inventory,
            130,
        )?,
        manifest_source_from_inventory(
            MOZC_EMOTICON_SOURCE_ID,
            MOZC_EMOTICON_SOURCE_NAME,
            "https://github.com/google/mozc/tree/28da5a39f9a7fd70251c85d269f4a8b47aa31cf8/src/data/emoticon",
            "tsv",
            "BSD-3-Clause",
            "Google and Mozc contributors",
            &paths.mozc_emoticon_inventory,
            135,
        )?,
        manifest_source_from_inventory(
            MODULE_CIN_SOURCE_ID,
            MODULE_CIN_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/DataTables",
            "cin",
            "BSD-3-Clause-style / Public Domain source tables",
            "Yahoo! Inc.; OpenVanilla contributors; opendesktop.org.tw CIN contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.module_cin_inventory,
            140,
        )?,
        manifest_source_from_inventory(
            LIBCHEWING_SOURCE_ID,
            LIBCHEWING_SOURCE_NAME,
            "https://github.com/chewing/libchewing-data/releases/tag/v2026.3.22",
            "csv",
            "LGPL-2.1-or-later",
            "libchewing Core Team",
            &paths.libchewing_inventory,
            250,
        )?,
        manifest_source_from_inventory(
            BPMF_EXT_SOURCE_ID,
            BPMF_EXT_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-ext.cin",
            "cin",
            "Public Domain",
            "opendesktop.org.tw phone.cin contributors; KeyKey Boneyard maintainers",
            &paths.bpmf_ext_inventory,
            180,
        )?,
        manifest_source_from_inventory(
            RIME_ESSAY_SOURCE_ID,
            RIME_ESSAY_SOURCE_NAME,
            "https://github.com/rime/rime-essay/tree/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed",
            "text",
            "LGPL-3.0",
            "Rime essay contributors",
            &paths.rime_essay_inventory,
            220,
        )?,
        manifest_source_from_files(
            RIME_CONVERSION_SOURCE_ID,
            RIME_CONVERSION_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiakey-rime-conversion-policy",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.rime_conversion_source_dir,
            &[&paths.rime_conversion_replacements],
            225,
        )?,
        manifest_source_from_files(
            OVERLAY_SOURCE_ID,
            OVERLAY_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiakey-modern-overlay",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.overlay_source_dir,
            &[&paths.overlay_phrases, &paths.overlay_explicit],
            300,
        )?,
        manifest_source_from_files(
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
            CHIAKI_WEB_OVERLAY_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiaki-web-overlay",
            "tsv",
            "CC BY-NC 4.0; commercial use requires permission from Chiaki.C",
            "Chiaki.C",
            &paths.chiaki_web_overlay_source_dir,
            &[&paths.chiaki_web_overlay_explicit, &paths.chiaki_web_overlay_bigrams],
            305,
        )?,
        manifest_source_from_files(
            CHIAKI_SYNTHETIC_SOURCE_ID,
            CHIAKI_SYNTHETIC_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiaki-synthetic-overlay",
            "tsv",
            "CC BY-NC 4.0; commercial use requires permission from Chiaki.C",
            "Chiaki.C",
            &paths.chiaki_synthetic_source_dir,
            &[&paths.chiaki_synthetic_unigrams, &paths.chiaki_synthetic_bigrams],
            306,
        )?,
        manifest_source_from_files(
            CHIAKEY_AUTO_HOTWORDS_SOURCE_ID,
            CHIAKEY_AUTO_HOTWORDS_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiakey-auto-hotwords-overlay",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.chiakey_auto_hotwords_source_dir,
            &[&paths.chiakey_auto_hotwords_phrases, &paths.chiakey_auto_hotwords_state],
            308,
        )?,
        manifest_source_from_files(
            OPENFORMOSA_COMMON_VOICE_SOURCE_ID,
            OPENFORMOSA_COMMON_VOICE_SOURCE_NAME,
            "https://huggingface.co/datasets/OpenFormosa/common_voice_25_zh-TW",
            "tsv",
            "CC0-1.0",
            "OpenFormosa / Mozilla Common Voice contributors",
            &paths.openformosa_common_voice_source_dir,
            &[&paths.openformosa_common_voice_bigrams],
            307,
        )?,
        manifest_source_from_files(
            FRAGMENT_DENYLIST_SOURCE_ID,
            FRAGMENT_DENYLIST_SOURCE_NAME,
            "https://language.moe.gov.tw/001/Upload/Files/site_content/M0001/respub/index.html",
            "tsv",
            "Self-authored (MOE revised dict used as offline review tool only)",
            "ChiaKey Lexicon maintainers",
            &paths.fragment_denylist_source_dir,
            &[&paths.fragment_demotions],
            311,
        )?,
    ];

    Ok(json!({
        "schema": 1,
        "version": cfg.release_version,
        "generated_at": cfg.generated_at,
        "minimum_app_version": cfg.minimum_app_version,
        "database_schema_version": DATABASE_SCHEMA_VERSION,
        "sources": sources,
        "artifacts": [
            artifact_json("smart-mandarin-db", "chiakey-source-db", &cfg.release_base_url, &paths.db_filename, db_info, &cfg.language_model_version),
            artifact_json("smart-mandarin-metadata", "metadata", &cfg.release_base_url, &paths.metadata_filename, metadata_info, &cfg.language_model_version),
            artifact_json("smart-mandarin-checksums", "checksum", &cfg.release_base_url, "SHA256SUMS", checksum_info, &cfg.language_model_version)
        ]
    }))
}

fn release_source_from_inventory(
    id: &str,
    name: &str,
    license: &str,
    attribution: &str,
    inventory_path: &Path,
    stats: Vec<Value>,
) -> Result<Value> {
    let info = file_info(inventory_path)?;
    Ok(json!({
        "id": id,
        "name": name,
        "license": license,
        "attribution": attribution,
        "inventory": {
            "path": repo_inventory_path(inventory_path),
            "sha256": info.sha256,
            "size": info.size
        },
        "stats": stats
    }))
}

fn release_source_from_files(
    id: &str,
    name: &str,
    license: &str,
    attribution: &str,
    source_root: &Path,
    source_files: &[&Path],
    stats: Vec<Value>,
) -> Result<Value> {
    let info = virtual_inventory_file_info(source_root, source_files)?;
    Ok(json!({
        "id": id,
        "name": name,
        "license": license,
        "attribution": attribution,
        "inventory": {
            "path": virtual_inventory_path(source_root),
            "sha256": info.sha256,
            "size": info.size
        },
        "stats": stats
    }))
}

fn manifest_source_from_inventory(
    id: &str,
    name: &str,
    url: &str,
    format: &str,
    license: &str,
    attribution: &str,
    inventory_path: &Path,
    priority: i64,
) -> Result<Value> {
    Ok(json!({
        "id": id,
        "name": name,
        "url": url,
        "format": format,
        "license": license,
        "attribution": attribution,
        "sha256": sha256_file(inventory_path)?,
        "enabled": true,
        "priority": priority
    }))
}

fn manifest_source_from_files(
    id: &str,
    name: &str,
    url: &str,
    format: &str,
    license: &str,
    attribution: &str,
    source_root: &Path,
    source_files: &[&Path],
    priority: i64,
) -> Result<Value> {
    let info = virtual_inventory_file_info(source_root, source_files)?;
    Ok(json!({
        "id": id,
        "name": name,
        "url": url,
        "format": format,
        "license": license,
        "attribution": attribution,
        "sha256": info.sha256,
        "enabled": true,
        "priority": priority
    }))
}

fn artifact_json(
    id: &str,
    kind: &str,
    release_base_url: &str,
    filename: &str,
    info: &FileInfo,
    language_model_version: &str,
) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "url": format!("{release_base_url}/{filename}"),
        "filename": filename,
        "sha256": info.sha256,
        "size": info.size,
        "database_schema_version": DATABASE_SCHEMA_VERSION,
        "language_model_version": language_model_version
    })
}

fn repo_inventory_path(path: &Path) -> String {
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let start = parts.iter().position(|part| part == "sources").unwrap_or(0);
    parts[start..].join("/")
}

fn virtual_inventory_path(source_root: &Path) -> String {
    format!(
        "{}/source-inventory.virtual.sha256",
        repo_inventory_path(source_root)
    )
}

fn virtual_inventory_file_info(source_root: &Path, source_files: &[&Path]) -> Result<FileInfo> {
    let mut lines = source_files
        .iter()
        .map(|path| {
            let rel = relative_to(path, source_root)?;
            Ok((rel.clone(), format!("{}  {}", sha256_file(path)?, rel)))
        })
        .collect::<Result<Vec<_>>>()?;
    lines.sort_by(|left, right| left.0.cmp(&right.0));
    let text = format!(
        "{}\n",
        lines
            .into_iter()
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n")
    );

    Ok(FileInfo {
        sha256: sha256_bytes(text.as_bytes()),
        size: text.len() as u64,
    })
}
