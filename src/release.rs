use crate::associated_phrases;
use crate::bpmf_ext;
use crate::config::{self, Config};
use crate::db;
use crate::files::{
    file_info, repo_relative, sha256_bytes, sha256_file, verify_required_files, write_inventory,
    write_json, write_text,
};
use crate::importers;
use crate::manifest;
use crate::module_cin;
use crate::paths::ReleasePaths;
use crate::prepopulated;
use crate::punctuations;
use crate::types::{ImportResult, SourceRecord};
use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

pub fn run() -> Result<()> {
    let cfg = config::load()?;
    let paths = ReleasePaths::new(&cfg);
    let libchewing_files = config::libchewing_files(&cfg);

    verify_inputs(&cfg, &paths, &libchewing_files)?;
    create_output_dirs(&cfg, &paths)?;
    write_source_inventories(&paths, &libchewing_files)?;

    fs::copy(&cfg.boneyard_db, &paths.db).with_context(|| {
        format!(
            "copy {} to {}",
            cfg.boneyard_db.display(),
            paths.db.display()
        )
    })?;
    let mut conn = Connection::open(&paths.db)?;
    let mut source_keys: HashMap<(String, String), SourceRecord> = HashMap::new();
    let mut import_results = Vec::new();

    import_libchewing(
        &mut conn,
        &cfg,
        &libchewing_files,
        &mut source_keys,
        &mut import_results,
    )?;
    import_libchewing_character_phrase_evidence(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_bpmf_ext(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    let normalized_rime = prepare_rime_essay(&cfg, &paths)?;
    import_rime_overlap_rerank(
        &mut conn,
        &cfg,
        &paths,
        &normalized_rime,
        &mut source_keys,
        &mut import_results,
    )?;
    import_rime_existing_phrase_rerank(
        &mut conn,
        &cfg,
        &paths,
        &normalized_rime,
        &mut source_keys,
        &mut import_results,
    )?;
    import_rime(
        &mut conn,
        &cfg,
        &paths,
        &normalized_rime,
        &mut source_keys,
        &mut import_results,
    )?;
    import_overlay(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_explicit_overlay(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_chiaki_web_overlay(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_chiaki_synthetic_overlay(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_chiakey_auto_hotwords_overlay(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_opencc_variant_policy(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_fragment_demotions(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_single_char_homophone_rerank(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_chiaki_synthetic_bigrams(&mut conn, &cfg, &paths, &mut import_results)?;
    import_openformosa_common_voice_bigrams(&mut conn, &cfg, &paths, &mut import_results)?;
    import_chiaki_web_bigrams(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_punctuations(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_symbol_overlay(
        &mut conn,
        &cfg,
        &paths,
        &mut source_keys,
        &mut import_results,
    )?;
    import_prepopulated_service_data(&mut conn, &cfg, &paths, &mut import_results)?;
    import_module_cin_tables(&mut conn, &cfg, &paths, &mut import_results)?;
    import_associated_phrases(&mut conn, &mut import_results)?;

    db::refresh_metadata_counts(&conn)?;
    db::update_release_metadata_rows(&conn, &cfg)?;
    prepopulated::validate_runtime_required_data(&conn)?;
    module_cin::validate_runtime_required_data(&conn)?;
    associated_phrases::validate_runtime_required_data(&conn)?;
    db::write_normalized(&conn, &cfg.normalized_path, &source_keys)?;

    let metadata = db::db_metadata(&conn)?;
    let source_rows = db::db_source_rows(&conn)?;
    let counts = db::db_counts(&conn, &cfg.normalized_path, &metadata)?;
    drop(conn);

    let db_info = file_info(&paths.db)?;
    let normalized_info = file_info(&cfg.normalized_path)?;
    let release_metadata = manifest::release_metadata(
        &cfg,
        &paths,
        &metadata,
        &counts,
        &source_rows,
        &db_info,
        &normalized_info,
    )?;
    write_json(&paths.metadata, &release_metadata)?;
    let metadata_info = file_info(&paths.metadata)?;

    write_text(
        &paths.checksum,
        &format!(
            "{}  {}\n{}  {}\n",
            db_info.sha256, paths.db_filename, metadata_info.sha256, paths.metadata_filename
        ),
    )?;
    let checksum_info = file_info(&paths.checksum)?;
    let manifest_json = manifest::manifest(&cfg, &paths, &db_info, &metadata_info, &checksum_info)?;
    write_json(&cfg.manifest_path, &manifest_json)?;
    fs::copy(&cfg.manifest_path, &paths.dist_manifest)?;

    print_summary(&cfg, &paths, &counts, &import_results);
    Ok(())
}

fn verify_inputs(
    cfg: &Config,
    paths: &ReleasePaths,
    libchewing_files: &[crate::types::LibchewingFile],
) -> Result<()> {
    let mut required = vec![
        cfg.boneyard_db.clone(),
        paths.boneyard_inventory.clone(),
        paths.punctuation_cin.clone(),
        paths.symbol_overlay_symbols.clone(),
        paths.symbol_overlay_alternatives.clone(),
        paths.canned_messages_plist.clone(),
        paths.mozc_emoticon_categorized.clone(),
        paths.mozc_emoticon_tsv.clone(),
        paths.bpmf_ext_cin.clone(),
        paths.overlay_phrases.clone(),
        paths.overlay_explicit.clone(),
        paths.chiaki_web_overlay_explicit.clone(),
        paths.chiaki_web_overlay_bigrams.clone(),
        paths.chiaki_synthetic_unigrams.clone(),
        paths.chiaki_synthetic_bigrams.clone(),
        paths.chiakey_auto_hotwords_phrases.clone(),
        paths.chiakey_auto_hotwords_state.clone(),
        paths.openformosa_common_voice_bigrams.clone(),
        paths.fragment_demotions.clone(),
        paths.rime_essay_raw.clone(),
        paths.rime_conversion_replacements.clone(),
    ];
    required.extend(module_cin_files(paths));
    required.extend(libchewing_files.iter().map(|entry| entry.path.clone()));
    verify_required_files(&required)
}

fn create_output_dirs(cfg: &Config, paths: &ReleasePaths) -> Result<()> {
    fs::create_dir_all(&cfg.dist_dir)?;
    if let Some(parent) = cfg.normalized_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir_all(&paths.boneyard_source_dir)?;
    fs::create_dir_all(&paths.punctuation_source_dir)?;
    fs::create_dir_all(&paths.symbol_overlay_source_dir)?;
    fs::create_dir_all(&paths.prepopulated_service_source_dir)?;
    fs::create_dir_all(&paths.mozc_emoticon_source_dir)?;
    fs::create_dir_all(&paths.module_cin_source_dir)?;
    fs::create_dir_all(&paths.bpmf_ext_source_dir)?;
    fs::create_dir_all(&paths.libchewing_source_dir)?;
    fs::create_dir_all(&paths.rime_essay_source_dir)?;
    fs::create_dir_all(&paths.rime_conversion_source_dir)?;
    fs::create_dir_all(&paths.overlay_source_dir)?;
    fs::create_dir_all(&paths.chiaki_web_overlay_source_dir)?;
    fs::create_dir_all(&paths.chiaki_synthetic_source_dir)?;
    fs::create_dir_all(&paths.chiakey_auto_hotwords_source_dir)?;
    fs::create_dir_all(&paths.openformosa_common_voice_source_dir)?;
    fs::create_dir_all(&paths.fragment_denylist_source_dir)?;
    Ok(())
}

fn write_source_inventories(
    paths: &ReleasePaths,
    libchewing_files: &[crate::types::LibchewingFile],
) -> Result<()> {
    // Keep source inventories focused on compatibility and external
    // vendored/pinned upstream inputs.
    // Internal project-owned overlays/policies are maintained in git and do not
    // need per-release source-inventory churn.
    let mut libchewing_paths = libchewing_files
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    libchewing_paths.sort();
    libchewing_paths.dedup();
    write_inventory(
        &paths.libchewing_inventory,
        &paths.libchewing_source_dir,
        &libchewing_paths,
        true,
    )?;
    write_inventory(
        &paths.punctuation_inventory,
        &paths.punctuation_source_dir,
        std::slice::from_ref(&paths.punctuation_cin),
        true,
    )?;
    write_inventory(
        &paths.prepopulated_service_inventory,
        &paths.prepopulated_service_source_dir,
        std::slice::from_ref(&paths.canned_messages_plist),
        true,
    )?;
    write_inventory(
        &paths.mozc_emoticon_inventory,
        &paths.mozc_emoticon_source_dir,
        &[
            paths.mozc_emoticon_categorized.clone(),
            paths.mozc_emoticon_tsv.clone(),
        ],
        true,
    )?;
    write_inventory(
        &paths.module_cin_inventory,
        &paths.module_cin_source_dir,
        &module_cin_files(paths),
        true,
    )?;
    write_inventory(
        &paths.bpmf_ext_inventory,
        &paths.bpmf_ext_source_dir,
        std::slice::from_ref(&paths.bpmf_ext_cin),
        true,
    )?;
    write_inventory(
        &paths.rime_essay_inventory,
        &paths.rime_essay_source_dir,
        std::slice::from_ref(&paths.rime_essay_raw),
        true,
    )?;
    Ok(())
}

fn import_libchewing(
    conn: &mut Connection,
    cfg: &Config,
    files: &[crate::types::LibchewingFile],
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let phrase_paths = files
        .iter()
        .filter(|entry| entry.min_codepoints >= 2)
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    let max_score = importers::libchewing_max_score(&phrase_paths)?;

    for entry in files {
        let existing_exact_keys = if entry.skip_existing_exact {
            Some(db::load_existing_exact_keys(conn)?)
        } else {
            None
        };
        let (records, seen, skipped) =
            importers::parse_libchewing_csv(entry, max_score, existing_exact_keys.as_ref())?;
        let source_path = format!(
            "{}{}",
            repo_relative(&cfg.root, &entry.path)?,
            entry.source_suffix
        );
        let result = db::apply_records(
            conn,
            records,
            &source_path,
            entry.kind,
            &sha256_file(&entry.path)?,
            seen,
            skipped,
            entry.replace_phrases,
        )?;
        remember_records(source_keys, &result);
        import_results.push(result);
    }
    Ok(())
}

fn import_punctuations(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) = punctuations::parse_cin(&paths.punctuation_cin)?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.punctuation_cin)?,
        "keykey-punctuation-cin",
        &sha256_file(&paths.punctuation_cin)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_prepopulated_service_data(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let data = prepopulated::load(
        &paths.canned_messages_plist,
        &paths.symbol_overlay_symbols,
        &paths.mozc_emoticon_categorized,
        &paths.mozc_emoticon_tsv,
        &cfg.generated_at,
    )?;
    prepopulated::validate_payload(&data)?;

    let source_rows = vec![
        (
            repo_relative(&cfg.root, &paths.canned_messages_plist)?,
            prepopulated::source_kind().to_string(),
            sha256_file(&paths.canned_messages_plist)?,
        ),
        (
            format!(
                "{}#canned-messages",
                repo_relative(&cfg.root, &paths.symbol_overlay_symbols)?
            ),
            "chiakey-symbols-overlay-canned-messages".to_string(),
            sha256_file(&paths.symbol_overlay_symbols)?,
        ),
        (
            format!(
                "{}#canned-messages",
                repo_relative(&cfg.root, &paths.mozc_emoticon_categorized)?
            ),
            "mozc-emoticon-categorized-canned-messages".to_string(),
            sha256_file(&paths.mozc_emoticon_categorized)?,
        ),
        (
            format!(
                "{}#canned-messages",
                repo_relative(&cfg.root, &paths.mozc_emoticon_tsv)?
            ),
            "mozc-emoticon-canned-messages".to_string(),
            sha256_file(&paths.mozc_emoticon_tsv)?,
        ),
    ];
    db::apply_prepopulated_service_data(conn, &data, &source_rows)?;

    import_results.push(ImportResult {
        source_path: format!(
            "{}/vendor",
            repo_relative(&cfg.root, &paths.prepopulated_service_source_dir)?
        ),
        seen: 1,
        added: 2 + data.supplemental_symbol_count + data.emoji_message_count,
        skipped: 0,
        records: Vec::new(),
    });
    Ok(())
}

fn import_symbol_overlay(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let existing_exact_keys = db::load_existing_exact_keys(conn)?;
    let (records, seen, skipped) = punctuations::parse_symbol_alternatives(
        &paths.symbol_overlay_alternatives,
        &existing_exact_keys,
    )?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.symbol_overlay_alternatives)?,
        "chiakey-symbol-alternatives-overlay",
        &sha256_file(&paths.symbol_overlay_alternatives)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);

    let existing_exact_keys = db::load_existing_exact_keys(conn)?;
    let (records, seen, skipped) =
        punctuations::parse_symbol_overlay(&paths.symbol_overlay_symbols, &existing_exact_keys)?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.symbol_overlay_symbols)?,
        "chiakey-symbol-list-overlay",
        &sha256_file(&paths.symbol_overlay_symbols)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

const GENERIC_CJ_INDEXES: &[(&str, &str)] = &[
    ("Generic-cj-cin-index-on-key", "key"),
    ("Generic-cj-cin-index-on-value", "value"),
];
const GENERIC_SIMPLEX_INDEXES: &[(&str, &str)] = &[("Generic-simplex-cin-index", "key")];
const CJ_PUNCTUATIONS_HALFWIDTH_INDEXES: &[(&str, &str)] =
    &[("Punctuations-cj-halfwidth-cin-index", "key")];
const CJ_PUNCTUATIONS_MIXEDWIDTH_INDEXES: &[(&str, &str)] =
    &[("Punctuations-cj-mixedwidth-cin-index", "key")];
const BOPOMOFO_CORRECTION_INDEXES: &[(&str, &str)] =
    &[("BopomofoCorrection-bopomofo-correction-cin-index", "key")];

fn import_module_cin_tables(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let specs = [
        (
            &paths.cj_ext_cin,
            "Generic-cj-cin",
            "module-cj-cin",
            GENERIC_CJ_INDEXES,
        ),
        (
            &paths.simplex_ext_cin,
            "Generic-simplex-cin",
            "module-simplex-cin",
            GENERIC_SIMPLEX_INDEXES,
        ),
        (
            &paths.cj_punctuations_halfwidth_cin,
            "Punctuations-cj-halfwidth-cin",
            "module-punctuation-cin",
            CJ_PUNCTUATIONS_HALFWIDTH_INDEXES,
        ),
        (
            &paths.cj_punctuations_mixedwidth_cin,
            "Punctuations-cj-mixedwidth-cin",
            "module-punctuation-cin",
            CJ_PUNCTUATIONS_MIXEDWIDTH_INDEXES,
        ),
        (
            &paths.bopomofo_correction_cin,
            "BopomofoCorrection-bopomofo-correction-cin",
            "module-bopomofo-correction-cin",
            BOPOMOFO_CORRECTION_INDEXES,
        ),
    ];

    for (path, table_name, kind, indexes) in specs {
        let (records, seen, skipped) = module_cin::parse_cin(path)?;
        let result = db::apply_key_value_records(
            conn,
            table_name,
            &records,
            &repo_relative(&cfg.root, path)?,
            kind,
            &sha256_file(path)?,
            seen,
            skipped,
            indexes,
        )?;
        import_results.push(result);
    }

    Ok(())
}

fn import_associated_phrases(
    conn: &mut Connection,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let build = associated_phrases::build_from_unigrams(conn)?;
    let result = db::apply_associated_phrase_records(
        conn,
        &build.records,
        associated_phrases::SOURCE_PATH,
        associated_phrases::SOURCE_KIND,
        &build.sha256,
        build.seen,
        build.tail_count,
        build.skipped,
    )?;
    import_results.push(result);
    Ok(())
}

fn import_bpmf_ext(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let existing_exact_keys = db::load_existing_exact_keys(conn)?;
    let (records, seen, skipped) = bpmf_ext::parse_cin(&paths.bpmf_ext_cin, &existing_exact_keys)?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.bpmf_ext_cin)?,
        "bpmf-ext-character-supplement",
        &sha256_file(&paths.bpmf_ext_cin)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn prepare_rime_essay(
    cfg: &Config,
    paths: &ReleasePaths,
) -> Result<importers::NormalizedRimeEssay> {
    let (conversion_rules, _conversion_seen, _conversion_skipped) =
        importers::parse_conversion_rules(&paths.rime_conversion_replacements)?;
    let normalization = importers::RimeNormalization::with_opencc(
        &conversion_rules,
        &cfg.opencc_binary,
        &cfg.opencc_t2tw_config,
    );
    importers::read_normalized_rime_essay(&paths.rime_essay_raw, &normalization)
}

fn import_rime(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    normalized_rime: &importers::NormalizedRimeEssay,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let char_readings = db::load_primary_character_readings(conn)?;
    let existing_phrases = db::load_existing_phrases(conn)?;
    let existing_qstring_weights = db::load_best_qstring_weights(conn)?;
    let (records, seen, skipped) = importers::parse_normalized_rime_essay(
        normalized_rime,
        cfg,
        &char_readings,
        &existing_phrases,
        &existing_qstring_weights,
    )?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.rime_essay_raw)?,
        "rime-supplement",
        &sha256_file(&paths.rime_essay_raw)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_libchewing_character_phrase_evidence(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let tsi_path = paths.libchewing_source_dir.join("raw/dict/chewing/tsi.csv");
    let evidence = db::load_character_phrase_evidence(
        conn,
        importers::LIBCHEWING_CHARACTER_PHRASE_EVIDENCE_MIN_PHRASE_WEIGHT,
    )?;
    let seen = evidence.len();
    let records = importers::phrase_evidence_character_records(&evidence);
    let skipped = seen.saturating_sub(records.len());
    let result = db::apply_records(
        conn,
        records,
        &format!(
            "{}#character-phrase-evidence",
            repo_relative(&cfg.root, &tsi_path)?
        ),
        "libchewing-character-phrase-evidence",
        &sha256_file(&tsi_path)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_rime_overlap_rerank(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    normalized_rime: &importers::NormalizedRimeEssay,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let existing_records = db::load_existing_phrase_weights(conn)?;
    let (records, seen, skipped) =
        importers::parse_normalized_rime_overlap_reranks(normalized_rime, cfg, &existing_records)?;
    let result = db::apply_records(
        conn,
        records,
        &format!(
            "{}#overlap-rerank",
            repo_relative(&cfg.root, &paths.rime_essay_raw)?
        ),
        "rime-overlap-rerank",
        &sha256_file(&paths.rime_essay_raw)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_rime_existing_phrase_rerank(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    normalized_rime: &importers::NormalizedRimeEssay,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let existing_records = db::load_existing_phrase_weights(conn)?;
    let existing_qstring_weights = db::load_best_qstring_weights(conn)?;
    let (records, seen, skipped) = importers::parse_normalized_rime_existing_phrase_reranks(
        normalized_rime,
        cfg,
        &existing_records,
        &existing_qstring_weights,
    )?;
    let result = db::apply_records(
        conn,
        records,
        &format!(
            "{}#existing-rerank",
            repo_relative(&cfg.root, &paths.rime_essay_raw)?
        ),
        "rime-existing-rerank",
        &sha256_file(&paths.rime_essay_raw)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_overlay(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, parse_skipped) = importers::parse_overlay(&paths.overlay_phrases, cfg)?;
    let (records, infer_skipped) =
        importers::infer_overlay_qstrings(records, &db::load_primary_character_readings(conn)?);
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.overlay_phrases)?,
        "overlay",
        &sha256_file(&paths.overlay_phrases)?,
        seen,
        parse_skipped + infer_skipped,
        true,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_opencc_variant_policy(
    conn: &mut Connection,
    cfg: &Config,
    _paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let rows = db::load_unigram_rows(conn)?;
    let (records, seen, skipped) = importers::generate_opencc_variant_demotions(
        &rows,
        &cfg.opencc_binary,
        &cfg.opencc_t2tw_config,
    )?;
    let source_path = "generated/opencc-t2tw-variant-demotions";
    let source_sha256 = sha256_bytes(b"opencc-t2tw qstring variant demotions v1");
    let result = db::apply_qstring_variant_demotions(
        conn,
        &records,
        source_path,
        "opencc-variant-demotion",
        &source_sha256,
        seen,
        skipped,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_single_char_homophone_rerank(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let existing_records = db::load_existing_phrase_weights(conn)?;
    let (records, seen, skipped) = importers::parse_single_char_homophone_reranks(
        &paths.rime_essay_raw,
        &existing_records,
        cfg.homophone_rerank_min_ratio,
    )?;
    let result = db::apply_records(
        conn,
        records,
        &format!(
            "{}#homophone-rerank",
            repo_relative(&cfg.root, &paths.rime_essay_raw)?
        ),
        "rime-homophone-rerank",
        &sha256_file(&paths.rime_essay_raw)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_fragment_demotions(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) = importers::parse_fragment_demotions(&paths.fragment_demotions)?;
    let result = db::apply_variant_demotions(
        conn,
        &records,
        &repo_relative(&cfg.root, &paths.fragment_demotions)?,
        "fragment-demotion",
        &sha256_file(&paths.fragment_demotions)?,
        seen,
        skipped,
        config::FRAGMENT_DENYLIST_SOURCE_ID,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_explicit_overlay(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) = importers::parse_explicit_overlay(&paths.overlay_explicit, cfg)?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.overlay_explicit)?,
        "overlay-explicit-qstring",
        &sha256_file(&paths.overlay_explicit)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_chiaki_web_overlay(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) =
        importers::parse_chiaki_web_overlay(&paths.chiaki_web_overlay_explicit, cfg)?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.chiaki_web_overlay_explicit)?,
        "chiaki-web-explicit-qstring",
        &sha256_file(&paths.chiaki_web_overlay_explicit)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_chiaki_synthetic_overlay(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) =
        importers::parse_chiaki_synthetic_overlay(&paths.chiaki_synthetic_unigrams, cfg)?;
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.chiaki_synthetic_unigrams)?,
        "chiaki-synthetic-unigrams",
        &sha256_file(&paths.chiaki_synthetic_unigrams)?,
        seen,
        skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_chiakey_auto_hotwords_overlay(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, parse_skipped) =
        importers::parse_auto_hotwords_overlay(&paths.chiakey_auto_hotwords_phrases, cfg)?;
    let (records, infer_skipped) =
        importers::infer_overlay_qstrings(records, &db::load_primary_character_readings(conn)?);
    let result = db::apply_records(
        conn,
        records,
        &repo_relative(&cfg.root, &paths.chiakey_auto_hotwords_phrases)?,
        "chiakey-auto-hotwords",
        &sha256_file(&paths.chiakey_auto_hotwords_phrases)?,
        seen,
        parse_skipped + infer_skipped,
        false,
    )?;
    remember_records(source_keys, &result);
    import_results.push(result);
    Ok(())
}

fn import_chiaki_web_bigrams(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) =
        importers::parse_bigram_overlay(&paths.chiaki_web_overlay_bigrams, cfg)?;
    let existing_phrases = db::load_existing_phrases(conn)?;
    let qstring_weights = db::load_best_qstring_weights(conn)?;
    let phrase_weights = db::load_best_unigram_weights_by_current(conn)?;
    let joined_records = importers::joined_phrase_records_from_bigrams(
        &records,
        &existing_phrases,
        &qstring_weights,
        &phrase_weights,
        cfg.max_phrase_codepoints,
        config::CHIAKI_WEB_OVERLAY_SOURCE_ID,
    );
    let joined_seen = records.len();
    let joined_skipped = joined_seen.saturating_sub(joined_records.len());
    let joined_result = db::apply_records(
        conn,
        joined_records,
        "generated/chiaki-web-bigram-joined-phrases",
        "chiaki-web-bigram-joined-phrases",
        &sha256_file(&paths.chiaki_web_overlay_bigrams)?,
        joined_seen,
        joined_skipped,
        false,
    )?;
    remember_records(source_keys, &joined_result);
    import_results.push(joined_result);

    let result = db::apply_bigram_records(
        conn,
        &records,
        &repo_relative(&cfg.root, &paths.chiaki_web_overlay_bigrams)?,
        "chiaki-web-bigram-overlay",
        &sha256_file(&paths.chiaki_web_overlay_bigrams)?,
        seen,
        skipped,
    )?;
    import_results.push(result);
    Ok(())
}

fn import_chiaki_synthetic_bigrams(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) =
        importers::parse_bigram_overlay(&paths.chiaki_synthetic_bigrams, cfg)?;
    let unigrams = db::load_best_unigram_weights_by_current(conn)?;
    let records = importers::calibrate_bigram_boost(records, cfg.synthetic_bigram_boost, &unigrams);
    let result = db::apply_bigram_records(
        conn,
        &records,
        &repo_relative(&cfg.root, &paths.chiaki_synthetic_bigrams)?,
        "chiaki-synthetic-bigrams",
        &sha256_file(&paths.chiaki_synthetic_bigrams)?,
        seen,
        skipped,
    )?;
    import_results.push(result);
    Ok(())
}

fn import_openformosa_common_voice_bigrams(
    conn: &mut Connection,
    cfg: &Config,
    paths: &ReleasePaths,
    import_results: &mut Vec<ImportResult>,
) -> Result<()> {
    let (records, seen, skipped) =
        importers::parse_bigram_overlay(&paths.openformosa_common_voice_bigrams, cfg)?;
    let unigrams = db::load_best_unigram_weights_by_current(conn)?;
    let records =
        importers::calibrate_bigram_boost(records, cfg.commonvoice_bigram_boost, &unigrams);
    let result = db::apply_bigram_records(
        conn,
        &records,
        &repo_relative(&cfg.root, &paths.openformosa_common_voice_bigrams)?,
        "openformosa-common-voice-bigrams",
        &sha256_file(&paths.openformosa_common_voice_bigrams)?,
        seen,
        skipped,
    )?;
    import_results.push(result);
    Ok(())
}

fn remember_records(
    source_keys: &mut HashMap<(String, String), SourceRecord>,
    result: &ImportResult,
) {
    for record in &result.records {
        source_keys.insert(
            (record.qstring.clone(), record.phrase.clone()),
            record.clone(),
        );
    }
}

fn module_cin_files(paths: &ReleasePaths) -> Vec<std::path::PathBuf> {
    vec![
        paths.bopomofo_correction_cin.clone(),
        paths.cj_ext_cin.clone(),
        paths.cj_punctuations_halfwidth_cin.clone(),
        paths.cj_punctuations_mixedwidth_cin.clone(),
        paths.simplex_ext_cin.clone(),
    ]
}

fn print_summary(
    cfg: &Config,
    paths: &ReleasePaths,
    counts: &Value,
    import_results: &[ImportResult],
) {
    println!("Prepared ChiaKey Lexicon {}", cfg.release_version);
    println!("  DB: {}", paths.db.display());
    println!("  Metadata: {}", paths.metadata.display());
    println!("  Manifest: {}", cfg.manifest_path.display());
    println!("  Checksums: {}", paths.checksum.display());
    println!(
        "  Normalized TSV: {} ({} rows)",
        cfg.normalized_path.display(),
        counts
            .get("normalized_rows")
            .and_then(Value::as_i64)
            .unwrap_or_default()
    );
    println!("  Imported:");
    for result in import_results {
        println!(
            "    {}: seen={} added={} skipped={}",
            result.source_path, result.seen, result.added, result.skipped
        );
    }
}
