# chiakey-fragment-denylist

Self-curated weight caps for non-lexical libchewing phrases that are sentence
fragments (auxiliary/modal + verb collocations such as `會比`, `會在`, `想用`),
not standalone words. Because the KeyKey walker maximizes a sum of node
log-probabilities, an over-weighted 2-syllable fragment can win a 2-node parse
over the correct 3-node one by stealing a syllable from a stronger straddling
word — e.g. `會比`(會 + 比) outranking `會 | 比較 | 準` on input `會比較準`.

## What this caps

`fragment-demotions.tsv` — `phrase <TAB> max_weight <TAB> tags`. Applied as a
final phrase-level cap (`apply_variant_demotions`): any unigram row for the
phrase stronger than `max_weight` is lowered to it. The cap is the safe bound
`w(lead_char) + w(stolen_word) − 0.30`, the least aggressive demotion that lets
the correct `lead | stolen_word | …` parse win. Standalone typing is unaffected:
the same characters still render via the character split.

## How the list was built

1. Structural filter (offline): 2-char libchewing phrases whose tail character
   heads a stronger straddling word (same-character, not just same-reading).
2. Scoped to a leading auxiliary/modal set (`會能要想該可肯願敢應須必得`); the
   broad unscoped rule over-matches real words and was rejected.
3. Classified against the MOE *重編國語辭典修訂本* headword set — entries absent
   from the dictionary are fragment candidates. The dictionary was used only as
   an **offline build tool**; none of its text is redistributed here.
4. Manual review of the residual (~64 rows): real words the dictionary happened
   to omit (e.g. `可愛`) were kept out of the list.

Neither frequency nor structure alone separates fragments from real words; the
combination (structure → dictionary → human spot-check) is what produced a clean
set. See the dictionary licence note: MOE text is CC BY-ND 3.0 TW (no-derivatives
applies to the text, not to downstream use); only this self-authored list ships.
