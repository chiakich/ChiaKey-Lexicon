use crate::config::{
    Config, BONEYARD_SOURCE_ID, BOPOMOFO_CORRECTION_VENDOR_PATH, BPMF_EXT_SOURCE_ID,
    BPMF_EXT_VENDOR_PATH, CANNED_MESSAGES_VENDOR_PATH, CHIAKEY_AUTO_HOTWORDS_SOURCE_ID,
    CHIAKI_SYNTHETIC_SOURCE_ID, CHIAKI_WEB_OVERLAY_SOURCE_ID, CJ_EXT_VENDOR_PATH,
    CJ_PUNCTUATIONS_HALFWIDTH_VENDOR_PATH, CJ_PUNCTUATIONS_MIXEDWIDTH_VENDOR_PATH,
    FRAGMENT_DENYLIST_SOURCE_ID, LIBCHEWING_SOURCE_ID, MODULE_CIN_SOURCE_ID,
    MOZC_EMOTICON_CATEGORIZED_PATH, MOZC_EMOTICON_SOURCE_ID, MOZC_EMOTICON_TSV_PATH,
    OPENFORMOSA_COMMON_VOICE_SOURCE_ID, OVERLAY_SOURCE_ID, PREPOPULATED_SERVICE_SOURCE_ID,
    PUNCTUATION_SOURCE_ID, PUNCTUATION_VENDOR_PATH, RIME_CONVERSION_SOURCE_ID,
    RIME_ESSAY_SOURCE_ID, SIMPLEX_EXT_VENDOR_PATH, SYMBOL_OVERLAY_ALTERNATIVES_PATH,
    SYMBOL_OVERLAY_PATH, SYMBOL_OVERLAY_SOURCE_ID,
};
use std::path::PathBuf;

pub struct ReleasePaths {
    pub boneyard_source_dir: PathBuf,
    pub punctuation_source_dir: PathBuf,
    pub symbol_overlay_source_dir: PathBuf,
    pub prepopulated_service_source_dir: PathBuf,
    pub mozc_emoticon_source_dir: PathBuf,
    pub module_cin_source_dir: PathBuf,
    pub bpmf_ext_source_dir: PathBuf,
    pub libchewing_source_dir: PathBuf,
    pub rime_essay_source_dir: PathBuf,
    pub rime_conversion_source_dir: PathBuf,
    pub overlay_source_dir: PathBuf,
    pub chiaki_web_overlay_source_dir: PathBuf,
    pub chiaki_synthetic_source_dir: PathBuf,
    pub chiakey_auto_hotwords_source_dir: PathBuf,
    pub openformosa_common_voice_source_dir: PathBuf,
    pub fragment_denylist_source_dir: PathBuf,
    pub overlay_phrases: PathBuf,
    pub overlay_explicit: PathBuf,
    pub chiaki_web_overlay_unigrams: PathBuf,
    pub chiaki_web_overlay_bigrams: PathBuf,
    pub chiaki_synthetic_unigrams: PathBuf,
    pub chiaki_synthetic_bigrams: PathBuf,
    pub chiakey_auto_hotwords_phrases: PathBuf,
    pub chiakey_auto_hotwords_state: PathBuf,
    pub openformosa_common_voice_bigrams: PathBuf,
    pub boneyard_inventory: PathBuf,
    pub punctuation_inventory: PathBuf,
    pub punctuation_cin: PathBuf,
    pub symbol_overlay_symbols: PathBuf,
    pub symbol_overlay_alternatives: PathBuf,
    pub prepopulated_service_inventory: PathBuf,
    pub canned_messages_plist: PathBuf,
    pub mozc_emoticon_inventory: PathBuf,
    pub mozc_emoticon_categorized: PathBuf,
    pub mozc_emoticon_tsv: PathBuf,
    pub module_cin_inventory: PathBuf,
    pub cj_ext_cin: PathBuf,
    pub simplex_ext_cin: PathBuf,
    pub cj_punctuations_halfwidth_cin: PathBuf,
    pub cj_punctuations_mixedwidth_cin: PathBuf,
    pub bopomofo_correction_cin: PathBuf,
    pub bpmf_ext_inventory: PathBuf,
    pub bpmf_ext_cin: PathBuf,
    pub libchewing_inventory: PathBuf,
    pub rime_essay_inventory: PathBuf,
    pub rime_essay_raw: PathBuf,
    pub rime_conversion_replacements: PathBuf,
    pub fragment_demotions: PathBuf,
    pub db_filename: String,
    pub metadata_filename: String,
    pub db: PathBuf,
    pub metadata: PathBuf,
    pub checksum: PathBuf,
    pub dist_manifest: PathBuf,
}

impl ReleasePaths {
    pub fn new(cfg: &Config) -> Self {
        let boneyard_source_dir = cfg.root.join("sources").join(BONEYARD_SOURCE_ID);
        let punctuation_source_dir = cfg.root.join("sources").join(PUNCTUATION_SOURCE_ID);
        let symbol_overlay_source_dir = cfg.root.join("sources").join(SYMBOL_OVERLAY_SOURCE_ID);
        let prepopulated_service_source_dir = cfg
            .root
            .join("sources")
            .join(PREPOPULATED_SERVICE_SOURCE_ID);
        let mozc_emoticon_source_dir = cfg.root.join("sources").join(MOZC_EMOTICON_SOURCE_ID);
        let module_cin_source_dir = cfg.root.join("sources").join(MODULE_CIN_SOURCE_ID);
        let bpmf_ext_source_dir = cfg.root.join("sources").join(BPMF_EXT_SOURCE_ID);
        let libchewing_source_dir = cfg.root.join("sources").join(LIBCHEWING_SOURCE_ID);
        let rime_essay_source_dir = cfg.root.join("sources").join(RIME_ESSAY_SOURCE_ID);
        let rime_conversion_source_dir = cfg.root.join("sources").join(RIME_CONVERSION_SOURCE_ID);
        let overlay_source_dir = cfg.root.join("sources").join(OVERLAY_SOURCE_ID);
        let chiaki_web_overlay_source_dir =
            cfg.root.join("sources").join(CHIAKI_WEB_OVERLAY_SOURCE_ID);
        let chiaki_synthetic_source_dir = cfg.root.join("sources").join(CHIAKI_SYNTHETIC_SOURCE_ID);
        let chiakey_auto_hotwords_source_dir = cfg
            .root
            .join("sources")
            .join(CHIAKEY_AUTO_HOTWORDS_SOURCE_ID);
        let openformosa_common_voice_source_dir = cfg
            .root
            .join("sources")
            .join(OPENFORMOSA_COMMON_VOICE_SOURCE_ID);
        let fragment_denylist_source_dir =
            cfg.root.join("sources").join(FRAGMENT_DENYLIST_SOURCE_ID);
        let db_filename = format!("ChiaKeySource-{}.db", cfg.release_version);
        let metadata_filename = format!("ChiaKeySource-{}.json", cfg.release_version);

        Self {
            overlay_phrases: overlay_source_dir.join("phrases.tsv"),
            overlay_explicit: overlay_source_dir.join("explicit.tsv"),
            chiaki_web_overlay_unigrams: chiaki_web_overlay_source_dir.join("unigrams.tsv"),
            chiaki_web_overlay_bigrams: chiaki_web_overlay_source_dir.join("bigrams.tsv"),
            chiaki_synthetic_unigrams: chiaki_synthetic_source_dir.join("unigrams.tsv"),
            chiaki_synthetic_bigrams: chiaki_synthetic_source_dir.join("bigrams.tsv"),
            chiakey_auto_hotwords_phrases: chiakey_auto_hotwords_source_dir.join("phrases.tsv"),
            chiakey_auto_hotwords_state: chiakey_auto_hotwords_source_dir.join("state.json"),
            openformosa_common_voice_bigrams: openformosa_common_voice_source_dir
                .join("bigrams.tsv"),
            boneyard_inventory: boneyard_source_dir.join("source-inventory.sha256"),
            punctuation_inventory: punctuation_source_dir.join("source-inventory.sha256"),
            punctuation_cin: cfg.root.join(PUNCTUATION_VENDOR_PATH),
            symbol_overlay_symbols: cfg.root.join(SYMBOL_OVERLAY_PATH),
            symbol_overlay_alternatives: cfg.root.join(SYMBOL_OVERLAY_ALTERNATIVES_PATH),
            prepopulated_service_inventory: prepopulated_service_source_dir
                .join("source-inventory.sha256"),
            canned_messages_plist: cfg.root.join(CANNED_MESSAGES_VENDOR_PATH),
            mozc_emoticon_inventory: mozc_emoticon_source_dir.join("source-inventory.sha256"),
            mozc_emoticon_categorized: cfg.root.join(MOZC_EMOTICON_CATEGORIZED_PATH),
            mozc_emoticon_tsv: cfg.root.join(MOZC_EMOTICON_TSV_PATH),
            module_cin_inventory: module_cin_source_dir.join("source-inventory.sha256"),
            cj_ext_cin: cfg.root.join(CJ_EXT_VENDOR_PATH),
            simplex_ext_cin: cfg.root.join(SIMPLEX_EXT_VENDOR_PATH),
            cj_punctuations_halfwidth_cin: cfg.root.join(CJ_PUNCTUATIONS_HALFWIDTH_VENDOR_PATH),
            cj_punctuations_mixedwidth_cin: cfg.root.join(CJ_PUNCTUATIONS_MIXEDWIDTH_VENDOR_PATH),
            bopomofo_correction_cin: cfg.root.join(BOPOMOFO_CORRECTION_VENDOR_PATH),
            bpmf_ext_inventory: bpmf_ext_source_dir.join("source-inventory.sha256"),
            bpmf_ext_cin: cfg.root.join(BPMF_EXT_VENDOR_PATH),
            libchewing_inventory: libchewing_source_dir.join("source-inventory.sha256"),
            rime_essay_inventory: rime_essay_source_dir.join("source-inventory.sha256"),
            rime_essay_raw: rime_essay_source_dir.join("raw/essay.txt"),
            rime_conversion_replacements: rime_conversion_source_dir.join("replacements.tsv"),
            fragment_demotions: fragment_denylist_source_dir.join("fragment-demotions.tsv"),
            db: cfg.dist_dir.join(&db_filename),
            metadata: cfg.dist_dir.join(&metadata_filename),
            checksum: cfg.dist_dir.join("SHA256SUMS"),
            dist_manifest: cfg.dist_dir.join("lexicon-manifest.json"),
            boneyard_source_dir,
            punctuation_source_dir,
            symbol_overlay_source_dir,
            prepopulated_service_source_dir,
            mozc_emoticon_source_dir,
            module_cin_source_dir,
            bpmf_ext_source_dir,
            libchewing_source_dir,
            rime_essay_source_dir,
            rime_conversion_source_dir,
            overlay_source_dir,
            chiaki_web_overlay_source_dir,
            chiaki_synthetic_source_dir,
            chiakey_auto_hotwords_source_dir,
            openformosa_common_voice_source_dir,
            fragment_denylist_source_dir,
            db_filename,
            metadata_filename,
        }
    }
}
