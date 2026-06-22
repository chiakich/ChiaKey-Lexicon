#!/usr/bin/env ruby
# frozen_string_literal: true

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
OVERLAY_SOURCE_ID = "chiaki-modern-overlay"
OVERLAY_SOURCE_NAME = "Chiaki modern overlay phrases"
RELEASE_VERSION = ENV.fetch("LEXICON_VERSION", "2026.06.2")
LANGUAGE_MODEL_VERSION = "chiaki-modern-#{RELEASE_VERSION}"
MINIMUM_APP_VERSION = ENV.fetch("MINIMUM_APP_VERSION", "0.1.0")
DATABASE_SCHEMA_VERSION = 1
GENERATED_AT = Time.now.utc.iso8601
RELEASE_BASE_URL = ENV.fetch(
  "RELEASE_BASE_URL",
  "https://github.com/akira02/Chiaki-KeyKey-Lexicon/releases/download/#{RELEASE_VERSION}"
)

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
OVERLAY_SOURCE_DIR = File.join(ROOT, "sources", OVERLAY_SOURCE_ID)
OVERLAY_PHRASES_PATH = File.join(OVERLAY_SOURCE_DIR, "phrases.tsv")
SOURCE_INVENTORY_PATH = File.join(BONEYARD_SOURCE_DIR, "source-inventory.sha256")
MANIFEST_PATH = File.join(ROOT, "manifests", "lexicon-manifest.json")
DIST_MANIFEST_PATH = File.join(DIST_DIR, "lexicon-manifest.json")
DB_FILENAME = "KeyKeySource-#{RELEASE_VERSION}.db"
METADATA_FILENAME = "KeyKeySource-#{RELEASE_VERSION}.json"
CHECKSUM_FILENAME = "SHA256SUMS"
DB_PATH = File.join(DIST_DIR, DB_FILENAME)
METADATA_PATH = File.join(DIST_DIR, METADATA_FILENAME)
CHECKSUM_PATH = File.join(DIST_DIR, CHECKSUM_FILENAME)

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

OverlayRecord = Struct.new(:phrase, :weight, :tags, :qstring, keyword_init: true)

def parse_overlay(path)
  return [] unless File.file?(path)

  records = []
  File.foreach(path, chomp: true).with_index(1) do |line, line_number|
    next if line.strip.empty? || line.start_with?("#")

    phrase, weight, tags = line.split("\t", 3)
    if phrase.to_s.empty? || weight.to_s.empty?
      warn "Invalid overlay row #{path}:#{line_number}"
      exit 1
    end

    Float(weight)
    records << OverlayRecord.new(phrase: phrase, weight: weight, tags: tags.to_s)
  rescue ArgumentError
    warn "Invalid overlay weight #{path}:#{line_number}: #{weight.inspect}"
    exit 1
  end

  records
end

def infer_qstring_for_phrase(db_path, phrase)
  qstrings = []

  phrase.each_char do |char|
    rows = run_sqlite_json(
      db_path,
      "SELECT qstring FROM unigrams WHERE current = #{sql(char)} ORDER BY probability DESC, qstring LIMIT 1;"
    )
    return nil if rows.empty?

    qstrings << rows.first.fetch("qstring")
  end

  qstrings.join
end

def apply_overlay!(db_path, overlay_path)
  records = parse_overlay(overlay_path)
  return { records: [], seen: 0, added: 0, skipped: 0 } if records.empty?

  sql_lines = ["BEGIN;"]
  added = 0
  skipped = 0

  records.each do |record|
    qstring = infer_qstring_for_phrase(db_path, record.phrase)
    unless qstring
      skipped += 1
      next
    end

    record.qstring = qstring
    sql_lines << "DELETE FROM unigrams WHERE qstring = #{sql(qstring)} AND current = #{sql(record.phrase)};"
    sql_lines << "INSERT INTO unigrams VALUES(#{sql(qstring)}, #{sql(record.phrase)}, #{record.weight}, 0.0);"
    sql_lines << "INSERT INTO 'Mandarin-bpmf-cin' SELECT #{sql(qstring)}, #{sql(record.phrase)} WHERE NOT EXISTS (SELECT 1 FROM 'Mandarin-bpmf-cin' WHERE key = #{sql(qstring)} AND value = #{sql(record.phrase)});"
    added += 1
  end

  inventory_path = "sources/#{OVERLAY_SOURCE_ID}/phrases.tsv"
  inventory_hash = sha256(overlay_path)
  sql_lines << "DELETE FROM chiaki_db_sources WHERE source = #{sql(inventory_path)};"
  sql_lines << "INSERT INTO chiaki_db_sources VALUES(#{sql(inventory_path)}, 'overlay', #{sql(inventory_hash)}, #{records.length}, #{added}, #{skipped});"
  sql_lines << "DELETE FROM chiaki_db_metadata WHERE key IN ('unigram_count', 'candidate_count');"
  sql_lines << "INSERT INTO chiaki_db_metadata VALUES('unigram_count', (SELECT COUNT(*) FROM unigrams));"
  sql_lines << "INSERT INTO chiaki_db_metadata VALUES('candidate_count', (SELECT COUNT(*) FROM 'Mandarin-bpmf-cin' WHERE key NOT LIKE '__property_%'));"
  sql_lines << "COMMIT;"

  run_sqlite(db_path, sql_lines.join("\n"))
  { records: records.select(&:qstring), seen: records.length, added: added, skipped: skipped }
end

def write_json(path, data)
  File.write(path, "#{JSON.pretty_generate(data)}\n")
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
  exit 1
end

verify_required_files!([BONEYARD_DB, *source_files])

FileUtils.mkdir_p(DIST_DIR)
FileUtils.mkdir_p(File.dirname(NORMALIZED_PATH))
FileUtils.mkdir_p(BONEYARD_SOURCE_DIR)
FileUtils.mkdir_p(OVERLAY_SOURCE_DIR)

inventory_lines = source_files.map do |path|
  "#{sha256(path)}  #{relative_to(path, BONEYARD_ROOT)}"
end
File.write(SOURCE_INVENTORY_PATH, "#{inventory_lines.join("\n")}\n")
source_inventory_info = file_info(SOURCE_INVENTORY_PATH)
overlay_info = file_info(OVERLAY_PHRASES_PATH)

FileUtils.cp(BONEYARD_DB, DB_PATH)
overlay_result = apply_overlay!(DB_PATH, OVERLAY_PHRASES_PATH)
overlay_keys = overlay_result.fetch(:records).each_with_object({}) do |record, hash|
  hash[[record.qstring, record.phrase]] = record
end

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
    overlay_record = overlay_keys[[row.fetch("reading"), phrase]]
    source_id = overlay_record ? OVERLAY_SOURCE_ID : BONEYARD_SOURCE_ID
    tags = overlay_record ? "unigram,#{overlay_record.tags}" : "unigram,keykey-boneyard"

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
        "sha256" => source_inventory_info.fetch("sha256"),
        "size" => source_inventory_info.fetch("size")
      },
      "stats" => source_rows
    },
    {
      "id" => OVERLAY_SOURCE_ID,
      "name" => OVERLAY_SOURCE_NAME,
      "license" => "CC0-1.0",
      "attribution" => "Chiaki KeyKey Lexicon maintainers",
      "path" => "sources/#{OVERLAY_SOURCE_ID}/phrases.tsv",
      "sha256" => overlay_info.fetch("sha256"),
      "size" => overlay_info.fetch("size"),
      "stats" => {
        "seen" => overlay_result.fetch(:seen),
        "added" => overlay_result.fetch(:added),
        "skipped" => overlay_result.fetch(:skipped)
      }
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
      "sha256" => source_inventory_info.fetch("sha256"),
      "enabled" => true,
      "priority" => 100
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
      "priority" => 200
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
