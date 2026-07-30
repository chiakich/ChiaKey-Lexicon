# ChiaKey Lexicon

[中文](README.md)

<img width="256" height="256" alt="chiakey-lexicon-icon" src="https://github.com/user-attachments/assets/222e1ddb-65b4-419c-88df-1f10b841ef49" />

ChiaKey Lexicon is the Traditional Chinese / bopomofo lexicon-data project for [ChiaKey](https://github.com/chiakich/ChiaKey). The input method itself focuses on its runtime; this project maintains evolving vocabulary, readings, and contextual data, then reproducibly builds them into release databases that the input method can use directly.

The project addresses more than word frequency—"which word is more common"—by tackling a problem with an outsized effect on bopomofo input: choosing the right word among homophones.

- Extracts Taiwan-context data from about 460 million characters of real text, including government news, Legislative Yuan gazettes, PTT, Plurk, and Traditional Chinese Wikipedia.
- Its primary layer contains 292,639 homophone-disambiguation bigrams, all targeting cases where a user's usual reading can change the selection result.
- Uses held-out tests across four domains—written prose, formal speech, forums, and messages—to measure both corrected selections and incorrect candidate takeovers, rather than judging quality by row count alone.
- Records the source, license, and integration history for every data layer; releases can be rebuilt and traced from reviewed inputs.

> Sponsorship is welcome and supports ongoing development, updates, and maintenance of this lexicon.

#### Sponsorship outside Taiwan

[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/A0A21UAIV9)

#### Taiwan credit cards / Apple Pay / ATM / convenience-store codes only

[Sponsor via ECPay](https://p.ecpay.com.tw/2A3B186)

## Report Lexicon Issues

- [Missing word report](https://github.com/akira02/ChiaKey-Lexicon/issues/new?template=add-unigram.yml): report a standalone word that should exist in the lexicon but is currently missing, for example `泳鏡`.
- [Long-sentence selection error](https://github.com/akira02/ChiaKey-Lexicon/issues/new?template=add-bigram.yml): report a wrong selection when two existing words occur in sequence, for example expecting `天意難測` but getting `天意南側`.

## Why this project exists

As the open-source successor to Yahoo! KeyKey Input Method, [ChiaKey](https://github.com/chiakich/ChiaKey) depends chiefly on bopomofo tables, unigram word-frequency data, and bigram tables. Open-source Traditional Chinese / bopomofo lexicons are abundant, but nearly all use the form "word or phrase + frequency":

- libchewing's `tsi.csv` is "phrase, frequency, bopomofo".
- Rime's shared `essay.txt` is "word, frequency".

These resources can tell us which word is more common, but not how words continue one another: after typing A, is B or C the more sensible continuation? That information matters most for homophone disambiguation and automatic selection. Taiwan's open-source bopomofo ecosystem has long lacked practical n-gram inference lexicons; static text does not necessarily reflect contemporary Taiwanese writing and everyday language. At the same time, n-gram weights require data cleaning, probability calculation, and model compilation, making them much harder to update continuously than a plain-text unigram lexicon.

ChiaKey Lexicon layers verified bigram data on mature unigram lexicons and produces a release database through a reproducible, source-traceable pipeline.

### Where the bigram data comes from

The original approach asked language models to write realistic passages, then extracted word-pair patterns from them in the hope that volume would cover real-world distributions. We tried two versions: `chiaki-synthetic-overlay`, and a later approach that enumerated colliding entries with local Gemma 4 and asked the model for context. Once the evaluation tools existed, both approaches were rejected. Model-predicted pairings were almost entirely outside actual usage, hitting only about 200 times in a 326,000-sentence test corpus.

The primary bigram layer is now `chiaki-tw-homophone-bigram`, extracted directly from real Taiwanese text: government news releases, Legislative Yuan gazette Q&A transcripts, PTT, public Plurk posts, and Traditional Chinese Wikipedia articles—about 460 million characters in total.

#### Keeping only pairs that can change an outcome

This research found that a bigram has only one role in a bopomofo input method: at the reading a user actually types, promote a homophone candidate that would otherwise lose. If a word already has the highest weight at its usual reading, the walker already selects it; a bigram for that word adds size but cannot change any outcome.

Under this criterion, only 11.6% of the 46,822 rows in the older synthetic layer could change a result. The new layer enforces the condition during generation: all 292,639 rows occur at colliding positions where the desired candidate loses without context.

#### Gating on real input accuracy

The project uses held-out evaluation: it segments real text, determines for each adjacent word pair whether the walker would select incorrectly without the bigram, then measures how many positions a layer fixes and how many already-correct positions it takes over incorrectly. Evaluation replays release calibration and, following the reachability classes in [Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md), counts only rows that are always effective.

The test data covers four domains: government news (written prose), Legislative Yuan gazettes (formal speech), PTT (forums), and group conversations between the author and friends. All participants consented to their use as a benchmark; the latter corpus is used only for evaluation, is not used for training, and is not distributed by this project.

Experiments show that domain-specific measurement is essential. A version trained only on public-affairs material had a net gain of +85,128 in written prose, but only +65 in real messages: 1,603 fixes and 1,538 incorrect takeovers, almost entirely cancelling out. Adding PTT data improved message-domain performance by roughly 65 times. A single metric would not have revealed this.

#### From corpus to weights

`bigrams.tsv` has the format `qstring<TAB>previous<TAB>current<TAB>probability`; sentence-boundary rows are also allowed, with one side left empty and marked by `!` / `$`. `probability` is an internal strength ordering, not a conditional probability.

During import, values are calibrated against the unigram score (`src/importers.rs`, `calibrate_bigram_boost`):

```
stored = min( unigram(current) + boost + (raw − raw_max_of_source), −0.05 )
```

The default `boost` is 1.5 and can be overridden per source with an environment variable (setting it to 0 passes raw values through). `raw − raw_max_of_source` preserves the source's confidence ordering while anchoring the complete set to the unigram baseline: sufficiently strong disambiguation edges rise above the character-by-character unigram path and become effective; weaker edges remain inert below the baseline.

The full method, measured results, and known limitations for each layer are in the research appendix of [sources/chiaki-tw-homophone-bigram/README.md](sources/chiaki-tw-homophone-bigram/README.md).

## Acknowledgments

This project builds on years of work by excellent open-source lexicons and communities. We are grateful to:

- **Chewing / libchewing** (`chewing/libchewing-data`): the primary modern Traditional Chinese / bopomofo vocabulary and explicit-reading base.
- **Rime** (`rime/rime-essay`): high-quality word frequencies and segmentation evidence, used for candidate reranking and supplemental terms.
- **Mozilla Common Voice / OpenFormosa**: source material for bigram sentences.
- **Legislative Yuan Gazette Information Network / g0v `ly.govapi.tw`**: Q&A transcripts that provide spontaneous spoken-language bigram material.
- **The Executive Yuan, Mainland Affairs Council, Academia Sinica, Hakka Affairs Council, and New Taipei City Government**: government news releases under the Government Open Data License, providing written-language bigram material.
- **PTT, Plurk, and Traditional Chinese Wikipedia**: forum, public-post, and encyclopedia-text sources for the primary bigram layer; each source's details are recorded in its source-specific license notice.
- **Mozc**: preloaded emoticon-category data.
- **Ministry of Education Revised Mandarin Chinese Dictionary / `g0v/moedict-data`**: source for heteronym reading supplements in `chiaki-modern-overlay/reading-supplements.tsv`, reproduced under the Ministry's CC BY-ND 3.0 Taiwan terms; attribution: Ministry of Education, Republic of China (Lifelong Education Administration).
- **G.yu, Attorney**: for legal discussion and feedback. This acknowledgment does not constitute formal legal advice, legal representation, or any warranty by G.yu; G.yu assumes no responsibility for this project's data, licensing, or use.

Our work primarily integrates these predecessors' contributions into a modern, reproducible, source-traceable input-method lexicon. The license, integration decisions, and risk records for every source are documented in [Docs/SourceReview.md](Docs/SourceReview.md).

Further documentation:

- [Lexicon release workflow](Docs/ReleaseFlow.zh-TW.md)
- [Source review](Docs/SourceReview.md)
- [ChiaKey walker scoring](Docs/WalkerScoring.zh-TW.md)
- [Bigram reachability audit and pruning](Docs/BigramPruning.zh-TW.md)

## Architecture

This repository is centered on a reproducible data pipeline:

1. `sources/<source-id>/` stores each reviewed input source and its local README. `source-inventory.sha256` is maintained only for compatibility-base and external lexicons, for provenance of vendored or pinned upstream files.
2. License files live in the corresponding `sources/<source-id>/` directory, keeping the required license text or notes next to each publicly released source.
3. `src/` is the Rust release toolchain. It validates inputs, imports data layers into the KeyKey database shape, writes generated audit artifacts, updates release metadata, and generates manifests.
4. `normalized/smart-mandarin.tsv` is the generated normalized audit view of Smart Mandarin language-model rows, and is not committed.
5. `manifests/lexicon-manifest.json` is the generated update contract consumed by the input method, and is not committed; release builds copy it to `dist/`.
6. `dist/dev/` or `dist/<version>/` is the local staging directory for release artifacts, and is not committed.

The data model has four broad categories:

1. **Compatibility base lexicons**: KeyKey-origin data required by existing database readers and input modules.
2. **External lexicons**: modern Traditional Chinese / bopomofo vocabulary and supplemental word coverage.
3. **Project lexicons**: small overlays for known input gaps, explicit readings, and candidate-order adjustments.
4. **Policy layers**: small, reviewed rules that keep the default Traditional Chinese release aligned with expected language and regional behavior.

## Data Layers

This repository is not managed as one flat source list. Sources belong to four layers that the release builder applies in a fixed order, preventing untraceable overrides.

### Compatibility Base Lexicons

Goal: retain compatibility with the ChiaKey runtime, existing schema, and module tables.

- `keykey-boneyard-bootstrap`: the initial cooked release DB base (`KeyKeySource.db`).
- `keykey-punctuations-cin`: BPMF punctuation and `_ctrl_*` compatibility data.
- `keykey-module-cin`: `Generic-cj-cin`, `Generic-simplex-cin`, Cangjie punctuation, and `BopomofoCorrection-bopomofo-correction-cin`.
- `keykey-prepopulated-service-data`: `canned_messages` and timestamps.
- `bpmf-ext-cin`: supplemental single-character `(reading, character)` coverage.

### External Lexicons

Goal: provide reviewable, redistributable external vocabulary and reading coverage.

- `libchewing-data`: the primary modern Traditional Chinese / bopomofo lexicon layer.
- `rime-essay`: lower-priority supplemental terms and reranking evidence.
- `mozc-emoticon-data`: supplemental `Emoticon` preloaded category data.

### Project Lexicons

Goal: maintain lexicon data within this project.

- `chiaki-auto-hotwords-overlay`: an automatically refreshed hotword overlay that keeps only project-output rows.
- `chiaki-symbols-overlay`: missing `_punctuation_list` symbols and runtime punctuation candidates.
- `chiaki-web-overlay`: unigram and bigram supplements for web usage.
- `chiaki-tw-homophone-bigram`: bigram rows extracted from government news, Legislative Yuan gazettes, PTT, Plurk, and Traditional Chinese Wikipedia. It keeps only pairs that lose to a homophone at their usual reading; this is the primary non-commercial bigram layer.
- `chiaki-tw-homophone-bigram-clean`: a standalone ODbL bigram add-on rebuilt from government news and Legislative Yuan gazettes.
- `chiaki-modern-overlay`: project-owned unigram supplements, exact overrides, and bigram corrections; it retains provenance tags from the former `chiaki-synthetic-overlay`.
- `openformosa-common-voice-25-zh-tw`: selected bigram rows from Common Voice sentences.
- `tw-ly-transcript`: manually reviewed bigram rows from Legislative Yuan gazette Q&A transcripts. The entire layer was disabled on 2026-07-29; its data and research record are retained for traceability.

### Policy Layers

Goal: translate external evidence into the expected default Traditional Chinese (zh-TW) output and suppress known segmentation risks.

- `chiaki-rime-conversion-policy`: post-OpenCC `t2tw` exceptions for project preferences that `t2tw` cannot decide safely, such as `里` in place names and `里肌` in food terms.
- `chiaki-fragment-denylist`: sentence-fragment weight caps that reduce bad segmentation caused by character stealing; it is applied after explicit overlays and takes precedence over normal candidate-order adjustments.

## Integration Flow

The release builder has a deterministic integration process:

1. Validate that every required source file exists, and generate `source-inventory.sha256` for vendored or pinned upstream files in the compatibility-base and external-lexicon layers.
2. Copy the cooked `KeyKeySource.db` from `keykey-boneyard-bootstrap` as the base.
3. Import `libchewing-data` to add modern vocabulary with explicit bopomofo readings; libchewing phrases replace old derived data for the same phrases in the bootstrap source.
4. Import `bpmf-ext-cin` to fill missing single-character readings without overwriting existing rows.
5. Batch-apply OpenCC `t2tw` to Rime essay phrases, then apply the small `chiaki-rime-conversion-policy` post-processing exceptions; the normalized result is shared between Rime reranking and supplemental import.
6. Apply `rime-essay` reranking: same-reading candidates may receive only bounded boosts; weak existing terms may receive limited uplift from Rime scores and segmentation evidence; single-character homophone groups can be slightly reordered when Rime frequency is sufficiently stronger. Then add only supplementary phrases that are absent from the current DB and whose bopomofo can be inferred safely.
   - Supplemental phrase `split-rerank` is deliberately conservative. If the Rime base is too far below the best existing segmentation, it is not boosted; otherwise it receives only a bounded boost. This prevents high-frequency splits such as `的` + `是` from flattening every candidate in a qstring group, for example `地市` and `的事`.
7. Import `chiaki-web-overlay/unigrams.tsv`, `chiaki-modern-overlay/unigrams.tsv`, and auto-hotwords; use phrase evidence to reinforce single-character readings.
8. Generate same-qstring variant weight caps from OpenCC `t2tw`, then apply Rime single-character homophone reranking.
9. Import `chiaki-modern-overlay/explicit.tsv` for project-owned, exact corrections that require a specific qstring or order. It overrides all general unigram sources and the preceding policies.
10. Apply `chiaki-fragment-denylist` last to cap non-lexical fragments that steal characters. This safety cap takes precedence over explicit overlays.
11. Import bigram sources in order: `openformosa-common-voice-25-zh-tw`, `chiaki-tw-homophone-bigram`, `chiaki-web-overlay`, and `chiaki-modern-overlay`. Later imports override overlapping rows, so the manually reviewed web overlay and manual corrections sit above corpus-statistics sources.
12. Add runtime compatibility data: BPMF punctuation, the ChiaKey supplemental symbol list, canned messages, Mozc emoticons, and module CIN tables.
13. Derive `associated_phrases` from final `unigrams` for phrase suggestions.
14. Run runtime-required validations and write normalized TSV, release metadata, manifests, and checksums.

The release builder also derives the runtime `associated_phrases` table from the integrated `unigrams`. This is not an independent source; it provides head-character → phrase-tail candidates for suggestions, such as suggesting `們` or `的` after `我`.

After integration, every traceable lexicon row carries a source path, source kind, checksum, and tags. The input method consumes the generated `ChiaKeySource.db` and `lexicon-manifest.json`; after a local build, maintainers can trace data origins through the generated `normalized/smart-mandarin.tsv` and metadata.

The license and redistribution decisions, together with risk records, are in [Docs/SourceReview.md](Docs/SourceReview.md). Day-to-day release operations are documented in [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md).

## License Policy

The Rust release tooling and repository scripts use the MIT License; see [LICENSE](LICENSE).

Lexicon data does not have one repository-wide license.

Every source must declare its own license before a public release. Data with unknown licensing may be used only for local experiments and must not appear in public release artifacts.

Project-authored experimental `chiaki` lexicons and lists use CC BY-NC 4.0 by default; a source-specific license notice takes precedence over that default.

Academic research and personal non-commercial projects are welcome to use them with attribution to the original author.

Commercial use depends on the source-specific license. `chiaki-tw-homophone-bigram` provides no commercial exception; for commercially usable homophone-disambiguation data, use the ODbL `chiaki-tw-homophone-bigram-clean` add-on.

## Future Work

- Remove compositional entries from the unigram layer. The lexicon includes about 6,300 Rime-essay-derived entries of the form “known word + directional or function word” (such as `電影裡` and `適當的`); 291 of them outrank the correct candidate at their own readings (`適當的` over `適當地`, `集中了` over `擊中了`). These whole-word paths win before the bigram layer can intervene. The bigram layer must be regenerated after cleanup.
- Prune the 15,358 Class B (never-winning) rows in `chiaki-tw-homophone-bigram`, along with newly discovered Class E (whole-word-path-winning) dead rows. See [Docs/WalkerScoring.zh-TW.md](Docs/WalkerScoring.zh-TW.md) for the criteria.
- Recover 767,923 candidates that were discarded because `previous` has multiple readings; this requires reading disambiguation.
- Add contemporary Taiwanese terms based on actual gaps.
- Adjust cross-source weight mapping through real typing tests.
- Recheck LGPL redistribution requirements when external lexicons change.
- Investigate whether lexicons from sources such as the Ministry of Education and National Academy for Educational Research can be incorporated.
