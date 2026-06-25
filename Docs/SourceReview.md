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

During release cooking, the builder augments the canned-message payload with a
set of button categories generated from `chiakey-symbols-overlay/symbols.tsv`,
so the visible symbol table receives the same supplemental symbols as
`_punctuation_list` without placing every symbol in one oversized category. It
also replaces the original annotated `顏文字` category with a Mozc-backed
`Messages` list, so the symbol table displays only the emoticon string instead
of strings such as `顏文字 + 中文說明`.

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

## Included Starting in 2026.06.9

### chiakey-symbols-overlay

- Name: ChiaKey supplemental symbol list
- Local source: `sources/chiakey-symbols-overlay/symbols.tsv`
- License: CC0-1.0
- Attribution: ChiaKey Lexicon maintainers
- Redistribution decision: included for public releases starting in `2026.06.9`

This source supplements the original KeyKey punctuation list with project-owned
symbols that are useful in modern text input: extended punctuation, currency
signs, legal and trademark marks, CJK symbols, enclosed numbers, roman numeral
variants, additional arrows, mathematical operators and relations, check marks,
stars, card suits, music symbols, and units.

The release builder imports this source only as `_punctuation_list` rows. It is
loaded after `keykey-punctuations-cin`, and skips any symbol already present in
the Yahoo KeyKey punctuation list so the original ordering and direct
punctuation key mappings are preserved.

The same source is also used to derive several supplemental canned-message
button categories inside `prepopulated_service_data/canned_messages`, because
the app symbol table reads canned messages rather than querying
`_punctuation_list` directly. The generated categories are `補充標點`,
`貨幣與標記`, `數字序號`, `補充箭頭`, `補充數學`, `勾叉與星號`,
`花色與音樂`, and `單位符號`.

The generated source inventory is stored at:

```text
sources/chiakey-symbols-overlay/source-inventory.sha256
```

### mozc-emoticon-data

- Name: Mozc emoticon data
- Local source:
  - `sources/mozc-emoticon-data/raw/categorized.tsv`
  - `sources/mozc-emoticon-data/raw/emoticon.tsv`
- Upstream source directory: <https://github.com/google/mozc/tree/master/src/data/emoticon>
- Upstream commit: `28da5a39f9a7fd70251c85d269f4a8b47aa31cf8`
- License: BSD-3-Clause
- Attribution: Google and Mozc contributors
- Redistribution decision: included for public releases starting in `2026.06.9`

This source replaces the original KeyKey `顏文字` canned-message category.
The release builder reads `categorized.tsv` first, then appends additional
unique emoticon values from `emoticon.tsv`. Only the first column, the emoticon
itself, is emitted into `prepopulated_service_data/canned_messages`; Japanese
reading keys, categories, and descriptions are kept only as source context.

The generated category intentionally remains a plain `Messages` list, without
`IsSymbolButtonList` and without `Buttons`, because the original 顏文字 UX is a
list. This keeps the symbol-table UI from showing the legacy Chinese
annotations bundled with the Yahoo-era canned-message data while preserving the
list interaction.

The generated source inventory is stored at:

```text
sources/mozc-emoticon-data/source-inventory.sha256
```

### chiaki-web-overlay

- Name: Chiaki reviewed web corpus overlay
- Local sources:
  - `sources/chiaki-web-overlay/explicit.tsv`
  - `sources/chiaki-web-overlay/bigrams.tsv`
- Source material: reviewed web-derived Taiwan internet usage material
- License: CC0-1.0 for the reviewed overlay rows; source text is not redistributed
- Attribution: Chiaki.C
- Redistribution decision: included for ChiaKey public releases; other projects
  or non-ChiaKey uses should exclude this source by default unless they perform
  their own source review

I use this source as a narrow ChiaKey overlay for reviewed unigram and bigram
values derived from web usage material. Because web-derived terms can carry
context, provenance, or licensing risk outside this specific lexicon use case, I
do not recommend treating it as a general-purpose reusable source. The
repository redistributes only the final lexicon rows in the release-builder
formats:

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
qstring<TAB>previous<TAB>current<TAB>probability
```

The release builder imports unigram rows after `chiakey-modern-overlay` and
before `opencc-variant-policy`. Bigram rows are imported into the runtime
`bigrams` table after unigram policies have been applied.

The generated source inventory is stored at:

```text
sources/chiaki-web-overlay/source-inventory.sha256
```

### chiaki-synthetic-overlay

- Name: Chiaki.C GPT-5.5 synthetic Taiwan internet usage overlay
- Local sources:
  - `sources/chiaki-synthetic-overlay/unigrams.tsv`
  - `sources/chiaki-synthetic-overlay/bigrams.tsv`
- Source material: GPT-5.5-generated synthetic "Taiwan internet usage" (台灣網路用語) corpus
- License: CC BY-NC 4.0; commercial use requires permission from Chiaki.C
- Attribution: Chiaki.C
- Redistribution decision: included for public source review, open-source project use, and non-commercial release builds starting in the next synthetic Taiwan internet usage overlay refresh

I generated this source with GPT-5.5 for simulated "Taiwan internet usage"
(台灣網路用語), then reduced it through project cleaning and statistical
selection for ChiaKey lexicon maintenance. The raw synthetic corpus is not
redistributed in this repository; only the final lexicon rows are kept:

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
qstring<TAB>previous<TAB>current<TAB>probability
```

The bigram file is prefiltered against the release unigram table and keeps
sentence-boundary rows using `!` / `$` qstring markers. Redundant synthetic
rows and weak single-character pairings are removed conservatively before
release.

This source was generated as OpenAI output for which OpenAI assigns any OpenAI
right, title, and interest in Output to the user, to the extent permitted by
applicable law. I am not using this material for a competing model, and I record
it as my synthetic overlay data rather than as an external public-domain corpus.

The generated source inventory is stored at:

```text
sources/chiaki-synthetic-overlay/source-inventory.sha256
```

### openformosa-common-voice-25-zh-tw

- Name: OpenFormosa Common Voice 25 zh-TW bigram overlay
- Local source: `sources/openformosa-common-voice-25-zh-tw/bigrams.tsv`
- Source material: OpenFormosa Common Voice 25 zh-TW validated sentences
- Upstream dataset: <https://huggingface.co/datasets/OpenFormosa/common_voice_25_zh-TW>
- License: CC0-1.0
- Attribution: OpenFormosa / Mozilla Common Voice contributors
- Redistribution decision: included for public releases as selected runtime
  bigram rows

This source contributes only filtered runtime bigram rows. The raw Common Voice
sentences are not redistributed in this repository.

The generated source inventory is stored at:

```text
sources/openformosa-common-voice-25-zh-tw/source-inventory.sha256
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
The character-frequency mapping keeps a small single-character segmentation penalty so common characters do not accidentally outrank explicit phrase rows with the same reading.
Lower-ranked multi-character libchewing rows also receive a bounded segment bonus so known phrases such as `地基` and `權重` can outrank same-reading character-by-character splits without pushing already-strong phrases beyond the original top of the phrase scale.
Weak single-character readings can additionally be promoted when several high-frequency libchewing phrases start with that exact character and reading. This keeps readings such as `數` / `ㄕㄨˋ` visible in candidate lists based on evidence from phrases like `數位`, `數學`, `數量`, and `數字`, while still keeping the single character below the strongest phrase evidence.

The final release database also derives `associated_phrases` from the assembled `unigrams` table for the runtime associated-phrase module. Each row maps a committed head character to comma-separated phrase tails, so a committed `我` can suggest tails such as `們` and `的`. This table is generated after all lexical imports and policy layers have been applied, and validation requires representative rows before a release can finish.

The raw files are vendored under `sources/libchewing-data/raw/` so release
builds do not need network access. Maintainers refresh pinned snapshots with:

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

Rime essay has useful modern vocabulary and scores, but it does not include Zhuyin readings. The release builder therefore uses it only after another source has supplied a reading.

First, an overlap rerank pass uses Rime scores to promote existing candidates inside the same KeyKey qstring group. This pass only raises lower-ranked candidates enough to respect Rime's ordering, never demotes existing rows, and caps promotions so Rime cannot push an ambiguous candidate beyond the established high-frequency phrase range.

Then a low-priority supplemental phrase pass imports entries that satisfy all of these constraints:

1. The phrase is not already present after the libchewing-data import.
2. The phrase length is between 2 and 7 Unicode codepoints.
3. The Rime score is at least `40`.
4. Every character has a primary single-character reading in the current database.

During this supplemental pass, entries that would otherwise be ranked below an existing split path are promoted just enough to beat that split, with a cap at the top of the Rime supplemental range. This gives complete phrases such as `趁現在` a small segmentation advantage over unlikely character-plus-phrase paths such as `稱` + `現在`, without using per-phrase explicit overrides.

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
