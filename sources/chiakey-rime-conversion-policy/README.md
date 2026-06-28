# ChiaKey Rime Conversion Policy

Project-owned conversion rules for Rime essay phrases before they are used as
ChiaKey supplemental vocabulary or rerank evidence.

Rime essay is a broad phrase and language-model source, but some entries use
phrase shapes that do not match the default modern Taiwan Traditional Chinese
lexicon. This layer keeps the frequency evidence while moving it onto the
preferred output form. For example, Rime's `喫壞` evidence is imported and
reranked as `吃壞`, and `爲啥` evidence is used as `為啥`.

`replacements.tsv` format:

```text
from<TAB>to<TAB>tags
```

Rules are applied to Rime phrase text only. Both sides must have the same
character count so the inferred qstring stays aligned with the phrase.

License: CC0-1.0.
