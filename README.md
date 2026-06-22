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

This repository now has a first seed release pipeline. The latest seed release is `2026.06.2`.

The first release is intentionally conservative: it packages the known-working KeyKey Boneyard bootstrap database shape, exports a normalized TSV, records source checksums, and writes release-ready artifacts under `dist/`.

Start with:

- [Docs/ImplementationGuide.md](Docs/ImplementationGuide.md)
- [Docs/SourceReview.md](Docs/SourceReview.md)

Build the local release package with:

```sh
Scripts/prepare-v1-release.rb
```

## Proposed Layout

```text
Docs/
  ImplementationGuide.md
LICENSES/
  README.md
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
KeyKeySource-YYYY.MM.db
KeyKeySource-YYYY.MM.json
lexicon-manifest.json
SHA256SUMS
```

The main app should download and verify `lexicon-manifest.json`, then install a compatible `KeyKeySource` database into:

```text
~/Library/Application Support/Chiaki KeyKey/Lexicons/
```

Runtime database loading should fall back to the bundled database if the active external database is missing, invalid, or incompatible.

## License Policy

There is no repository-wide data license yet.

Every source must declare its own license before it can be used in a public release. Unknown-license data may be used only for local experiments and must not be included in release artifacts.
