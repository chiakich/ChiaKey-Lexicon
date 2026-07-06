# ChiaKey Lexicon

[中文](README.md)

ChiaKey Lexicon is the lexicon-data repository for ChiaKey.

The main input-method repository stay focus on the macOS runtime. This repository is responsible for evolving lexicon data, source manifests, licensing records, release database artifacts, checksums, and changelog history.

## Why this project exists

There is no shortage of open-source Traditional Chinese / bopomofo lexicons, but almost all of them are "unigram" data — i.e. "word / phrase + frequency":

- libchewing's `tsi.csv` is "phrase, frequency, bopomofo".
- Rime's shared `essay.txt` is "word, frequency".

This kind of data tells you which word is more common, but rarely describes the transition between words (bigram / transition probability) — i.e. "after typing A, is B or C the more sensible continuation". That is exactly the information that matters most for homophone disambiguation and auto candidate selection. (Rime *can* support n-grams via the octagram grammar plugin, but that is an engine add-on paired with a separately trained `.gram` model, not shared corpus data distributed alongside the lexicon.)

What `ChiaKey-Lexicon` aims to add is precisely this layer: on top of mature unigram lexicons, it overlays bigram data (from web corpora, Mozilla Common Voice sentences, and synthetic corpora), produced through a reproducible, source-traceable pipeline into releases the input method can consume directly.

## Thanks

This project did not appear out of nowhere; it builds on excellent open-source lexicons and years of community effort. With gratitude to:

- **Chewing / libchewing** (`chewing/libchewing-data`): the primary modern Traditional Chinese / bopomofo vocabulary and explicit-reading base.
- **Rime** (`rime/rime-essay`): high-quality word frequencies and segmentation evidence, a key basis for candidate rerank and supplemental phrases.
- **Mozilla Common Voice / OpenFormosa**: the corpus source for bigram sentences.
- **Mozc**: preloaded emoticon (`顏文字`) category data.

Our work is mostly "curation, connection, and reinforcement": integrating these predecessors' work into a modern, bigram-aware, reproducible, source-traceable input-method lexicon. License, redistribution decisions, and risk notes for each source are documented in [Docs/SourceReview.md](Docs/SourceReview.md).

## Architecture

This repository is centered on a reproducible data pipeline:

1. `sources/<source-id>/` stores each reviewed input source and its local README. `source-inventory.sha256` is maintained only for compatibility-base and external-source layers, to track provenance for vendored or pinned upstream files.
2. License files live in each `sources/<source-id>/` directory so every source keeps its own license text or license notes nearby.
3. `src/` contains the Rust release toolchain. It validates inputs, imports data layers into the KeyKey database shape, writes generated audit artifacts, updates release metadata, and generates manifests.
4. `normalized/smart-mandarin.tsv` is the generated normalized audit view of Smart Mandarin language-model rows, and is not committed.
5. `manifests/lexicon-manifest.json` is the generated update contract consumed by the app, and is not committed; release builds copy it into `dist/`.
6. `dist/dev/` or `dist/<version>/` is the local staging directory for release artifacts, and is not committed.

The data layer model has four categories:

1. **Runtime compatibility data**: KeyKey-origin data required by existing readers and input modules.
2. **Lexicon sources**: modern Traditional Chinese / bopomofo vocabulary and supplemental coverage.
3. **Project-owned corrections**: small overlays for known input gaps, explicit readings, and candidate-order adjustments.
4. **Policy layers**: reviewed rule layers that keep default Traditional Chinese releases aligned with expected language and region behavior.

## Data Layers

This repository is not managed as a flat source list. Sources are organized into four layers that the release builder applies in a fixed order for traceable behavior.

### Compatibility Base Lexicon

Goal: keep compatibility with ChiaKey runtime expectations, existing schema, and module tables.

- `keykey-boneyard-bootstrap`: initial cooked release DB base (`KeyKeySource.db`).
- `keykey-punctuations-cin`: BPMF punctuations and `_ctrl_*` compatibility rows.
- `keykey-module-cin`: `Generic-cj-cin`, `Generic-simplex-cin`, Cangjie punctuation tables, and `BopomofoCorrection-bopomofo-correction-cin`.
- `keykey-prepopulated-service-data`: `canned_messages` and timestamps.
- `bpmf-ext-cin`: supplemental single-character `(reading, character)` coverage.

### External Lexicon Sources

Goal: provide reviewable and redistributable external vocabulary and reading coverage.

- `libchewing-data`: primary modern Traditional Chinese / bopomofo lexicon layer.
- `rime-essay`: lower-priority supplemental terms and rerank evidence.
- `mozc-emoticon-data`: supplemental `Emoticon` preloaded category rows.

### Project Lexicon Sources

Goal: project-maintained lexicon data that directly reflects ChiaKey usage context.

- `chiaki-modern-overlay`: project-owned fixes and explicit reading/order adjustments.
- `chiaki-web-overlay`: reviewed web-usage unigram and bigram supplements.
- `chiaki-synthetic-overlay`: synthetic-corpus-derived unigram and bigram supplements.
- `openformosa-common-voice-25-zh-tw`: selected bigram rows from Common Voice data.
- `chiaki-auto-hotwords-overlay`: automatically refreshed hotwords overlay (project-output rows only).
- `chiaki-symbols-overlay`: supplemental `_punctuation_list` symbols and runtime punctuation candidates.

### Policy Layers

Goal: map external evidence into default zh-TW output expectations and suppress known segmentation risks.

- `chiaki-rime-conversion-policy`: post-OpenCC Rime overrides for cases `t2tw` cannot safely decide, such as `里` in place names and `里肌` food terms.
- `chiaki-fragment-denylist`: fragment weight caps to reduce bad segmentation from non-lexical shards.

The release builder also derives `associated_phrases` from final `unigrams` for runtime phrase suggestions. This is not an independent source layer; it provides head-character -> phrase-tail candidates (for example, after `我`, suggest `們` or `的`).

## Integration Flow

The release builder integration flow is deterministic:

1. Validate required source files. Generate `source-inventory.sha256` for compatibility-base and external-source entries that include vendored or pinned upstream files.
2. Copy cooked `KeyKeySource.db` from `keykey-boneyard-bootstrap` as the base.
3. Import `libchewing-data` to strengthen modern vocabulary with explicit bopomofo readings; overlapping bootstrap phrases are replaced by libchewing data.
4. Import `bpmf-ext-cin` to fill missing single-character readings without overwriting existing rows.
5. Batch-normalize Rime essay phrases with OpenCC `t2tw`, then apply the small `chiaki-rime-conversion-policy` override table; the normalized result is shared by Rime rerank and supplemental import passes.
6. Apply `rime-essay` rerank: cap same-pronunciation boosts, allow limited uplift from Rime evidence for weak existing phrases, apply small single-character homophone reorders where frequency advantage is sufficient, then import only safe supplemental phrases not already in DB.
   - Supplemental `split-rerank` is intentionally conservative: if the Rime base is too far below the best existing split, it stays on the Rime scale; otherwise it only receives a bounded boost. This prevents high-frequency character splits such as `的`+`是` from flattening every candidate in the same qstring group, for example `地市` and `的事`.
7. Import `chiaki-modern-overlay/phrases.tsv` so project-owned fixes can replace known problematic phrases.
8. Import `chiaki-modern-overlay/explicit.tsv` for explicit qstring and ranking corrections.
9. Import `chiaki-web-overlay/explicit.tsv` and `chiaki-synthetic-overlay/unigrams.tsv`.
10. Generate OpenCC `t2tw` same-qstring variant weight caps for candidates that already have Taiwan-standard counterparts, then apply `chiaki-fragment-denylist` to keep non-lexical fragments below safety thresholds.
11. Import `chiaki-synthetic-overlay/bigrams.tsv`, then `openformosa-common-voice-25-zh-tw/bigrams.tsv`, then `chiaki-web-overlay/bigrams.tsv` so reviewed web bigrams can override overlapping statistical rows.
12. Import runtime compatibility data: BPMF punctuations, supplemental symbol list, canned messages, Mozc emoticons, and module CIN tables.
13. Derive `associated_phrases` from final `unigrams` for runtime phrase suggestions.
14. Run runtime-required validations and write normalized TSV, release metadata, manifest, and checksums.

After integration, each traceable row carries source path, source kind, checksum, and tags. The app consumes generated `ChiaKeySource.db` and `lexicon-manifest.json`; maintainers can trace row origins through generated `normalized/smart-mandarin.tsv` and metadata after local builds.

Source-specific licensing decisions, redistribution decisions, and risk records are documented in [Docs/SourceReview.md](Docs/SourceReview.md). Day-to-day release operations are documented in [Docs/ReleaseFlow.zh-TW.md](Docs/ReleaseFlow.zh-TW.md).


## License Policy

Rust release tooling and repository scripts are licensed under the MIT License; see [LICENSE](LICENSE).

Lexicon data does not have a single repository-wide license.

Every source must declare its own license before public release. Unknown-license data may be used only for local experiments and must not be included in public release artifacts.

The project-authored `chiaki` series lexicons and lists are released under CC BY-NC 4.0 by default.

Academic research and personal non-commercial projects are welcome to use them freely, with attribution to the original author.

Commercial Use:
If your project involves commercial or revenue-generating use (for example: integration into paid products, commercial API products, or internal enterprise deployment), it is outside the scope of the default license terms above. For commercial licensing, please contact:

Contact: maid@chiaki.ch
