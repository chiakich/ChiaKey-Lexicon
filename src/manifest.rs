use crate::config::{
    Config, BONEYARD_SOURCE_ID, BONEYARD_SOURCE_NAME, BPMF_EXT_SOURCE_ID, BPMF_EXT_SOURCE_NAME,
    CHIAKI_SYNTHETIC_SOURCE_ID, CHIAKI_SYNTHETIC_SOURCE_NAME, CHIAKI_WEB_OVERLAY_SOURCE_ID,
    CHIAKI_WEB_OVERLAY_SOURCE_NAME, DATABASE_SCHEMA_VERSION, LIBCHEWING_SOURCE_ID,
    LIBCHEWING_SOURCE_NAME, MODULE_CIN_SOURCE_ID, MODULE_CIN_SOURCE_NAME, MOZC_EMOTICON_SOURCE_ID,
    MOZC_EMOTICON_SOURCE_NAME, OPENCC_VARIANT_SOURCE_ID, OPENCC_VARIANT_SOURCE_NAME,
    OPENFORMOSA_COMMON_VOICE_SOURCE_ID, OPENFORMOSA_COMMON_VOICE_SOURCE_NAME, OVERLAY_SOURCE_ID,
    OVERLAY_SOURCE_NAME, PREPOPULATED_SERVICE_SOURCE_ID, PREPOPULATED_SERVICE_SOURCE_NAME,
    PUNCTUATION_SOURCE_ID, PUNCTUATION_SOURCE_NAME, RIME_ESSAY_SOURCE_ID, RIME_ESSAY_SOURCE_NAME,
    SYMBOL_OVERLAY_SOURCE_ID, SYMBOL_OVERLAY_SOURCE_NAME,
};
use crate::db;
use crate::files::{file_info, sha256_file};
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
        release_source(
            BONEYARD_SOURCE_ID,
            BONEYARD_SOURCE_NAME,
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.boneyard_inventory,
            db::stats_for_source_rows(source_rows, "YahooKeyKey-Source-1.1.2528/"),
        )?,
        release_source(
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
        release_source(
            SYMBOL_OVERLAY_SOURCE_ID,
            SYMBOL_OVERLAY_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.symbol_overlay_inventory,
            db::stats_for_source_rows(source_rows, "sources/chiakey-symbols-overlay/"),
        )?,
        release_source(
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
        release_source(
            MOZC_EMOTICON_SOURCE_ID,
            MOZC_EMOTICON_SOURCE_NAME,
            "BSD-3-Clause",
            "Google and Mozc contributors",
            &paths.mozc_emoticon_inventory,
            db::stats_for_source_rows(source_rows, "sources/mozc-emoticon-data/raw/"),
        )?,
        release_source(
            MODULE_CIN_SOURCE_ID,
            MODULE_CIN_SOURCE_NAME,
            "BSD-3-Clause-style / Public Domain source tables",
            "Yahoo! Inc.; OpenVanilla contributors; opendesktop.org.tw CIN contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.module_cin_inventory,
            db::stats_for_source_rows(source_rows, "sources/keykey-module-cin/vendor/"),
        )?,
        release_source(
            LIBCHEWING_SOURCE_ID,
            LIBCHEWING_SOURCE_NAME,
            "LGPL-2.1-or-later",
            "libchewing Core Team",
            &paths.libchewing_inventory,
            db::stats_for_source_rows(source_rows, "sources/libchewing-data/raw/"),
        )?,
        release_source(
            BPMF_EXT_SOURCE_ID,
            BPMF_EXT_SOURCE_NAME,
            "Public Domain",
            "opendesktop.org.tw phone.cin contributors; KeyKey Boneyard maintainers",
            &paths.bpmf_ext_inventory,
            db::stats_for_source_rows(source_rows, "sources/bpmf-ext-cin/vendor/bpmf-ext.cin"),
        )?,
        release_source(
            RIME_ESSAY_SOURCE_ID,
            RIME_ESSAY_SOURCE_NAME,
            "LGPL-3.0",
            "Rime essay contributors",
            &paths.rime_essay_inventory,
            db::stats_for_source_rows(source_rows, "sources/rime-essay/raw/"),
        )?,
        release_source(
            OVERLAY_SOURCE_ID,
            OVERLAY_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.overlay_inventory,
            db::stats_for_source_rows(source_rows, "sources/chiakey-modern-overlay/"),
        )?,
        release_source(
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
            CHIAKI_WEB_OVERLAY_SOURCE_NAME,
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.chiaki_web_overlay_inventory,
            db::stats_for_source_rows(source_rows, "sources/chiaki-web-overlay/"),
        )?,
        release_source(
            CHIAKI_SYNTHETIC_SOURCE_ID,
            CHIAKI_SYNTHETIC_SOURCE_NAME,
            "CC BY-NC 4.0; commercial use requires permission from Chiaki.C",
            "Chiaki.C",
            &paths.chiaki_synthetic_inventory,
            db::stats_for_source_rows(source_rows, "sources/chiaki-synthetic-overlay/"),
        )?,
        release_source(
            OPENFORMOSA_COMMON_VOICE_SOURCE_ID,
            OPENFORMOSA_COMMON_VOICE_SOURCE_NAME,
            "CC0-1.0",
            "OpenFormosa / Mozilla Common Voice contributors",
            &paths.openformosa_common_voice_inventory,
            db::stats_for_source_rows(source_rows, "sources/openformosa-common-voice-25-zh-tw/"),
        )?,
        release_source(
            OPENCC_VARIANT_SOURCE_ID,
            OPENCC_VARIANT_SOURCE_NAME,
            "Apache-2.0-derived policy",
            "OpenCC contributors; ChiaKey Lexicon maintainers",
            &paths.opencc_variant_inventory,
            db::stats_for_source_rows(source_rows, "sources/opencc-variant-policy/"),
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
        manifest_source(
            BONEYARD_SOURCE_ID,
            BONEYARD_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard",
            "sqlite",
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.boneyard_inventory,
            100,
        )?,
        manifest_source(
            PUNCTUATION_SOURCE_ID,
            PUNCTUATION_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-punctuations.cin",
            "cin",
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.punctuation_inventory,
            120,
        )?,
        manifest_source(
            SYMBOL_OVERLAY_SOURCE_ID,
            SYMBOL_OVERLAY_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiakey-symbols-overlay",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.symbol_overlay_inventory,
            125,
        )?,
        manifest_source(
            PREPOPULATED_SERVICE_SOURCE_ID,
            PREPOPULATED_SERVICE_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/Distributions/Takao/OnlineData",
            "plist",
            "BSD-3-Clause-style",
            "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.prepopulated_service_inventory,
            130,
        )?,
        manifest_source(
            MOZC_EMOTICON_SOURCE_ID,
            MOZC_EMOTICON_SOURCE_NAME,
            "https://github.com/google/mozc/tree/28da5a39f9a7fd70251c85d269f4a8b47aa31cf8/src/data/emoticon",
            "tsv",
            "BSD-3-Clause",
            "Google and Mozc contributors",
            &paths.mozc_emoticon_inventory,
            135,
        )?,
        manifest_source(
            MODULE_CIN_SOURCE_ID,
            MODULE_CIN_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/DataTables",
            "cin",
            "BSD-3-Clause-style / Public Domain source tables",
            "Yahoo! Inc.; OpenVanilla contributors; opendesktop.org.tw CIN contributors; KeyKey Boneyard / ChiaKey maintainers",
            &paths.module_cin_inventory,
            140,
        )?,
        manifest_source(
            LIBCHEWING_SOURCE_ID,
            LIBCHEWING_SOURCE_NAME,
            "https://github.com/chewing/libchewing-data/releases/tag/v2026.3.22",
            "csv",
            "LGPL-2.1-or-later",
            "libchewing Core Team",
            &paths.libchewing_inventory,
            250,
        )?,
        manifest_source(
            BPMF_EXT_SOURCE_ID,
            BPMF_EXT_SOURCE_NAME,
            "https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-ext.cin",
            "cin",
            "Public Domain",
            "opendesktop.org.tw phone.cin contributors; KeyKey Boneyard maintainers",
            &paths.bpmf_ext_inventory,
            180,
        )?,
        manifest_source(
            RIME_ESSAY_SOURCE_ID,
            RIME_ESSAY_SOURCE_NAME,
            "https://github.com/rime/rime-essay/tree/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed",
            "text",
            "LGPL-3.0",
            "Rime essay contributors",
            &paths.rime_essay_inventory,
            220,
        )?,
        manifest_source(
            OVERLAY_SOURCE_ID,
            OVERLAY_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiakey-modern-overlay",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.overlay_inventory,
            300,
        )?,
        manifest_source(
            CHIAKI_WEB_OVERLAY_SOURCE_ID,
            CHIAKI_WEB_OVERLAY_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiaki-web-overlay",
            "tsv",
            "CC0-1.0",
            "ChiaKey Lexicon maintainers",
            &paths.chiaki_web_overlay_inventory,
            305,
        )?,
        manifest_source(
            CHIAKI_SYNTHETIC_SOURCE_ID,
            CHIAKI_SYNTHETIC_SOURCE_NAME,
            "https://github.com/akira02/ChiaKey-Lexicon/tree/main/sources/chiaki-synthetic-overlay",
            "tsv",
            "CC BY-NC 4.0; commercial use requires permission from Chiaki.C",
            "Chiaki.C",
            &paths.chiaki_synthetic_inventory,
            306,
        )?,
        manifest_source(
            OPENFORMOSA_COMMON_VOICE_SOURCE_ID,
            OPENFORMOSA_COMMON_VOICE_SOURCE_NAME,
            "https://huggingface.co/datasets/OpenFormosa/common_voice_25_zh-TW",
            "tsv",
            "CC0-1.0",
            "OpenFormosa / Mozilla Common Voice contributors",
            &paths.openformosa_common_voice_inventory,
            307,
        )?,
        manifest_source(
            OPENCC_VARIANT_SOURCE_ID,
            OPENCC_VARIANT_SOURCE_NAME,
            "https://github.com/BYVoid/OpenCC",
            "tsv",
            "Apache-2.0-derived policy",
            "OpenCC contributors; ChiaKey Lexicon maintainers",
            &paths.opencc_variant_inventory,
            310,
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
            artifact_json("smart-mandarin-db", "keykey-source-db", &cfg.release_base_url, &paths.db_filename, db_info, &cfg.language_model_version),
            artifact_json("smart-mandarin-metadata", "metadata", &cfg.release_base_url, &paths.metadata_filename, metadata_info, &cfg.language_model_version),
            artifact_json("smart-mandarin-checksums", "checksum", &cfg.release_base_url, "SHA256SUMS", checksum_info, &cfg.language_model_version)
        ]
    }))
}

fn release_source(
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

fn manifest_source(
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
