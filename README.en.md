# ChiaKey Lexicon

[中文](README.md)

ChiaKey Lexicon is the data-side repository for 千秋輸入法 (ChiaKey).

The main app repository should stay focused on the macOS input method runtime, database reader, builder scripts, installation tooling, and a small bundled fallback database. This repository owns the evolving lexicon data, source manifests, license notes, release database artifacts, checksums, and changelog.

## Intended Split

`ChiaKey` owns:

1. macOS IMK runtime.
2. Input engine integration.
3. Database schema and reader.
4. Builder or installation scripts that can consume this repo's release artifacts.
5. A bundled fallback `KeyKeySource.db`.

`ChiaKey-Lexicon` owns:

1. Source manifests.
2. Source license and attribution records.
3. Vendored raw lexicon sources.
4. Release-ready `KeyKeySource` database artifacts.
5. Checksums or signatures.
6. Lexicon release changelog.

## Current Status

This repository has an active seed release pipeline. Pushes to `main` build and publish versioned lexicon releases through GitHub Actions.

The current pipeline builds a complete `KeyKeySource.db` from reviewed source data, project-owned corrections, generated metadata, source inventories, and checksum manifests. Local verification builds default to `dist/dev/`; public release versions are computed by CI, injected into the builder, and uploaded to GitHub Releases.

Start with:

- [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)
- [Docs/SourceReview.md](Docs/SourceReview.md)

Build a local verification package with:

```sh
cargo run --release -- prepare-release
```

Public releases do not require manually updating a version in the repository. GitHub Actions computes the next `YYYY.MM.N` from existing tags.

## Architecture

The repository is organized around a reproducible data pipeline:

1. `sources/<source-id>/` holds each reviewed input source, its local README, and a `source-inventory.sha256` provenance file.
2. `LICENSES/` records the license text or license notes needed for every source that can ship in a public release.
3. `src/` contains the Rust release toolchain. It verifies inputs, imports data layers into the KeyKey database shape, writes generated audit artifacts, updates release metadata, and generates manifests.
4. `normalized/smart-mandarin.tsv` is the generated normalized audit view of the Smart Mandarin language-model rows and is not committed.
5. `manifests/lexicon-manifest.json` is the generated update contract consumed by the app and is not committed; release builds copy it into `dist/`.
6. `dist/dev/` or `dist/<version>/` is local staging for release artifacts and is not committed.

The data layers fall into four broad groups:

1. **Runtime compatibility data**: reviewed KeyKey-origin data needed by the app's existing database reader and input modules.
2. **Lexicon sources**: modern Traditional Chinese / Zhuyin vocabulary and supplemental character or phrase coverage.
3. **Project-owned corrections**: small overlays for known typing misses, explicit reading fixes, and candidate ordering adjustments.
4. **Policy layers**: small reviewed rules that keep the default Traditional Chinese release aligned with the app's language and region expectations.

Detailed source-by-source license and redistribution decisions live in [Docs/SourceReview.md](Docs/SourceReview.md). Day-to-day release mechanics live in [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md).

## Repository Layout

```text
Docs/
  ReleaseFlow.zh-TW.md
  SourceReview.md
LICENSES/
  README.md
src/
  main.rs
manifests/
  lexicon-manifest.example.json
normalized/
  .gitkeep
schemas/
  lexicon-manifest.schema.json
sources/
  .gitkeep
```

Built release artifacts are not tracked in git. Use a local staging directory such as `dist/`, then upload the artifacts to GitHub Releases.

Maintainers can update pinned external sources with:

```sh
cargo run --release -- fetch-modern-sources
```

That command refreshes vendored raw source snapshots and source inventories.
Normal CI release builds do not need network access to fetch source data.

## Release Shape

A GitHub Release should publish:

```text
KeyKeySource-YYYY.MM.N.db
KeyKeySource-YYYY.MM.N.json
lexicon-manifest.json
SHA256SUMS
```

The main app should download and verify `lexicon-manifest.json`, then install a compatible `KeyKeySource` database into:

```text
~/Library/Application Support/ChiaKey/Lexicons/
```

Runtime database loading should fall back to the bundled database if the active external database is missing, invalid, or incompatible.

## License Policy

Rust release tooling and repository scripts are licensed under the MIT License; see [LICENSE-CODE](LICENSE-CODE).

There is no repository-wide data license yet.

Every source must declare its own license before it can be used in a public release. Unknown-license data may be used only for local experiments and must not be included in release artifacts.
