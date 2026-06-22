# Implementation Guide

This document is written for the next engineer or maintainer implementing Chiaki KeyKey's lexicon update pipeline.

## Objective

Build a maintainable lexicon repository that can produce versioned `KeyKeySource.db` artifacts for Chiaki KeyKey.

The target behavior is:

1. Lexicon data is maintained outside the main input method repository.
2. Every source has explicit license and attribution metadata.
3. Raw source data is converted into a stable normalized TSV format.
4. The existing Chiaki KeyKey database builder can consume normalized data.
5. GitHub Releases publish signed or checksummed database artifacts.
6. The main app can safely download, verify, install, and fall back from lexicon updates.

## Related Repository

Main app repository:

```text
https://github.com/akira02/Chiaki-KeyKey
```

Important current files in the main repo:

```text
Docs/ModernizationPlan.md
Scripts/build-dev-smart-mandarin-db.rb
YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Modern/ChiakiModernPhrases.txt
YahooKeyKey-Source-1.1.2528/Frameworks/Manjusri
YahooKeyKey-Source-1.1.2528/ModulePackages/OVIMMandarin
```

## Non-goals

Do not directly import large third-party dictionaries without license review.

Do not put unknown-license personal corpora into a public release.

Do not require the input method process to perform network access during key handling.

Do not make runtime lexicon updates overwrite the bundled fallback database.

## Phase 1: Repository Structure

Keep this repo data-first and release-oriented.

Recommended layout:

```text
Docs/
  ImplementationGuide.md
  SourceReview.md
LICENSES/
  README.md
manifests/
  lexicon-manifest.example.json
  lexicon-manifest.json
normalized/
  smart-mandarin.tsv
schemas/
  lexicon-manifest.schema.json
sources/
  <source-id>/
    README.md
    raw/
    normalized.tsv
```

`raw/` may be omitted or gitignored when raw files are large, generated, or license-sensitive. In that case, document how to fetch and verify them.

Built release artifacts must not be committed to git. Use a local staging directory such as `dist/`, then upload `KeyKeySource` databases, metadata JSON, manifests, and checksum files to GitHub Releases.

## Phase 2: Manifest Contract

The manifest is the public contract between this repo and the main app.

The manifest must include:

1. `schema`
2. `version`
3. `generated_at`
4. `minimum_app_version`
5. `database_schema_version`
6. `sources`
7. `artifacts`

Each source entry must include:

1. `id`
2. `name`
3. `url`
4. `format`
5. `license`
6. `attribution`
7. `sha256`
8. `enabled`
9. `priority`

Each artifact entry must include:

1. `id`
2. `kind`
3. `url`
4. `filename`
5. `sha256`
6. `size`
7. `database_schema_version`
8. `language_model_version`

Use `schemas/lexicon-manifest.schema.json` as the starting validation schema.

## Phase 3: Normalized Format

Use TSV as the first normalized interchange format:

```text
reading<TAB>phrase<TAB>weight<TAB>source_id<TAB>tags
```

Rules:

1. `reading` is normalized Bopomofo reading, using the same internal spelling expected by the Chiaki KeyKey builder.
2. `phrase` is Traditional Chinese output unless the source is explicitly tagged otherwise.
3. `weight` is numeric and comparable across merged sources.
4. `source_id` matches the manifest source id.
5. `tags` is comma-separated and optional.

Validation rules:

1. Reject empty readings.
2. Reject empty phrases.
3. Reject rows without a source id.
4. Reject rows whose source id is not present in the manifest.
5. Warn on Simplified Chinese in the Traditional Mandarin default path.
6. Warn on extremely long phrases unless explicitly tagged.

## Phase 4: Builder Integration

Refactor the main repo builder from:

```text
Scripts/build-dev-smart-mandarin-db.rb
```

into a reusable script, likely:

```text
Scripts/build-smart-mandarin-db.rb
```

The script should accept:

```text
--manifest /path/to/lexicon-manifest.json
--normalized /path/to/smart-mandarin.tsv
--output /path/to/KeyKeySource-YYYY.MM.db
--metadata-output /path/to/KeyKeySource-YYYY.MM.json
```

The metadata JSON should include:

1. database schema version
2. lexicon version
3. source ids and checksums
4. builder git commit
5. build timestamp
6. unigram count
7. bigram count
8. candidate row count

The output DB should remain compatible with the existing Manjusri/OVIMMandarin reader until a later schema migration is planned.

## Phase 5: GitHub Release Workflow

A release should be reproducible.

Recommended workflow:

1. Validate manifest.
2. Fetch enabled sources.
3. Verify checksums.
4. Normalize sources.
5. Validate normalized TSV.
6. Build DB.
7. Generate metadata JSON.
8. Generate `SHA256SUMS`.
9. Create a GitHub Release.
10. Upload DB, metadata, manifest, and checksum files.

GitHub Release assets:

```text
KeyKeySource-YYYY.MM.db
KeyKeySource-YYYY.MM.json
lexicon-manifest.json
SHA256SUMS
```

Do not commit these files to the repo. Do not rely on mutable branch URLs for production updates. Prefer immutable GitHub Release asset URLs.

## Phase 6: Main App Integration

The main app should search databases in this order:

```text
~/Library/Application Support/Chiaki KeyKey/Lexicons/active/KeyKeySource.db
<app bundle>/Contents/Resources/DataSource/KeyKeySource.db
```

The update installer should write versioned directories:

```text
~/Library/Application Support/Chiaki KeyKey/Lexicons/
  versions/
    YYYY.MM/
      KeyKeySource.db
      metadata.json
  active -> versions/YYYY.MM
```

Install steps:

1. Download manifest.
2. Check manifest schema.
3. Check app compatibility.
4. Download artifact.
5. Verify checksum.
6. Validate DB can be opened and contains required tables.
7. Install into `versions/YYYY.MM`.
8. Atomically switch `active`.
9. Keep previous version for rollback.

The input method runtime should not fetch remote data while handling key events.

## Suggested Acceptance Criteria

The implementation is ready when:

1. `lexicon-manifest.json` validates against the schema.
2. Unknown-license sources are excluded from public artifacts.
3. A normalized TSV can be generated from at least one source.
4. A `KeyKeySource.db` can be built from normalized TSV.
5. The DB metadata reports row counts and source checksums.
6. `SHA256SUMS` verifies all release assets.
7. The main app can load an external DB from Application Support.
8. The main app falls back to the bundled DB when the external DB is invalid.
9. User-specific learning data is not overwritten by lexicon updates.

## First Concrete Task List

1. Create `lexicon-manifest.json` from the example manifest.
2. Decide the first public source with known license.
3. Add `Docs/SourceReview.md` summarizing source license decisions.
4. Define the exact Bopomofo normalized reading syntax used by the builder.
5. Refactor the main repo builder to accept external normalized TSV.
6. Build a first dev DB from this repo's normalized data.
7. Publish a draft GitHub Release with DB, metadata, manifest, and checksums.
8. Update the main repo to support Application Support active DB fallback.

## Notes On vChewing

vChewing is useful as an engineering reference, especially for modern macOS IME packaging and compatibility strategies.

Do not import vChewing lexicon data unless its source data license and attribution chain are explicitly reviewed and compatible with Chiaki KeyKey's distribution plan.
