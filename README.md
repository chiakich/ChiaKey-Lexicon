# Chiaki KeyKey Lexicon

Chiaki KeyKey Lexicon is the data-side repository for Chiaki KeyKey.

The main app repository should stay focused on the macOS input method runtime, database reader, builder scripts, installation tooling, and a small bundled fallback database. This repository owns the evolving lexicon data, source manifests, license notes, normalized intermediate data, release database artifacts, checksums, and changelog.

## Intended Split

`Chiaki-KeyKey` owns:

1. macOS IMK runtime.
2. Input engine integration.
3. Database schema and reader.
4. Builder scripts that can consume this repo's normalized data.
5. A bundled fallback `KeyKeySource.db`.

`Chiaki-KeyKey-Lexicon` owns:

1. Source manifests.
2. Source license and attribution records.
3. Normalized lexicon data.
4. Release-ready `KeyKeySource` database artifacts.
5. Checksums or signatures.
6. Lexicon release changelog.

## Current Status

This repository now has a seed release pipeline. The latest seed release is `2026.06.6`.

The current release packages the known-working KeyKey Boneyard database shape, restores the original KeyKey BPMF punctuation CIN rows, canned-message prepopulated service data, and module CIN tables needed by runtime punctuation, symbol, generic-input, and correction lookup, then layers in libchewing-data as the main Traditional Chinese / Zhuyin lexicon source, a public-domain extended BPMF character table for missing single-character readings, Rime essay as a low-priority supplemental phrase source, and a small Chiaki-owned overlay for hands-on input-method fixes.

Start with:

- [Docs/ImplementationGuide.md](Docs/ImplementationGuide.md)
- [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md)
- [Docs/SourceReview.md](Docs/SourceReview.md)

Fetch pinned external source files with:

```sh
cargo run --release -- fetch-modern-sources
```

Then build the local release package with:

```sh
cargo run --release -- prepare-release
```

## Proposed Layout

```text
Docs/
  ImplementationGuide.md
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
~/Library/Application Support/Chiaki KeyKey/Lexicons/
```

Runtime database loading should fall back to the bundled database if the active external database is missing, invalid, or incompatible.

## License Policy

Rust release tooling and repository scripts are licensed under the MIT License; see [LICENSE-CODE](LICENSE-CODE).

There is no repository-wide data license yet.

Every source must declare its own license before it can be used in a public release. Unknown-license data may be used only for local experiments and must not be included in release artifacts.
