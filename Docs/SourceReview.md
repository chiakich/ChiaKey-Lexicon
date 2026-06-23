# Source Review

Last reviewed: 2026-06-23

## Included in v1

### keykey-boneyard-bootstrap

- Name: KeyKey Boneyard bootstrap data
- Local release input: `sources/keykey-boneyard-bootstrap/vendor/KeyKeySource.db`
- Upstream archive: <https://github.com/vChewing/KeyKey-Boneyard>
- Current fork note: <https://github.com/akira02/ChiaKey>
- License: BSD-3-Clause-style Yahoo! KeyKey upstream license
- Attribution: Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / ChiaKey maintainers
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

## Included Starting in 2026.06.6

### keykey-punctuations-cin

- Name: KeyKey BPMF punctuation table
- Local source: `sources/keykey-punctuations-cin/vendor/bpmf-punctuations.cin`
- Upstream source file: <https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-punctuations.cin>
- License: BSD-3-Clause-style Yahoo! KeyKey upstream license
- Attribution: Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / ChiaKey maintainers
- Redistribution decision: included for public releases starting in `2026.06.6`

This source restores the original KeyKey runtime punctuation lookup rows. The release builder imports only rows inside `%chardef` whose keys start with `_punctuation_` or `_ctrl_`, and writes them to both `unigrams` and `Mandarin-bpmf-cin`.

These rows are required for Smart Mandarin punctuation handling, for example:

```text
_punctuation_<          ，
_punctuation_Standard_< ，
```

The generated source inventory is stored at:

```text
sources/keykey-punctuations-cin/source-inventory.sha256
```

### keykey-prepopulated-service-data

- Name: KeyKey prepopulated service data
- Local source: `sources/keykey-prepopulated-service-data/vendor/`
- Upstream source directory: <https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/Distributions/Takao/OnlineData>
- License: BSD-3-Clause-style Yahoo! KeyKey upstream license
- Attribution: Yahoo! Inc., OpenVanilla contributors, KeyKey Boneyard / ChiaKey maintainers
- Redistribution decision: included for public releases starting in `2026.06.6`

This source restores the original KeyKey prepopulated canned-message payload. The release builder writes the complete `CannedMessages.plist` contents to `prepopulated_service_data` as `canned_messages`, then writes a positive release timestamp as `canned_messages_timestamp`.

`OneKey.plist` is intentionally omitted from public releases. OneKey was a Yahoo-era URL launcher rather than input lexicon data, and modern ChiaKey no longer loads it. New release databases must not contain `onekey_services` or `onekey_services_timestamp`.

The generated source inventory is stored at:

```text
sources/keykey-prepopulated-service-data/source-inventory.sha256
```

### keykey-module-cin

- Name: KeyKey module CIN tables
- Local source: `sources/keykey-module-cin/vendor/`
- Upstream source directory: <https://github.com/vChewing/KeyKey-Boneyard/tree/master/YahooKeyKey-Source-1.1.2528/DataTables>
- License: BSD-3-Clause-style Yahoo! KeyKey upstream license / Public Domain source tables
- Attribution: Yahoo! Inc., OpenVanilla contributors, opendesktop.org.tw CIN contributors, KeyKey Boneyard / ChiaKey maintainers
- Redistribution decision: included for public releases starting in `2026.06.6`

This source restores module SQLite tables used by KeyKey runtime modules outside the Smart Mandarin language model:

```text
Generic-cj-cin
Generic-simplex-cin
Punctuations-cj-halfwidth-cin
Punctuations-cj-mixedwidth-cin
BopomofoCorrection-bopomofo-correction-cin
```

The generated source inventory is stored at:

```text
sources/keykey-module-cin/source-inventory.sha256
```

## Excluded from v1

These sources are useful references, but they are not included as raw sources in the first release artifacts:

- Yahoo search terms from the historical data package.
- Sinica Corpus raw material.
- Commercial CEROD / SQLite extension assets.
- CC-CEDICT, moedict, Wikimedia, Tatoeba, wordfreq, SUBTLEX-CH, Google Books Ngram, and Google Chinese Web 5-gram.

Some bootstrap files inherited from the open KeyKey Boneyard tree have historical names such as `Yahoo.txt` or `SinicaCorpusOverrides.txt`. In v1, these are treated as part of the BSD-style Boneyard bootstrap source. The repository does not copy private raw Yahoo search logs, Sinica corpus files, or CEROD binaries.

## Included Starting in 2026.06.2

### chiakey-modern-overlay

- Name: ChiaKey modern overlay phrases
- Local source:
  - `sources/chiakey-modern-overlay/phrases.tsv`
  - `sources/chiakey-modern-overlay/explicit.tsv`
- License: CC0-1.0
- Attribution: ChiaKey Lexicon maintainers
- Redistribution decision: included for public releases

This source is intentionally small and project-owned. It is used for obvious seed lexicon misses discovered during hands-on testing, such as basic input-method phrases that should not depend on a future large frequency corpus.

`phrases.tsv` lets the release builder infer readings from single-character data. `explicit.tsv` is used when a fix depends on a specific KeyKey qstring, such as promoting `個` for neutral-tone `ㄍㄜ˙` / `ek7`.

## Included Starting in 2026.06.7

### opencc-variant-policy

- Name: OpenCC-derived Traditional Chinese variant policy
- Local source: `sources/opencc-variant-policy/variant-demotions.tsv`
- Upstream reference: <https://github.com/BYVoid/OpenCC>
- License: Apache-2.0-derived policy
- Attribution: OpenCC contributors; ChiaKey Lexicon maintainers
- Redistribution decision: included for public releases starting in `2026.06.7`

This source is a small reviewed policy table derived from OpenCC's simplified/traditional conversion knowledge. It is not imported as a frequency dictionary. The release builder uses it only to lower Simplified or non-Taiwan-preferred variants when those variants otherwise tie with Traditional Chinese candidates.

The first row demotes `个`, the Simplified counterpart of `個`, so neutral-tone `ㄍㄜ˙` / `ek7` no longer sorts `个` ahead of `個` by tie-break alone.

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

### bpmf-ext-cin

- Name: Public domain extended BPMF character table
- Local source: `sources/bpmf-ext-cin/vendor/bpmf-ext.cin`
- Upstream source file: <https://github.com/vChewing/KeyKey-Boneyard/blob/master/YahooKeyKey-Source-1.1.2528/DataTables/bpmf-ext.cin>
- License: Public Domain, per source file header
- Attribution: opendesktop.org.tw phone.cin contributors; KeyKey Boneyard maintainers
- Redistribution decision: included for public releases starting in `2026.06.5`

This source is imported after libchewing-data and before Rime essay. The release builder uses it only as a low-priority single-character reading supplement:

1. It imports CJK BMP characters only.
2. It excludes non-BMP and private-use characters.
3. It only adds missing exact `(reading, character)` pairs.
4. It does not override libchewing character frequencies.

This fills native/Yahoo character coverage gaps such as the `ㄨㄛˇ` candidate set:

```text
我 婐 捰 倭 䂺 婑 䰀 㦱
```

The generated source inventory is stored at:

```text
sources/bpmf-ext-cin/source-inventory.sha256
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

The v1 normalized TSV uses the current KeyKey / Manjusri internal `qstring` reading representation in the first column. This is the two-byte-per-syllable ordering string produced by the historical builder's `absolute_order_string` function, not literal Bopomofo text.

This keeps the first release directly compatible with the current database reader. A later source-normalization pass can add a human-readable Bopomofo column if the builder contract changes.

## Current Risk Notes

This release is still a seed lexicon, but it now includes a substantially larger modern Traditional Chinese / Zhuyin layer.

Expected follow-up work:

1. Add Taiwan-specific modern phrases based on actual misses.
2. Tune the cross-source weight mapping after real typing tests.
3. Review LGPL redistribution requirements whenever release packaging changes.
4. Review CC BY-SA and research-only sources before any future public release includes them.
