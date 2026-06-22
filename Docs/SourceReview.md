# Source Review

Last reviewed: 2026-06-22

## Included in v1

### keykey-boneyard-bootstrap

- Name: KeyKey Boneyard bootstrap data
- Local release input: `sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db`
- Upstream archive: <https://github.com/vChewing/KeyKey-Boneyard>
- Current fork note: <https://github.com/akira02/Chiaki-KeyKey>
- License: BSD-3-Clause-style Yahoo! KeyKey upstream license
- Attribution: Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / Chiaki KeyKey maintainers
- Redistribution decision: included for the first public seed release

This repository vendors only the cooked bootstrap database needed by the release builder:

```text
sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db
sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db.sha256
```

The source files used to produce that bootstrap database are limited to the redistributable KeyKey Boneyard inputs:

- `YahooKeyKey-Source-1.1.2528/DataTables/bpmf.cin`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Addendum/*.txt`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Overrides/*.txt`
- `YahooKeyKey-Source-1.1.2528/Distributions/Takao/DataSource/Modern/*.txt`

The generated source inventory is stored at:

```text
sources/keykey-boneyard-bootstrap/source-inventory.sha256
```

The manifest records the SHA-256 of that inventory file, not a single raw upstream archive.

`source-inventory.sha256` is kept as provenance for the vendored cooked database. The full KeyKey Boneyard tree is not copied into this repository.

## Excluded from v1

These sources are useful references, but they are not included as raw sources in the first release artifacts:

- Yahoo search terms from the historical data package.
- Sinica Corpus raw material.
- Commercial CEROD / SQLite extension assets.
- CC-CEDICT, moedict, Wikimedia, Tatoeba, wordfreq, SUBTLEX-CH, Google Books Ngram, and Google Chinese Web 5-gram.

Some bootstrap files inherited from the open KeyKey Boneyard tree have legacy names such as `Yahoo.txt` or `SinicaCorpusOverrides.txt`. In v1, these are treated as part of the BSD-style Boneyard bootstrap source. The repository does not copy private raw Yahoo search logs, Sinica corpus files, or CEROD binaries.

## Included Starting in 2026.06.2

### chiaki-modern-overlay

- Name: Chiaki modern overlay phrases
- Local source: `sources/chiaki-modern-overlay/phrases.tsv`
- License: CC0-1.0
- Attribution: Chiaki KeyKey Lexicon maintainers
- Redistribution decision: included for public releases

This source is intentionally small and project-owned. It is used for obvious seed lexicon misses discovered during hands-on testing, such as basic input-method phrases that should not depend on a future large frequency corpus.

## Included Starting in 2026.06.3

### libchewing-data

- Name: libchewing-data Traditional Chinese Zhuyin dictionary
- Local source: `sources/libchewing-data/raw/`
- Upstream release: <https://github.com/chewing/libchewing-data/releases/tag/v2026.3.22>
- Current upstream home: <https://codeberg.org/chewing/libchewing-data>
- License: LGPL-2.1-or-later
- Attribution: libchewing Core Team
- Redistribution decision: included for public releases starting in `2026.06.3`

The release builder imports these pinned files:

- `dict/chewing/tsi.csv`
- `dict/chewing/alt.csv`
- `dict/chewing/word.csv`

`tsi.csv` and `alt.csv` are imported as the main modern phrase layer because they include explicit Zhuyin readings. For phrases present in libchewing-data, the builder replaces older inferred phrase readings from the bootstrap database with libchewing's explicit readings. `word.csv` is used only to add missing single-character readings.

Starting in `2026.06.5`, single-character rows from `tsi.csv` are also imported as a character-frequency correction layer. This lets common characters such as `我` keep their libchewing frequency instead of tying with rare same-reading characters from the bootstrap database.

The raw files are fetched by:

```text
cargo run --release -- fetch-modern-sources
```

The generated source inventory is stored at:

```text
sources/libchewing-data/source-inventory.sha256
```

### rime-essay

- Name: Rime essay shared vocabulary and language model
- Local source: `sources/rime-essay/raw/essay.txt`
- Upstream commit: <https://github.com/rime/rime-essay/tree/48c7538f0b760fcc8c9d6bf08711f82cfbd2e9ed>
- License: LGPL-3.0
- Attribution: Rime essay contributors
- Redistribution decision: included for public releases starting in `2026.06.3`

Rime essay is imported as a low-priority supplemental phrase layer. It has useful modern vocabulary and scores, but it does not include Zhuyin readings. The release builder therefore imports only entries that satisfy all of these constraints:

1. The phrase is not already present after the libchewing-data import.
2. The phrase length is between 2 and 7 Unicode codepoints.
3. The Rime score is at least `40`.
4. Every character has a primary single-character reading in the current database.

This avoids replacing libchewing's explicit Zhuyin data with inferred readings, while still adding modern terms such as social, news, and technology vocabulary when the reading can be inferred safely enough for a supplemental layer.

The generated source inventory is stored at:

```text
sources/rime-essay/source-inventory.sha256
```

## Reading Format

The v1 normalized TSV uses the current KeyKey / Manjusri internal `qstring` reading representation in the first column. This is the two-byte-per-syllable ordering string produced by the legacy builder's `absolute_order_string` function, not literal Bopomofo text.

This keeps the first release directly compatible with the current database reader. A later source-normalization pass can add a human-readable Bopomofo column if the builder contract changes.

## Current Risk Notes

This release is still a seed lexicon, but it now includes a substantially larger modern Traditional Chinese / Zhuyin layer.

Expected follow-up work:

1. Add Taiwan-specific modern phrases based on actual misses.
2. Tune the cross-source weight mapping after real typing tests.
3. Review LGPL redistribution requirements whenever release packaging changes.
4. Review CC BY-SA and research-only sources before any future public release includes them.
