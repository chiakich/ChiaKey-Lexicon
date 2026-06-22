# Source Review

Last reviewed: 2026-06-22

## Included in v1

### keykey-boneyard-bootstrap

- Name: KeyKey Boneyard bootstrap data
- Local source: `../KeyKey-Boneyard/YahooKeyKey-Source-1.1.2528`
- Upstream archive: <https://github.com/vChewing/KeyKey-Boneyard>
- Current fork note: <https://github.com/akira02/Chiaki-KeyKey>
- License: BSD-3-Clause-style Yahoo! KeyKey upstream license
- Attribution: Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / Chiaki KeyKey maintainers
- Redistribution decision: included for the first public seed release

The source files used for v1 are limited to the redistributable bootstrap database inputs:

- `YahooKeyKey-Source-1.1.2528/DataTables/bpmf.cin`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Addendum/*.txt`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Overrides/*.txt`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Modern/*.txt`

The generated source inventory is stored at:

```text
sources/keykey-boneyard-bootstrap/source-inventory.sha256
```

The manifest records the SHA-256 of that inventory file, not a single raw upstream archive.

## Excluded from v1

These sources are useful references, but they are not included as raw sources in the first release artifacts:

- Yahoo search terms from the historical data package.
- Sinica Corpus raw material.
- Commercial CEROD / SQLite extension assets.
- CC-CEDICT, moedict, Wikimedia, Tatoeba, wordfreq, SUBTLEX-CH, Google Books Ngram, and Google Chinese Web 5-gram.

Some bootstrap files inherited from the open KeyKey Boneyard tree have legacy names such as `Yahoo.txt` or `SinicaCorpusOverrides.txt`. In v1, these are treated as part of the BSD-style Boneyard bootstrap source. The repository does not copy private raw Yahoo search logs, Sinica corpus files, or CEROD binaries.

## Reading Format

The v1 normalized TSV uses the current KeyKey / Manjusri internal `qstring` reading representation in the first column. This is the two-byte-per-syllable ordering string produced by the legacy builder's `absolute_order_string` function, not literal Bopomofo text.

This keeps the first release directly compatible with the current database reader. A later source-normalization pass can add a human-readable Bopomofo column if the builder contract changes.

## v1 Risk Notes

This release is intentionally a seed lexicon. It keeps the known-working KeyKey data shape and avoids adding external frequency corpora before the user has tried the input method in real typing.

Expected follow-up work:

1. Add Taiwan-specific modern phrases based on actual misses.
2. Add a clear frequency normalization strategy before importing larger public corpora.
3. Review CC BY-SA and research-only sources before any future public release includes them.
4. Split source attribution more granularly if the source mix grows beyond the bootstrap set.
