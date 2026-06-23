# ChiaKey Modern Overlay

This source contains small project-owned overrides used to fix obvious misses in
the seed lexicon before larger public frequency corpora are integrated.

`phrases.tsv` format:

```text
phrase<TAB>weight<TAB>tags
```

The release script infers each phrase reading from single-character readings in the bootstrap KeyKey database, then inserts or replaces the unigram with the supplied weight.

`explicit.tsv` format:

```text
qstring<TAB>phrase<TAB>weight<TAB>tags
```

Use `explicit.tsv` when the fix depends on a specific reading, tone, or KeyKey
internal qstring. These rows replace only the exact qstring/phrase pair.

License: CC0-1.0.
