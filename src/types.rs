use std::path::PathBuf;

#[derive(Clone)]
pub struct SourceDownload {
    pub url: &'static str,
    pub path: &'static str,
    pub sha256: &'static str,
}

#[derive(Clone)]
pub struct LibchewingFile {
    pub path: PathBuf,
    pub kind: &'static str,
    pub source_suffix: &'static str,
    pub min_codepoints: usize,
    pub max_codepoints: usize,
    pub replace_phrases: bool,
    pub skip_existing_exact: bool,
    pub weight_mode: LibchewingWeightMode,
}

#[derive(Clone, Copy)]
pub enum LibchewingWeightMode {
    Frequency,
    CharacterFrequency,
    CharacterFallback,
}

#[derive(Clone)]
pub struct SourceRecord {
    pub qstring: String,
    pub phrase: String,
    pub weight: f64,
    pub source_id: &'static str,
    pub tags: String,
}

pub struct KeyValueRecord {
    pub key: String,
    pub value: String,
}

pub struct VariantDemotionRecord {
    pub phrase: String,
    pub max_weight: f64,
    pub tags: String,
}

pub struct ImportResult {
    pub source_path: String,
    pub seen: usize,
    pub added: usize,
    pub skipped: usize,
    pub records: Vec<SourceRecord>,
}

#[derive(Clone)]
pub struct FileInfo {
    pub sha256: String,
    pub size: u64,
}
