# Chiaki Modern Overlay Phrases

This source contains small project-owned phrase overrides used to fix obvious misses in the seed lexicon before larger public frequency corpora are integrated.

Format:

```text
phrase<TAB>weight<TAB>tags
```

The release script infers each phrase reading from single-character readings in the bootstrap KeyKey database, then inserts or replaces the unigram with the supplied weight.

License: CC0-1.0.

