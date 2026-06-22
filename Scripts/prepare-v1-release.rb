#!/usr/bin/env ruby
# frozen_string_literal: true

require "csv"
require "digest"
require "fileutils"
require "json"
require "open3"
require "pathname"
require "set"
require "time"

ROOT = File.expand_path("..", __dir__)

BONEYARD_SOURCE_ID = "keykey-boneyard-bootstrap"
BONEYARD_SOURCE_NAME = "KeyKey Boneyard bootstrap data"
LIBCHEWING_SOURCE_ID = "libchewing-data"
LIBCHEWING_SOURCE_NAME = "libchewing-data Traditional Chinese Zhuyin dictionary"
RIME_ESSAY_SOURCE_ID = "rime-essay"
RIME_ESSAY_SOURCE_NAME = "Rime essay shared vocabulary and language model"
OVERLAY_SOURCE_ID = "chiaki-modern-overlay"
OVERLAY_SOURCE_NAME = "Chiaki modern overlay phrases"

RELEASE_VERSION = ENV.fetch("LEXICON_VERSION", "2026.06.3")
LANGUAGE_MODEL_VERSION = "chiaki-modern-#{RELEASE_VERSION}"
MINIMUM_APP_VERSION = ENV.fetch("MINIMUM_APP_VERSION", "0.1.0")
DATABASE_SCHEMA_VERSION = 1
GENERATED_AT = Time.now.utc.iso8601
RELEASE_BASE_URL = ENV.fetch(
  "RELEASE_BASE_URL",
  "https://github.com/akira02/Chiaki-KeyKey-Lexicon/releases/download/#{RELEASE_VERSION}"
)

MAX_PHRASE_CODEPOINTS = Integer(ENV.fetch("MAX_PHRASE_CODEPOINTS", "7"))
RIME_ESSAY_MIN_SCORE = Integer(ENV.fetch("RIME_ESSAY_MIN_SCORE", "40"))

BONEYARD_ROOT = File.expand_path(
  ENV.fetch("KEYKEY_BONEYARD_ROOT", File.join(ROOT, "..", "KeyKey-Boneyard"))
)
BONEYARD_SOURCE_ROOT = File.join(BONEYARD_ROOT, "YahooKeyKey-Source-1.1.2528")
BONEYARD_DB = File.join(
  BONEYARD_SOURCE_ROOT,
  "Distributions",
  "Takao",
  "CookedDatabase",
  "KeyKeySource.db"
)

DIST_DIR = File.join(ROOT, "dist", RELEASE_VERSION)
NORMALIZED_PATH = File.join(ROOT, "normalized", "smart-mandarin.tsv")
BONEYARD_SOURCE_DIR = File.join(ROOT, "sources", BONEYARD_SOURCE_ID)
LIBCHEWING_SOURCE_DIR = File.join(ROOT, "sources", LIBCHEWING_SOURCE_ID)
RIME_ESSAY_SOURCE_DIR = File.join(ROOT, "sources", RIME_ESSAY_SOURCE_ID)
OVERLAY_SOURCE_DIR = File.join(ROOT, "sources", OVERLAY_SOURCE_ID)
OVERLAY_PHRASES_PATH = File.join(OVERLAY_SOURCE_DIR, "phrases.tsv")
BONEYARD_SOURCE_INVENTORY_PATH = File.join(BONEYARD_SOURCE_DIR, "source-inventory.sha256")
LIBCHEWING_SOURCE_INVENTORY_PATH = File.join(LIBCHEWING_SOURCE_DIR, "source-inventory.sha256")
RIME_ESSAY_SOURCE_INVENTORY_PATH = File.join(RIME_ESSAY_SOURCE_DIR, "source-inventory.sha256")
MANIFEST_PATH = File.join(ROOT, "manifests", "lexicon-manifest.json")
DIST_MANIFEST_PATH = File.join(DIST_DIR, "lexicon-manifest.json")
DB_FILENAME = "KeyKeySource-#{RELEASE_VERSION}.db"
METADATA_FILENAME = "KeyKeySource-#{RELEASE_VERSION}.json"
CHECKSUM_FILENAME = "SHA256SUMS"
DB_PATH = File.join(DIST_DIR, DB_FILENAME)
METADATA_PATH = File.join(DIST_DIR, METADATA_FILENAME)
CHECKSUM_PATH = File.join(DIST_DIR, CHECKSUM_FILENAME)

LIBCHEWING_RAW_FILES = [
  {
    path: File.join(LIBCHEWING_SOURCE_DIR, "raw", "dict", "chewing", "tsi.csv"),
    kind: "libchewing-phrase",
    min_codepoints: 2,
    max_codepoints: MAX_PHRASE_CODEPOINTS,
    replace_phrases: true
  },
  {
    path: File.join(LIBCHEWING_SOURCE_DIR, "raw", "dict", "chewing", "alt.csv"),
    kind: "libchewing-alternate",
    min_codepoints: 2,
    max_codepoints: MAX_PHRASE_CODEPOINTS,
    replace_phrases: true
  },
  {
    path: File.join(LIBCHEWING_SOURCE_DIR, "raw", "dict", "chewing", "word.csv"),
    kind: "libchewing-character",
    min_codepoints: 1,
    max_codepoints: 1,
    replace_phrases: false
  }
].freeze

RIME_ESSAY_RAW_PATH = File.join(RIME_ESSAY_SOURCE_DIR, "raw", "essay.txt")

COMPONENTS = {
  "ㄅ" => 0x0001, "ㄆ" => 0x0002, "ㄇ" => 0x0003, "ㄈ" => 0x0004,
  "ㄉ" => 0x0005, "ㄊ" => 0x0006, "ㄋ" => 0x0007, "ㄌ" => 0x0008,
  "ㄍ" => 0x0009, "ㄎ" => 0x000a, "ㄏ" => 0x000b, "ㄐ" => 0x000c,
  "ㄑ" => 0x000d, "ㄒ" => 0x000e, "ㄓ" => 0x000f, "ㄔ" => 0x0010,
  "ㄕ" => 0x0011, "ㄖ" => 0x0012, "ㄗ" => 0x0013, "ㄘ" => 0x0014,
  "ㄙ" => 0x0015, "ㄧ" => 0x0020, "ㄨ" => 0x0040, "ㄩ" => 0x0060,
  "ㄚ" => 0x0080, "ㄛ" => 0x0100, "ㄜ" => 0x0180, "ㄝ" => 0x0200,
  "ㄞ" => 0x0280, "ㄟ" => 0x0300, "ㄠ" => 0x0380, "ㄡ" => 0x0400,
  "ㄢ" => 0x0480, "ㄣ" => 0x0500, "ㄤ" => 0x0580, "ㄥ" => 0x0600,
  "ㄦ" => 0x0680, "ˊ" => 0x0800, "ˇ" => 0x1000, "ˋ" => 0x1800,
  "˙" => 0x2000
}.freeze

BPMF_COMPONENTS = COMPONENTS.keys.freeze

SourceRecord = Struct.new(:qstring, :phrase, :weight, :source_id, :source_path, :kind, :tags, keyword_init: true)
ImportResult = Struct.new(:source_path, :kind, :sha256, :seen, :added, :skipped, :records, keyword_init: true)

def sql(value)
  "'#{value.to_s.gsub("'", "''")}'"
end

def run_sqlite(db_path, sql_text)
  stdout, stderr, status = Open3.capture3("/usr/bin/sqlite3", db_path, stdin_data: sql_text)
  return stdout if status.success?

  warn stderr
  warn stdout
  exit(status.exitstatus || 1)
end

def run_sqlite_json(db_path, sql_text)
  stdout, stderr, status = Open3.capture3("/usr/bin/sqlite3", "-json", db_path, sql_text)
  unless status.success?
    warn stderr
    warn stdout
    exit(status.exitstatus || 1)
  end

  JSON.parse(stdout.empty? ? "[]" : stdout)
end

def sha256(path)
  Digest::SHA256.file(path).hexdigest
end

def file_info(path)
  {
    "sha256" => sha256(path),
    "size" => File.size(path)
  }
end

def relative_to(path, root)
  Pathname.new(path).relative_path_from(Pathname.new(root)).to_s
end

def repo_relative(path)
  relative_to(path, ROOT)
end

def phrase_codepoints(phrase)
  phrase.each_char.count
end

def absolute_order_string(components)
  syllable = 0
  components.each { |component| syllable |= COMPONENTS.fetch(component) }
  order = (syllable & 0x001f) +
          (((syllable & 0x0060) >> 5) * 22) +
          (((syllable & 0x0780) >> 7) * 22 * 4) +
          (((syllable & 0x3800) >> 11) * 22 * 4 * 14)

  (48 + (order % 79)).chr + (48 + (order / 79)).chr
end

def qstring_for_bpmf_syllable(syllable)
  components = syllable.each_char.select { |char| COMPONENTS.key?(char) }
  return nil if components.empty?

  absolute_order_string(components)
rescue KeyError
  nil
end

def qstring_for_bpmf_sequence(sequence)
  syllables = sequence.to_s.split(/[,\s]+/).reject(&:empty?)
  return nil if syllables.empty?

  qstrings = syllables.map { |syllable| qstring_for_bpmf_syllable(syllable) }
  return nil if qstrings.any?(&:nil?)

  [qstrings.join, syllables.length]
end

def bopomofo_candidate?(text)
  text.each_char.any? { |char| BPMF_COMPONENTS.include?(char) }
end

def phrase_candidate?(text, min_codepoints: 1, max_codepoints: MAX_PHRASE_CODEPOINTS)
  return false if text.empty? || text.include?("\t") || text.include?("\n")
  return false if bopomofo_candidate?(text)
  return false if text =~ %r{https?://}

  length = phrase_codepoints(text)
  length >= min_codepoints && length <= max_codepoints
end

def source_files
  data_source = File.join(BONEYARD_SOURCE_ROOT, "Distributions", "Takao", "DataSource")
  [
    File.join(BONEYARD_SOURCE_ROOT, "DataTables", "bpmf.cin"),
    *Dir[File.join(data_source, "Addendum", "*.txt")].sort,
    *Dir[File.join(data_source, "Overrides", "*.txt")].sort,
    *Dir[File.join(data_source, "Modern", "*.txt")].sort
  ]
end

def verify_required_files!(paths)
  missing = paths.reject { |path| File.file?(path) }
  return if missing.empty?

  warn "Missing required file(s):"
  missing.each { |path| warn "  #{path}" }
  if missing.any? { |path| path.include?("/sources/#{LIBCHEWING_SOURCE_ID}/raw/") || path.include?("/sources/#{RIME_ESSAY_SOURCE_ID}/raw/") }
    warn ""
    warn "Run Scripts/fetch-modern-sources.rb to download pinned modern lexicon sources."
  end
  exit 1
end

def write_inventory(path, root, files, sort: false)
  source_files = sort ? files.sort_by { |source_file| relative_to(source_file, root) } : files
  lines = source_files.map do |source_file|
    "#{sha256(source_file)}  #{relative_to(source_file, root)}"
  end
  File.write(path, "#{lines.join("\n")}\n")
end

def libchewing_max_score(paths)
  max_score = 1

  paths.each do |path|
    File.foreach(path, chomp: true) do |line|
      next if line.strip.empty? || line.start_with?("#")

      row = CSV.parse_line(line)
      next unless row && row.length >= 2

      score = Integer(row[1], exception: false)
      max_score = score if score && score > max_score
    end
  end

  max_score
end

def libchewing_weight(score, max_score)
  return -2.8 if score <= 0

  ratio = Math.log(score + 1) / Math.log(max_score + 1)
  (-0.25 - (2.35 * (1.0 - ratio))).round(6)
end

def rime_weight(score, max_score)
  ratio = Math.log(score + 1) / Math.log(max_score + 1)
  (-1.35 - (1.85 * (1.0 - ratio))).round(6)
end

def dedupe_records(records)
  records.each_with_object({}) do |record, hash|
    key = [record.qstring, record.phrase]
    existing = hash[key]
    hash[key] = record if existing.nil? || record.weight > existing.weight
  end.values
end

def parse_libchewing_csv(path, kind:, source_id:, max_score:, min_codepoints:, max_codepoints:, existing_exact_keys: nil)
  source_path = repo_relative(path)
  seen = 0
  skipped = 0
  records = []

  File.foreach(path, chomp: true) do |line|
    next if line.strip.empty? || line.start_with?("#")

    seen += 1
    row = CSV.parse_line(line)
    unless row && row.length >= 3
      skipped += 1
      next
    end

    phrase = row[0].to_s
    score = Integer(row[1], exception: false)
    reading = row[2].to_s
    qstring_result = qstring_for_bpmf_sequence(reading)

    unless score && qstring_result && phrase_candidate?(phrase, min_codepoints: min_codepoints, max_codepoints: max_codepoints)
      skipped += 1
      next
    end

    qstring, syllable_count = qstring_result
    unless syllable_count == phrase_codepoints(phrase)
      skipped += 1
      next
    end

    if existing_exact_keys&.include?([qstring, phrase])
      skipped += 1
      next
    end

    length = phrase_codepoints(phrase)
    weight = length == 1 ? -3.2 : libchewing_weight(score, max_score)
    tags = ["unigram", LIBCHEWING_SOURCE_ID, kind.delete_prefix("libchewing-")]

    records << SourceRecord.new(
      qstring: qstring,
      phrase: phrase,
      weight: weight,
      source_id: source_id,
      source_path: source_path,
      kind: kind,
      tags: tags.join(",")
    )
  end

  [dedupe_records(records), seen, skipped]
end

def load_existing_exact_keys(db_path)
  run_sqlite_json(
    db_path,
    "SELECT qstring, current AS phrase FROM unigrams WHERE current <> '';"
  ).each_with_object(Set.new) do |row, set|
    set << [row.fetch("qstring"), row.fetch("phrase")]
  end
end

def load_existing_phrases(db_path)
  run_sqlite_json(
    db_path,
    "SELECT DISTINCT current AS phrase FROM unigrams WHERE current <> '';"
  ).each_with_object(Set.new) do |row, set|
    set << row.fetch("phrase")
  end
end

def load_primary_character_readings(db_path)
  rows = run_sqlite_json(
    db_path,
    "SELECT qstring, current AS phrase, probability AS weight FROM unigrams WHERE current <> '' ORDER BY current, probability DESC, qstring;"
  )

  rows.each_with_object({}) do |row, hash|
    phrase = row.fetch("phrase")
    next unless phrase_codepoints(phrase) == 1
    next if hash.key?(phrase)

    hash[phrase] = row.fetch("qstring")
  end
end

def parse_rime_essay(path, char_readings:, existing_phrases:)
  source_path = repo_relative(path)
  raw_rows = []
  seen = 0
  skipped = 0
  max_score = 1

  File.foreach(path, chomp: true) do |line|
    next if line.strip.empty? || line.start_with?("#")

    seen += 1
    phrase, score_text = line.split("\t", 2)
    score = Integer(score_text, exception: false)
    unless score && score >= RIME_ESSAY_MIN_SCORE &&
           phrase_candidate?(phrase, min_codepoints: 2, max_codepoints: MAX_PHRASE_CODEPOINTS) &&
           !existing_phrases.include?(phrase)
      skipped += 1
      next
    end

    qstrings = phrase.each_char.map { |char| char_readings[char] }
    if qstrings.any?(&:nil?)
      skipped += 1
      next
    end

    max_score = score if score > max_score
    raw_rows << [phrase, score, qstrings.join]
  end

  records = raw_rows.map do |phrase, score, qstring|
    SourceRecord.new(
      qstring: qstring,
      phrase: phrase,
      weight: rime_weight(score, max_score),
      source_id: RIME_ESSAY_SOURCE_ID,
      source_path: source_path,
      kind: "rime-supplement",
      tags: "unigram,#{RIME_ESSAY_SOURCE_ID},supplemental"
    )
  end

  [dedupe_records(records), seen, skipped]
end

def parse_overlay(path)
  return [[], 0, 0] unless File.file?(path)

  seen = 0
  skipped = 0
  records = []

  File.foreach(path, chomp: true).with_index(1) do |line, line_number|
    next if line.strip.empty? || line.start_with?("#")

    seen += 1
    phrase, weight, tags = line.split("\t", 3)
    unless phrase && weight && phrase_candidate?(phrase, min_codepoints: 1, max_codepoints: MAX_PHRASE_CODEPOINTS)
      skipped += 1
      next
    end

    Float(weight)
    records << SourceRecord.new(
      phrase: phrase,
      weight: weight.to_f,
      source_id: OVERLAY_SOURCE_ID,
      source_path: repo_relative(path),
      kind: "overlay",
      tags: "unigram,#{tags}"
    )
  rescue ArgumentError
    warn "Invalid overlay weight #{path}:#{line_number}: #{weight.inspect}"
    exit 1
  end

  [records, seen, skipped]
end

def infer_overlay_qstrings(records, db_path)
  char_readings = load_primary_character_readings(db_path)
  skipped = 0
  inferred = []

  records.each do |record|
    qstrings = record.phrase.each_char.map { |char| char_readings[char] }
    if qstrings.any?(&:nil?)
      skipped += 1
      next
    end

    record.qstring = qstrings.join
    inferred << record
  end

  [dedupe_records(inferred), skipped]
end

def apply_records!(db_path, records, source_path:, kind:, source_sha256:, seen:, skipped:, replace_phrases: false)
  records = dedupe_records(records)
  sql_lines = ["BEGIN;"]

  if replace_phrases && records.any?
    sql_lines << "CREATE TEMP TABLE chiaki_import_replace_phrases (phrase TEXT PRIMARY KEY);"
    records.map(&:phrase).uniq.each do |phrase|
      sql_lines << "INSERT OR IGNORE INTO chiaki_import_replace_phrases VALUES(#{sql(phrase)});"
    end
    sql_lines << "DELETE FROM unigrams WHERE current IN (SELECT phrase FROM chiaki_import_replace_phrases);"
    sql_lines << "DELETE FROM 'Mandarin-bpmf-cin' WHERE value IN (SELECT phrase FROM chiaki_import_replace_phrases);"
  end

  records.each do |record|
    sql_lines << "DELETE FROM unigrams WHERE qstring = #{sql(record.qstring)} AND current = #{sql(record.phrase)};"
    sql_lines << "INSERT INTO unigrams VALUES(#{sql(record.qstring)}, #{sql(record.phrase)}, #{record.weight}, 0.0);"
    sql_lines << "INSERT INTO 'Mandarin-bpmf-cin' SELECT #{sql(record.qstring)}, #{sql(record.phrase)} WHERE NOT EXISTS (SELECT 1 FROM 'Mandarin-bpmf-cin' WHERE key = #{sql(record.qstring)} AND value = #{sql(record.phrase)});"
  end

  sql_lines << "DROP TABLE IF EXISTS chiaki_import_replace_phrases;" if replace_phrases
  sql_lines << "DELETE FROM chiaki_db_sources WHERE source = #{sql(source_path)};"
  sql_lines << "INSERT INTO chiaki_db_sources VALUES(#{sql(source_path)}, #{sql(kind)}, #{sql(source_sha256)}, #{seen}, #{records.length}, #{skipped});"
  sql_lines << "COMMIT;"

  run_sqlite(db_path, sql_lines.join("\n"))

  ImportResult.new(
    source_path: source_path,
    kind: kind,
    sha256: source_sha256,
    seen: seen,
    added: records.length,
    skipped: skipped,
    records: records
  )
end

def refresh_metadata_counts!(db_path)
  run_sqlite(
    db_path,
    [
      "BEGIN;",
      "DELETE FROM chiaki_db_metadata WHERE key IN ('unigram_count', 'candidate_count');",
      "INSERT INTO chiaki_db_metadata VALUES('unigram_count', (SELECT COUNT(*) FROM unigrams));",
      "INSERT INTO chiaki_db_metadata VALUES('candidate_count', (SELECT COUNT(*) FROM 'Mandarin-bpmf-cin' WHERE key NOT LIKE '__property_%'));",
      "COMMIT;"
    ].join("\n")
  )
end

def write_json(path, data)
  File.write(path, "#{JSON.pretty_generate(data)}\n")
end

def stats_for_source_rows(source_rows, prefix_or_path)
  source_rows.select do |row|
    source = row.fetch("source")
    source == prefix_or_path || source.start_with?(prefix_or_path)
  end
end

verify_required_files!([
  BONEYARD_DB,
  *source_files,
  *LIBCHEWING_RAW_FILES.map { |entry| entry.fetch(:path) },
  RIME_ESSAY_RAW_PATH,
  OVERLAY_PHRASES_PATH
])

FileUtils.mkdir_p(DIST_DIR)
FileUtils.mkdir_p(File.dirname(NORMALIZED_PATH))
FileUtils.mkdir_p(BONEYARD_SOURCE_DIR)
FileUtils.mkdir_p(LIBCHEWING_SOURCE_DIR)
FileUtils.mkdir_p(RIME_ESSAY_SOURCE_DIR)
FileUtils.mkdir_p(OVERLAY_SOURCE_DIR)

write_inventory(BONEYARD_SOURCE_INVENTORY_PATH, BONEYARD_ROOT, source_files)
write_inventory(LIBCHEWING_SOURCE_INVENTORY_PATH, LIBCHEWING_SOURCE_DIR, LIBCHEWING_RAW_FILES.map { |entry| entry.fetch(:path) }, sort: true)
write_inventory(RIME_ESSAY_SOURCE_INVENTORY_PATH, RIME_ESSAY_SOURCE_DIR, [RIME_ESSAY_RAW_PATH], sort: true)

boneyard_source_inventory_info = file_info(BONEYARD_SOURCE_INVENTORY_PATH)
libchewing_source_inventory_info = file_info(LIBCHEWING_SOURCE_INVENTORY_PATH)
rime_essay_source_inventory_info = file_info(RIME_ESSAY_SOURCE_INVENTORY_PATH)
overlay_info = file_info(OVERLAY_PHRASES_PATH)

FileUtils.cp(BONEYARD_DB, DB_PATH)

source_keys = {}
import_results = []
libchewing_phrase_max_score = libchewing_max_score(
  LIBCHEWING_RAW_FILES
    .select { |entry| entry.fetch(:min_codepoints) >= 2 }
    .map { |entry| entry.fetch(:path) }
)

LIBCHEWING_RAW_FILES.each do |entry|
  existing_exact_keys = entry.fetch(:min_codepoints) == 1 ? load_existing_exact_keys(DB_PATH) : nil
  records, seen, skipped = parse_libchewing_csv(
    entry.fetch(:path),
    kind: entry.fetch(:kind),
    source_id: LIBCHEWING_SOURCE_ID,
    max_score: libchewing_phrase_max_score,
    min_codepoints: entry.fetch(:min_codepoints),
    max_codepoints: entry.fetch(:max_codepoints),
    existing_exact_keys: existing_exact_keys
  )
  result = apply_records!(
    DB_PATH,
    records,
    source_path: repo_relative(entry.fetch(:path)),
    kind: entry.fetch(:kind),
    source_sha256: sha256(entry.fetch(:path)),
    seen: seen,
    skipped: skipped,
    replace_phrases: entry.fetch(:replace_phrases)
  )
  import_results << result
  result.records.each { |record| source_keys[[record.qstring, record.phrase]] = record }
end

rime_records, rime_seen, rime_skipped = parse_rime_essay(
  RIME_ESSAY_RAW_PATH,
  char_readings: load_primary_character_readings(DB_PATH),
  existing_phrases: load_existing_phrases(DB_PATH)
)
rime_result = apply_records!(
  DB_PATH,
  rime_records,
  source_path: repo_relative(RIME_ESSAY_RAW_PATH),
  kind: "rime-supplement",
  source_sha256: sha256(RIME_ESSAY_RAW_PATH),
  seen: rime_seen,
  skipped: rime_skipped,
  replace_phrases: false
)
import_results << rime_result
rime_result.records.each { |record| source_keys[[record.qstring, record.phrase]] = record }

overlay_records, overlay_seen, overlay_skipped = parse_overlay(OVERLAY_PHRASES_PATH)
overlay_records, overlay_infer_skipped = infer_overlay_qstrings(overlay_records, DB_PATH)
overlay_result = apply_records!(
  DB_PATH,
  overlay_records,
  source_path: repo_relative(OVERLAY_PHRASES_PATH),
  kind: "overlay",
  source_sha256: sha256(OVERLAY_PHRASES_PATH),
  seen: overlay_seen,
  skipped: overlay_skipped + overlay_infer_skipped,
  replace_phrases: true
)
import_results << overlay_result
overlay_result.records.each { |record| source_keys[[record.qstring, record.phrase]] = record }

refresh_metadata_counts!(DB_PATH)

run_sqlite(
  DB_PATH,
  [
    "BEGIN;",
    "DELETE FROM cooked_information WHERE key = 'version';",
    "INSERT INTO cooked_information VALUES('version', #{sql(LANGUAGE_MODEL_VERSION)});",
    "DELETE FROM chiaki_db_metadata WHERE key IN ('version', 'lexicon_release_version', 'lexicon_release_generator', 'generated_at');",
    "INSERT INTO chiaki_db_metadata VALUES('version', #{sql(LANGUAGE_MODEL_VERSION)});",
    "INSERT INTO chiaki_db_metadata VALUES('lexicon_release_version', #{sql(RELEASE_VERSION)});",
    "INSERT INTO chiaki_db_metadata VALUES('lexicon_release_generator', 'Scripts/prepare-v1-release.rb');",
    "INSERT INTO chiaki_db_metadata VALUES('generated_at', #{sql(GENERATED_AT)});",
    "COMMIT;"
  ].join("\n")
)

unigrams = run_sqlite_json(
  DB_PATH,
  "SELECT qstring AS reading, current AS phrase, probability AS weight FROM unigrams WHERE current <> '' ORDER BY qstring, current;"
)

File.open(NORMALIZED_PATH, "w") do |file|
  unigrams.each do |row|
    phrase = row.fetch("phrase")
    next if phrase.include?("\t") || phrase.include?("\n")

    source_record = source_keys[[row.fetch("reading"), phrase]]
    source_id = source_record ? source_record.source_id : BONEYARD_SOURCE_ID
    tags = source_record ? source_record.tags : "unigram,keykey-boneyard"

    file.puts [
      row.fetch("reading"),
      phrase,
      row.fetch("weight"),
      source_id,
      tags
    ].join("\t")
  end
end

metadata_rows = run_sqlite_json(
  DB_PATH,
  "SELECT key, value FROM chiaki_db_metadata ORDER BY key;"
)
metadata = metadata_rows.each_with_object({}) { |row, hash| hash[row.fetch("key")] = row.fetch("value") }

source_rows = run_sqlite_json(
  DB_PATH,
  "SELECT source, kind, sha256, seen, added, skipped FROM chiaki_db_sources ORDER BY source;"
)

counts = {
  "unigrams" => run_sqlite_json(DB_PATH, "SELECT COUNT(*) AS count FROM unigrams;").first.fetch("count"),
  "bigrams" => run_sqlite_json(DB_PATH, "SELECT COUNT(*) AS count FROM bigrams;").first.fetch("count"),
  "candidate_rows" => metadata.fetch("candidate_count").to_i,
  "mandarin_bpmf_cin_rows" => run_sqlite_json(DB_PATH, "SELECT COUNT(*) AS count FROM 'Mandarin-bpmf-cin';").first.fetch("count"),
  "normalized_rows" => File.foreach(NORMALIZED_PATH).count
}

db_info = file_info(DB_PATH)
normalized_info = file_info(NORMALIZED_PATH)

release_metadata = {
  "schema" => 1,
  "version" => RELEASE_VERSION,
  "generated_at" => GENERATED_AT,
  "language_model_version" => LANGUAGE_MODEL_VERSION,
  "database_schema_version" => DATABASE_SCHEMA_VERSION,
  "database" => {
    "filename" => DB_FILENAME,
    "sha256" => db_info.fetch("sha256"),
    "size" => db_info.fetch("size"),
    "metadata" => metadata,
    "counts" => counts
  },
  "normalized" => {
    "path" => "normalized/smart-mandarin.tsv",
    "sha256" => normalized_info.fetch("sha256"),
    "size" => normalized_info.fetch("size"),
    "rows" => counts.fetch("normalized_rows"),
    "format" => "reading<TAB>phrase<TAB>weight<TAB>source_id<TAB>tags"
  },
  "sources" => [
    {
      "id" => BONEYARD_SOURCE_ID,
      "name" => BONEYARD_SOURCE_NAME,
      "license" => "BSD-3-Clause-style",
      "attribution" => "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / Chiaki KeyKey maintainers",
      "inventory" => {
        "path" => "sources/#{BONEYARD_SOURCE_ID}/source-inventory.sha256",
        "sha256" => boneyard_source_inventory_info.fetch("sha256"),
        "size" => boneyard_source_inventory_info.fetch("size")
      },
      "stats" => stats_for_source_rows(source_rows, "YahooKeyKey-Source-1.1.2528/")
    },
    {
      "id" => LIBCHEWING_SOURCE_ID,
      "name" => LIBCHEWING_SOURCE_NAME,
      "license" => "LGPL-2.1-or-later",
      "attribution" => "libchewing Core Team",
      "inventory" => {
        "path" => "sources/#{LIBCHEWING_SOURCE_ID}/source-inventory.sha256",
        "sha256" => libchewing_source_inventory_info.fetch("sha256"),
        "size" => libchewing_source_inventory_info.fetch("size")
      },
      "stats" => stats_for_source_rows(source_rows, "sources/#{LIBCHEWING_SOURCE_ID}/raw/")
    },
    {
      "id" => RIME_ESSAY_SOURCE_ID,
      "name" => RIME_ESSAY_SOURCE_NAME,
      "license" => "LGPL-3.0",
      "attribution" => "Rime essay contributors",
      "inventory" => {
        "path" => "sources/#{RIME_ESSAY_SOURCE_ID}/source-inventory.sha256",
        "sha256" => rime_essay_source_inventory_info.fetch("sha256"),
        "size" => rime_essay_source_inventory_info.fetch("size")
      },
      "stats" => stats_for_source_rows(source_rows, "sources/#{RIME_ESSAY_SOURCE_ID}/raw/")
    },
    {
      "id" => OVERLAY_SOURCE_ID,
      "name" => OVERLAY_SOURCE_NAME,
      "license" => "CC0-1.0",
      "attribution" => "Chiaki KeyKey Lexicon maintainers",
      "path" => "sources/#{OVERLAY_SOURCE_ID}/phrases.tsv",
      "sha256" => overlay_info.fetch("sha256"),
      "size" => overlay_info.fetch("size"),
      "stats" => stats_for_source_rows(source_rows, "sources/#{OVERLAY_SOURCE_ID}/phrases.tsv")
    }
  ]
}
write_json(METADATA_PATH, release_metadata)
metadata_info = file_info(METADATA_PATH)

checksum_lines = [
  [db_info.fetch("sha256"), DB_FILENAME],
  [metadata_info.fetch("sha256"), METADATA_FILENAME]
].map { |hash, filename| "#{hash}  #{filename}" }
File.write(CHECKSUM_PATH, "#{checksum_lines.join("\n")}\n")
checksum_info = file_info(CHECKSUM_PATH)

manifest = {
  "schema" => 1,
  "version" => RELEASE_VERSION,
  "generated_at" => GENERATED_AT,
  "minimum_app_version" => MINIMUM_APP_VERSION,
  "database_schema_version" => DATABASE_SCHEMA_VERSION,
  "sources" => [
    {
      "id" => BONEYARD_SOURCE_ID,
      "name" => BONEYARD_SOURCE_NAME,
      "url" => "https://github.com/vChewing/KeyKey-Boneyard",
      "format" => "sqlite",
      "license" => "BSD-3-Clause-style",
      "attribution" => "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / Chiaki KeyKey maintainers",
      "sha256" => boneyard_source_inventory_info.fetch("sha256"),
      "enabled" => true,
      "priority" => 100
    },
    {
      "id" => LIBCHEWING_SOURCE_ID,
      "name" => LIBCHEWING_SOURCE_NAME,
      "url" => "https://github.com/chewing/libchewing-data/releases/tag/v2026.3.22",
      "format" => "csv",
      "license" => "LGPL-2.1-or-later",
      "attribution" => "libchewing Core Team",
      "sha256" => libchewing_source_inventory_info.fetch("sha256"),
      "enabled" => true,
      "priority" => 250
    },
    {
      "id" => RIME_ESSAY_SOURCE_ID,
      "name" => RIME_ESSAY_SOURCE_NAME,
      "url" => "https://github.com/rime/rime-essay/tree/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed",
      "format" => "text",
      "license" => "LGPL-3.0",
      "attribution" => "Rime essay contributors",
      "sha256" => rime_essay_source_inventory_info.fetch("sha256"),
      "enabled" => true,
      "priority" => 220
    },
    {
      "id" => OVERLAY_SOURCE_ID,
      "name" => OVERLAY_SOURCE_NAME,
      "url" => "https://github.com/akira02/Chiaki-KeyKey-Lexicon/blob/main/sources/chiaki-modern-overlay/phrases.tsv",
      "format" => "tsv",
      "license" => "CC0-1.0",
      "attribution" => "Chiaki KeyKey Lexicon maintainers",
      "sha256" => overlay_info.fetch("sha256"),
      "enabled" => true,
      "priority" => 300
    }
  ],
  "artifacts" => [
    {
      "id" => "smart-mandarin-db",
      "kind" => "keykey-source-db",
      "url" => "#{RELEASE_BASE_URL}/#{DB_FILENAME}",
      "filename" => DB_FILENAME,
      "sha256" => db_info.fetch("sha256"),
      "size" => db_info.fetch("size"),
      "database_schema_version" => DATABASE_SCHEMA_VERSION,
      "language_model_version" => LANGUAGE_MODEL_VERSION
    },
    {
      "id" => "smart-mandarin-metadata",
      "kind" => "metadata",
      "url" => "#{RELEASE_BASE_URL}/#{METADATA_FILENAME}",
      "filename" => METADATA_FILENAME,
      "sha256" => metadata_info.fetch("sha256"),
      "size" => metadata_info.fetch("size"),
      "database_schema_version" => DATABASE_SCHEMA_VERSION,
      "language_model_version" => LANGUAGE_MODEL_VERSION
    },
    {
      "id" => "smart-mandarin-checksums",
      "kind" => "checksum",
      "url" => "#{RELEASE_BASE_URL}/#{CHECKSUM_FILENAME}",
      "filename" => CHECKSUM_FILENAME,
      "sha256" => checksum_info.fetch("sha256"),
      "size" => checksum_info.fetch("size"),
      "database_schema_version" => DATABASE_SCHEMA_VERSION,
      "language_model_version" => LANGUAGE_MODEL_VERSION
    }
  ]
}

write_json(MANIFEST_PATH, manifest)
FileUtils.cp(MANIFEST_PATH, DIST_MANIFEST_PATH)

puts "Prepared Chiaki KeyKey Lexicon #{RELEASE_VERSION}"
puts "  DB: #{DB_PATH}"
puts "  Metadata: #{METADATA_PATH}"
puts "  Manifest: #{MANIFEST_PATH}"
puts "  Checksums: #{CHECKSUM_PATH}"
puts "  Normalized TSV: #{NORMALIZED_PATH} (#{counts.fetch("normalized_rows")} rows)"
puts "  Imported:"
import_results.each do |result|
  puts "    #{result.source_path}: seen=#{result.seen} added=#{result.added} skipped=#{result.skipped}"
end
