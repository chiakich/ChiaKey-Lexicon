# License Notes

The Rust release tooling and repository scripts are licensed under the MIT License; see [LICENSE-CODE](../LICENSE-CODE).

This repository does not declare a single license for all lexicon data.

Each source must document:

1. source name
2. source URL
3. license
4. attribution requirement
5. redistribution permission
6. transformation steps
7. checksum of the fetched source

Unknown-license data must not be included in public release artifacts.

## Reviewed v1 Source Licenses

### KeyKey Boneyard bootstrap data

- Source id: `keykey-boneyard-bootstrap`
- License file: [YahooKeyKey-BSD-3-Clause.txt](YahooKeyKey-BSD-3-Clause.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included in `2026.06.1`

### KeyKey BPMF punctuation table

- Source id: `keykey-punctuations-cin`
- License file: [YahooKeyKey-BSD-3-Clause.txt](YahooKeyKey-BSD-3-Clause.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.6`

### KeyKey prepopulated service data

- Source id: `keykey-prepopulated-service-data`
- License file: [YahooKeyKey-BSD-3-Clause.txt](YahooKeyKey-BSD-3-Clause.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.6`

### KeyKey module CIN tables

- Source id: `keykey-module-cin`
- License files: [YahooKeyKey-BSD-3-Clause.txt](YahooKeyKey-BSD-3-Clause.txt), [bpmf-ext-cin-Public-Domain.txt](bpmf-ext-cin-Public-Domain.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.6`

### ChiaKey modern overlay phrases

- Source id: `chiakey-modern-overlay`
- License file: [CC0-1.0.txt](CC0-1.0.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.2`

### ChiaKey supplemental symbol list

- Source id: `chiakey-symbols-overlay`
- License file: [CC0-1.0.txt](CC0-1.0.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.9`

### Chiaki.C GPT-5.5 synthetic Taiwan internet usage overlay

- Source id: `chiaki-synthetic-overlay`
- License file: [chiaki-synthetic-overlay-NC.txt](chiaki-synthetic-overlay-NC.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included for public source review, open-source project use, and non-commercial release builds
- Note: licensed under CC BY-NC 4.0; non-commercial and open-source projects may use this source, but commercial use requires permission from Chiaki.C

### OpenFormosa Common Voice 25 zh-TW bigram overlay

- Source id: `openformosa-common-voice-25-zh-tw`
- License file: [CC0-1.0.txt](CC0-1.0.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included as selected runtime bigram rows

### OpenCC-derived Traditional Chinese variant policy

- Source id: `opencc-variant-policy`
- License file: [opencc-Apache-2.0.txt](opencc-Apache-2.0.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.7`

### libchewing-data

- Source id: `libchewing-data`
- License file: [libchewing-data-LGPL-2.1-or-later.txt](libchewing-data-LGPL-2.1-or-later.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.3`

### Public domain extended BPMF character table

- Source id: `bpmf-ext-cin`
- License file: [bpmf-ext-cin-Public-Domain.txt](bpmf-ext-cin-Public-Domain.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.5`

### Rime essay

- Source id: `rime-essay`
- License file: [rime-essay-LGPL-3.0.txt](rime-essay-LGPL-3.0.txt)
- Review notes: [Docs/SourceReview.md](../Docs/SourceReview.md)
- Release decision: included starting in `2026.06.3`

New code should remain under the project code license unless a file explicitly states otherwise. New lexicon data must continue to declare source-specific licensing before public release.
