#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
require "fileutils"
require "net/http"
require "pathname"
require "uri"

ROOT = File.expand_path("..", __dir__)

SourceFile = Struct.new(:url, :path, :sha256, keyword_init: true)

FILES = [
  SourceFile.new(
    url: "https://raw.githubusercontent.com/chewing/libchewing-data/v2026.3.22/dict/chewing/tsi.csv",
    path: "sources/libchewing-data/raw/dict/chewing/tsi.csv",
    sha256: "c889a1ac3ae1901b3f8f62748bc41b958f010bf995f7f88dbaf9e3494f341428"
  ),
  SourceFile.new(
    url: "https://raw.githubusercontent.com/chewing/libchewing-data/v2026.3.22/dict/chewing/word.csv",
    path: "sources/libchewing-data/raw/dict/chewing/word.csv",
    sha256: "da55b8e599c1389bc486453554f3410cf9c621d0ffff0ce38855698d26b3892a"
  ),
  SourceFile.new(
    url: "https://raw.githubusercontent.com/chewing/libchewing-data/v2026.3.22/dict/chewing/alt.csv",
    path: "sources/libchewing-data/raw/dict/chewing/alt.csv",
    sha256: "66df78f53ff18ab97bc39b3f3108a1f6d8d5be3237d9e72ff9f6f7186b4d6b2e"
  ),
  SourceFile.new(
    url: "https://raw.githubusercontent.com/chewing/libchewing/v0.12.0/COPYING",
    path: "LICENSES/libchewing-data-LGPL-2.1-or-later.txt",
    sha256: "dc626520dcd53a22f727af3ee42c770e56c97a64fe3adb063799d8ab032fe551"
  ),
  SourceFile.new(
    url: "https://raw.githubusercontent.com/rime/rime-essay/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed/essay.txt",
    path: "sources/rime-essay/raw/essay.txt",
    sha256: "09086a44204f469d2c16ad72784e1f567a6f016570dfc9aa79f868267a9c1385"
  ),
  SourceFile.new(
    url: "https://raw.githubusercontent.com/rime/rime-essay/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed/LICENSE",
    path: "LICENSES/rime-essay-LGPL-3.0.txt",
    sha256: "da7eabb7bafdf7d3ae5e9f223aa5bdc1eece45ac569dc21b3b037520b4464768"
  )
].freeze

def fetch(url)
  uri = URI(url)
  response = Net::HTTP.start(uri.host, uri.port, use_ssl: uri.scheme == "https") do |http|
    request = Net::HTTP::Get.new(uri)
    request["User-Agent"] = "chiaki-keykey-lexicon-fetcher"
    http.request(request)
  end

  case response
  when Net::HTTPSuccess
    response.body
  when Net::HTTPRedirection
    fetch(response.fetch("location"))
  else
    warn "Failed to fetch #{url}: #{response.code} #{response.message}"
    exit 1
  end
end

def sha256_bytes(bytes)
  Digest::SHA256.hexdigest(bytes)
end

def write_inventory(source_id)
  root = File.join(ROOT, "sources", source_id)
  raw_root = File.join(root, "raw")
  inventory_path = File.join(root, "source-inventory.sha256")
  lines = Dir[File.join(raw_root, "**", "*")]
          .select { |path| File.file?(path) }
          .sort
          .map do |path|
            relative = Pathname.new(path).relative_path_from(Pathname.new(root)).to_s
            "#{Digest::SHA256.file(path).hexdigest}  #{relative}"
          end

  File.write(inventory_path, "#{lines.join("\n")}\n")
end

FILES.each do |source_file|
  target = File.join(ROOT, source_file.path)
  FileUtils.mkdir_p(File.dirname(target))

  bytes = fetch(source_file.url)
  actual = sha256_bytes(bytes)
  unless actual == source_file.sha256
    warn "Checksum mismatch for #{source_file.url}"
    warn "  expected: #{source_file.sha256}"
    warn "  actual:   #{actual}"
    exit 1
  end

  File.binwrite(target, bytes)
  puts "fetched #{source_file.path}"
end

write_inventory("libchewing-data")
write_inventory("rime-essay")

puts "modern source fetch complete"
