#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "fileutils"
require "json"
require "open3"
require "pathname"
require "time"

ROOT = File.expand_path("..", __dir__)
SOURCE_ID = "keykey-boneyard-bootstrap"
SOURCE_NAME = "KeyKey Boneyard bootstrap data"
RELEASE_VERSION = ENV.fetch("LEXICON_VERSION", "2026.06.1")
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
SOURCE_DIR = File.join(ROOT, "sources", SOURCE_ID)
SOURCE_INVENTORY_PATH = File.join(SOURCE_DIR, "source-inventory.sha256")
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
FileUtils.mkdir_p(SOURCE_DIR)

inventory_lines = source_files.map do |path|
  "#{sha256(path)}  #{relative_to(path, BONEYARD_ROOT)}"
end
File.write(SOURCE_INVENTORY_PATH, "#{inventory_lines.join("\n")}\n")
source_inventory_info = file_info(SOURCE_INVENTORY_PATH)

FileUtils.cp(BONEYARD_DB, DB_PATH)

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

    file.puts [
      row.fetch("reading"),
      phrase,
      row.fetch("weight"),
      SOURCE_ID,
      "unigram,keykey-boneyard"
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
      "id" => SOURCE_ID,
      "name" => SOURCE_NAME,
      "license" => "BSD-3-Clause-style",
      "attribution" => "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / Chiaki KeyKey maintainers",
      "inventory" => {
        "path" => "sources/#{SOURCE_ID}/source-inventory.sha256",
        "sha256" => source_inventory_info.fetch("sha256"),
        "size" => source_inventory_info.fetch("size")
      },
      "stats" => source_rows
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
      "id" => SOURCE_ID,
      "name" => SOURCE_NAME,
      "url" => "https://github.com/vChewing/KeyKey-Boneyard",
      "format" => "sqlite",
      "license" => "BSD-3-Clause-style",
      "attribution" => "Yahoo! Inc.; OpenVanilla contributors; KeyKey Boneyard / Chiaki KeyKey maintainers",
      "sha256" => source_inventory_info.fetch("sha256"),
      "enabled" => true,
      "priority" => 100
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
