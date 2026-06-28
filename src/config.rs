use crate::types::{LibchewingFile, LibchewingWeightMode, SourceDownload};
use anyhow::{Context, Result};
use chrono::SecondsFormat;
use std::env;
use std::path::PathBuf;

pub const BONEYARD_SOURCE_ID: &str = "keykey-boneyard-bootstrap";
pub const BONEYARD_SOURCE_NAME: &str = "KeyKey Boneyard bootstrap data";
pub const BONEYARD_VENDOR_DB_PATH: &str =
    "sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db";
pub const PUNCTUATION_SOURCE_ID: &str = "keykey-punctuations-cin";
pub const PUNCTUATION_SOURCE_NAME: &str = "KeyKey BPMF punctuation table";
pub const PUNCTUATION_VENDOR_PATH: &str =
    "sources/keykey-punctuations-cin/vendor/bpmf-punctuations.cin";
pub const SYMBOL_OVERLAY_SOURCE_ID: &str = "chiakey-symbols-overlay";
pub const SYMBOL_OVERLAY_SOURCE_NAME: &str = "ChiaKey supplemental symbol list";
pub const SYMBOL_OVERLAY_PATH: &str = "sources/chiakey-symbols-overlay/symbols.tsv";
pub const SYMBOL_OVERLAY_ALTERNATIVES_PATH: &str =
    "sources/chiakey-symbols-overlay/punctuation-alternatives.tsv";
pub const PREPOPULATED_SERVICE_SOURCE_ID: &str = "keykey-prepopulated-service-data";
pub const PREPOPULATED_SERVICE_SOURCE_NAME: &str = "KeyKey prepopulated service data";
pub const CANNED_MESSAGES_VENDOR_PATH: &str =
    "sources/keykey-prepopulated-service-data/vendor/CannedMessages.plist";
pub const MOZC_EMOTICON_SOURCE_ID: &str = "mozc-emoticon-data";
pub const MOZC_EMOTICON_SOURCE_NAME: &str = "Mozc emoticon data";
pub const MOZC_EMOTICON_CATEGORIZED_PATH: &str = "sources/mozc-emoticon-data/raw/categorized.tsv";
pub const MOZC_EMOTICON_TSV_PATH: &str = "sources/mozc-emoticon-data/raw/emoticon.tsv";
pub const MODULE_CIN_SOURCE_ID: &str = "keykey-module-cin";
pub const MODULE_CIN_SOURCE_NAME: &str = "KeyKey module CIN tables";
pub const CJ_EXT_VENDOR_PATH: &str = "sources/keykey-module-cin/vendor/cj-ext.cin";
pub const SIMPLEX_EXT_VENDOR_PATH: &str = "sources/keykey-module-cin/vendor/simplex-ext.cin";
pub const CJ_PUNCTUATIONS_HALFWIDTH_VENDOR_PATH: &str =
    "sources/keykey-module-cin/vendor/cj-punctuations-halfwidth.cin";
pub const CJ_PUNCTUATIONS_MIXEDWIDTH_VENDOR_PATH: &str =
    "sources/keykey-module-cin/vendor/cj-punctuations-mixedwidth.cin";
pub const BOPOMOFO_CORRECTION_VENDOR_PATH: &str =
    "sources/keykey-module-cin/vendor/bopomofo-correction.cin";
pub const BPMF_EXT_SOURCE_ID: &str = "bpmf-ext-cin";
pub const BPMF_EXT_SOURCE_NAME: &str = "Public domain extended BPMF character table";
pub const BPMF_EXT_VENDOR_PATH: &str = "sources/bpmf-ext-cin/vendor/bpmf-ext.cin";
pub const LIBCHEWING_SOURCE_ID: &str = "libchewing-data";
pub const LIBCHEWING_SOURCE_NAME: &str = "libchewing-data Traditional Chinese Zhuyin dictionary";
pub const RIME_ESSAY_SOURCE_ID: &str = "rime-essay";
pub const RIME_ESSAY_SOURCE_NAME: &str = "Rime essay shared vocabulary and language model";
pub const RIME_CONVERSION_SOURCE_ID: &str = "chiakey-rime-conversion-policy";
pub const RIME_CONVERSION_SOURCE_NAME: &str = "ChiaKey Rime OpenCC override policy";
pub const OVERLAY_SOURCE_ID: &str = "chiakey-modern-overlay";
pub const OVERLAY_SOURCE_NAME: &str = "ChiaKey modern overlay phrases";
pub const CHIAKI_WEB_OVERLAY_SOURCE_ID: &str = "chiaki-web-overlay";
pub const CHIAKI_WEB_OVERLAY_SOURCE_NAME: &str = "Chiaki reviewed web corpus overlay";
pub const CHIAKI_SYNTHETIC_SOURCE_ID: &str = "chiaki-synthetic-overlay";
pub const CHIAKI_SYNTHETIC_SOURCE_NAME: &str =
    "Chiaki.C GPT-5.5 synthetic Taiwan internet usage overlay";
pub const CHIAKEY_AUTO_HOTWORDS_SOURCE_ID: &str = "chiakey-auto-hotwords-overlay";
pub const CHIAKEY_AUTO_HOTWORDS_SOURCE_NAME: &str =
    "ChiaKey automatically refreshed hotwords overlay";
pub const OPENFORMOSA_COMMON_VOICE_SOURCE_ID: &str = "openformosa-common-voice-25-zh-tw";
pub const OPENFORMOSA_COMMON_VOICE_SOURCE_NAME: &str =
    "OpenFormosa Common Voice 25 zh-TW bigram overlay";
pub const OPENCC_VARIANT_SOURCE_ID: &str = "opencc-variant-policy";
pub const FRAGMENT_DENYLIST_SOURCE_ID: &str = "chiakey-fragment-denylist";
pub const FRAGMENT_DENYLIST_SOURCE_NAME: &str = "ChiaKey non-lexical fragment weight caps";
pub const DATABASE_SCHEMA_VERSION: i64 = 1;
pub const DEFAULT_RELEASE_VERSION: &str = "dev";

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
        url: "https://codeberg.org/chewing/libchewing/raw/tag/v0.12.0/COPYING",
        path: "sources/libchewing-data/COPYING",
        sha256: "dc626520dcd53a22f727af3ee42c770e56c97a64fe3adb063799d8ab032fe551",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/rime/rime-essay/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed/essay.txt",
        path: "sources/rime-essay/raw/essay.txt",
        sha256: "09086a44204f469d2c16ad72784e1f567a6f016570dfc9aa79f868267a9c1385",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/rime/rime-essay/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed/LICENSE",
        path: "sources/rime-essay/LICENSE",
        sha256: "da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/google/mozc/28da5a39f9a7fd70251c85d269f4a8b47aa31cf8/src/data/emoticon/categorized.tsv",
        path: MOZC_EMOTICON_CATEGORIZED_PATH,
        sha256: "4497c16a706de418b05e73eaddbce13e5d3390e7c2de71200b28c0c97ae5c4fc",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/google/mozc/28da5a39f9a7fd70251c85d269f4a8b47aa31cf8/src/data/emoticon/emoticon.tsv",
        path: MOZC_EMOTICON_TSV_PATH,
        sha256: "366558b380bef07dda26822c9100d1efabee539f7961b2c6363d4614c4a762c4",
    },
    SourceDownload {
        url: "https://raw.githubusercontent.com/google/mozc/28da5a39f9a7fd70251c85d269f4a8b47aa31cf8/LICENSE",
        path: "sources/mozc-emoticon-data/LICENSE",
        sha256: "44cdd923b91ea9199293abecc2762c70c87dbf1e581c027a94c416368d1a648c",
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
    pub opencc_binary: PathBuf,
    pub opencc_t2tw_config: PathBuf,
    // How much each source's strongest collocation should beat its unigram floor
    // when re-anchored to the unigram scale (see importers::calibrate_bigram_boost).
    // 0 = raw passthrough.
    pub synthetic_bigram_boost: f64,
    pub commonvoice_bigram_boost: f64,
    // Min rime-essay frequency advantage for a homophone to be promoted to its
    // reading group's top single-char candidate (see single-char homophone rerank).
    pub homophone_rerank_min_ratio: f64,
    pub dist_dir: PathBuf,
    pub normalized_path: PathBuf,
    pub manifest_path: PathBuf,
}

pub fn load() -> Result<Config> {
    let root = env::current_dir().context("read current directory")?;
    let release_version = env_or("LEXICON_VERSION", DEFAULT_RELEASE_VERSION);
    let language_model_version = release_version.clone();
    let minimum_app_version = env_or("MINIMUM_APP_VERSION", "0.1.0");
    let generated_at = env::var("GENERATED_AT")
        .unwrap_or_else(|_| chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true));
    let release_base_url = env_or(
        "RELEASE_BASE_URL",
        format!("https://github.com/akira02/ChiaKey-Lexicon/releases/download/{release_version}"),
    );
    let max_phrase_codepoints = env_or("MAX_PHRASE_CODEPOINTS", "7")
        .parse()
        .context("parse MAX_PHRASE_CODEPOINTS")?;
    let rime_essay_min_score = env_or("RIME_ESSAY_MIN_SCORE", "40")
        .parse()
        .context("parse RIME_ESSAY_MIN_SCORE")?;
    let opencc_binary = PathBuf::from(env_or("OPENCC_BINARY", "opencc"));
    let opencc_t2tw_config = PathBuf::from(env_or("OPENCC_T2TW_CONFIG", "t2tw.json"));
    let synthetic_bigram_boost = env_or("SYNTHETIC_BIGRAM_BOOST", "1.5")
        .parse()
        .context("parse SYNTHETIC_BIGRAM_BOOST")?;
    let commonvoice_bigram_boost = env_or("COMMONVOICE_BIGRAM_BOOST", "1.5")
        .parse()
        .context("parse COMMONVOICE_BIGRAM_BOOST")?;
    let homophone_rerank_min_ratio = env_or("HOMOPHONE_RERANK_MIN_RATIO", "2.5")
        .parse()
        .context("parse HOMOPHONE_RERANK_MIN_RATIO")?;
    let boneyard_checkout_root = env::var("KEYKEY_BONEYARD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| root.join("..").join("KeyKey-Boneyard"));
    let boneyard_checkout_db = boneyard_checkout_root
        .join("YahooKeyKey-Source-1.1.2528")
        .join("Distributions/Takao/CookedDatabase/KeyKeySource.db");
    let boneyard_db = env::var("BONEYARD_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let vendored = root.join(BONEYARD_VENDOR_DB_PATH);
            if vendored.is_file() {
                vendored
            } else {
                boneyard_checkout_db
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
        opencc_binary,
        opencc_t2tw_config,
        synthetic_bigram_boost,
        commonvoice_bigram_boost,
        homophone_rerank_min_ratio,
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
