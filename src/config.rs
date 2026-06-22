use crate::types::{LibchewingFile, LibchewingWeightMode, SourceDownload};
use anyhow::{Context, Result};
use chrono::SecondsFormat;
use std::env;
use std::path::PathBuf;

pub const BONEYARD_SOURCE_ID: &str = "keykey-boneyard-bootstrap";
pub const BONEYARD_SOURCE_NAME: &str = "KeyKey Boneyard bootstrap data";
pub const BONEYARD_VENDOR_DB_PATH: &str =
    "sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db";
pub const LIBCHEWING_SOURCE_ID: &str = "libchewing-data";
pub const LIBCHEWING_SOURCE_NAME: &str = "libchewing-data Traditional Chinese Zhuyin dictionary";
pub const RIME_ESSAY_SOURCE_ID: &str = "rime-essay";
pub const RIME_ESSAY_SOURCE_NAME: &str = "Rime essay shared vocabulary and language model";
pub const OVERLAY_SOURCE_ID: &str = "chiaki-modern-overlay";
pub const OVERLAY_SOURCE_NAME: &str = "Chiaki modern overlay phrases";
pub const DATABASE_SCHEMA_VERSION: i64 = 1;

pub const DOWNLOADS: &[SourceDownload] = &[
    SourceDownload {
        url: "https://raw.githubusercontent.com/chewing/libchewing-data/v2026.3.22/dict/chewing/tsi.csv",
        path: "sources/libchewing-data/raw/dict/chewing/tsi.csv",
        sha256: "c889a1ac3ae1901b3f8f62748bc41b958f010bf995f7f88dbaf9e3494f341428",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/chewing/libchewing-data/v2026.3.22/dict/chewing/word.csv",
        path: "sources/libchewing-data/raw/dict/chewing/word.csv",
        sha256: "da55b8e599c1389bc486453554f3410cf9c621d0ffff0ce38855698d26b3892a",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/chewing/libchewing-data/v2026.3.22/dict/chewing/alt.csv",
        path: "sources/libchewing-data/raw/dict/chewing/alt.csv",
        sha256: "66df78f53ff18ab97bc39b3f3108a1f6d8d5be3237d9e72ff9f6f7186b4d6b2e",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/chewing/libchewing/v0.12.0/COPYING",
        path: "LICENSES/libchewing-data-LGPL-2.1-or-later.txt",
        sha256: "dc626520dcd53a22f727af3ee42c770e56c97a64fe3adb063799d8ab032fe551",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/rime/rime-essay/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed/essay.txt",
        path: "sources/rime-essay/raw/essay.txt",
        sha256: "09086a44204f469d2c16ad72784e1f567a6f016570dfc9aa79f868267a9c1385",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/rime/rime-essay/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed/LICENSE",
        path: "LICENSES/rime-essay-LGPL-3.0.txt",
        sha256: "da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768",
    },
];

pub struct Config {
    pub root: PathBuf,
    pub boneyard_db: PathBuf,
    pub release_version: String,
    pub language_model_version: String,
    pub minimum_app_version: String,
    pub generated_at: String,
    pub release_base_url: String,
    pub max_phrase_codepoints: usize,
    pub rime_essay_min_score: i64,
    pub dist_dir: PathBuf,
    pub normalized_path: PathBuf,
    pub manifest_path: PathBuf,
}

pub fn load() -> Result<Config> {
    let root = env::current_dir().context("read current directory")?;
    let release_version = env_or("LEXICON_VERSION", "2026.06.5");
    let language_model_version = format!("chiaki-modern-{release_version}");
    let minimum_app_version = env_or("MINIMUM_APP_VERSION", "0.1.0");
    let generated_at = env::var("GENERATED_AT")
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    let release_base_url = env_or(
        "RELEASE_BASE_URL",
        format!(
            "https://github.com/akira02/Chiaki-KeyKey-Lexicon/releases/download/{release_version}"
        ),
    );
    let max_phrase_codepoints = env_or("MAX_PHRASE_CODEPOINTS", "7")
        .parse()
        .context("parse MAX_PHRASE_CODEPOINTS")?;
    let rime_essay_min_score = env_or("RIME_ESSAY_MIN_SCORE", "40")
        .parse()
        .context("parse RIME_ESSAY_MIN_SCORE")?;
    let legacy_boneyard_root = env::var("KEYKEY_BONEYARD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("..").join("KeyKey-Boneyard"));
    let legacy_boneyard_db = legacy_boneyard_root
        .join("YahooKeyKey-Source-1.1.2528")
        .join("Distributions/Takao/CookedDatabase/KeyKeySource.db");
    let boneyard_db = env::var("BONEYARD_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let vendored = root.join(BONEYARD_VENDOR_DB_PATH);
            if vendored.is_file() {
                vendored
            } else {
                legacy_boneyard_db
            }
        });
    let dist_dir = env::var("DIST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("dist").join(&release_version));
    let normalized_path = env::var("NORMALIZED_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("normalized/smart-mandarin.tsv"));
    let manifest_path = env::var("MANIFEST_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("manifests/lexicon-manifest.json"));

    Ok(Config {
        root,
        boneyard_db,
        release_version,
        language_model_version,
        minimum_app_version,
        generated_at,
        release_base_url,
        max_phrase_codepoints,
        rime_essay_min_score,
        dist_dir,
        normalized_path,
        manifest_path,
    })
}

pub fn libchewing_files(cfg: &Config) -> Vec<LibchewingFile> {
    let source_dir = cfg.root.join("sources").join(LIBCHEWING_SOURCE_ID);
    vec![
        LibchewingFile {
            path: source_dir.join("raw/dict/chewing/tsi.csv"),
            kind: "libchewing-phrase",
            source_suffix: "",
            min_codepoints: 2,
            max_codepoints: cfg.max_phrase_codepoints,
            replace_phrases: true,
            skip_existing_exact: false,
            weight_mode: LibchewingWeightMode::Frequency,
        },
        LibchewingFile {
            path: source_dir.join("raw/dict/chewing/alt.csv"),
            kind: "libchewing-alternate",
            source_suffix: "",
            min_codepoints: 2,
            max_codepoints: cfg.max_phrase_codepoints,
            replace_phrases: true,
            skip_existing_exact: false,
            weight_mode: LibchewingWeightMode::Frequency,
        },
        LibchewingFile {
            path: source_dir.join("raw/dict/chewing/word.csv"),
            kind: "libchewing-character",
            source_suffix: "",
            min_codepoints: 1,
            max_codepoints: 1,
            replace_phrases: false,
            skip_existing_exact: true,
            weight_mode: LibchewingWeightMode::CharacterFallback,
        },
        LibchewingFile {
            path: source_dir.join("raw/dict/chewing/tsi.csv"),
            kind: "libchewing-character-frequency",
            source_suffix: "#characters",
            min_codepoints: 1,
            max_codepoints: 1,
            replace_phrases: false,
            skip_existing_exact: false,
            weight_mode: LibchewingWeightMode::CharacterFrequency,
        },
    ]
}

fn env_or(key: &str, fallback: impl Into<String>) -> String {
    env::var(key).unwrap_or_else(|_| fallback.into())
}
