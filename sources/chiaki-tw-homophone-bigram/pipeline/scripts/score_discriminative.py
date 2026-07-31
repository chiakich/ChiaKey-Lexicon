#!/usr/bin/env python
"""Discriminative pair scoring: does `prev` actually flip `cur` past its rival?

A bigram only earns a runtime row when, given prev, the model prefers the
contested word over the higher-weight rival that currently wins its qstring.
We score margin = logP(cur|prev) - logP(rival|prev), averaged over several
carrier sentences so a single carrier's bias cannot decide the outcome.

Input TSV: previous<TAB>current<TAB>rival[<TAB>...]
Output TSV: previous, current, rival, margin, logp_cur, logp_rival, n_carriers
"""
import argparse
import sys

import mlx.core as mx
from mlx_vlm import load
from transformers import AutoTokenizer

INSTRUCTION = "請寫一段自然的台灣繁體中文句子。"
# Several registers so the score is not an artifact of one frame.
CARRIERS = [
    "他昨天跟我說，",
    "根據現場的說明，",
    "我看了一下，發現",
    "這件事情其實是",
]


def logprob_of(model, prefix_ids, continuation_ids):
    ids = prefix_ids + continuation_ids
    output = model(mx.array(ids)[None])
    logits = getattr(output, "logits", output).astype(mx.float32)
    logprobs = logits[0] - mx.logsumexp(logits[0], axis=-1, keepdims=True)
    return sum(
        float(logprobs[position - 1, ids[position]])
        for position in range(len(prefix_ids), len(ids))
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pairs", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--carriers", type=int, default=len(CARRIERS))
    parser.add_argument("--model", default="mlx-community/gemma-4-12B-it-4bit")
    args = parser.parse_args()

    model, _processor = load(args.model)
    tokenizer = AutoTokenizer.from_pretrained(args.model)
    template = tokenizer.apply_chat_template(
        [{"role": "user", "content": INSTRUCTION}],
        add_generation_prompt=True,
        tokenize=False,
    ) + "\n"
    template_ids = tokenizer.encode(template, add_special_tokens=False)
    carriers = CARRIERS[: max(1, args.carriers)]

    rows = []
    with open(args.pairs, encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("#"):
                continue
            fields = line.rstrip("\n").split("\t")
            if len(fields) < 3 or not all(fields[:3]):
                continue
            rows.append(tuple(fields[:3]))
            if args.limit and len(rows) >= args.limit:
                break

    # prefix ids and their logP are shared by cur and rival, and reused across
    # every pair with the same prev — one third of the passes disappear.
    prefix_cache = {}
    base_cache = {}
    with open(args.out, "w", encoding="utf-8") as out:
        out.write("# previous\tcurrent\trival\tmargin\tlogp_cur\tlogp_rival\tn_carriers\n")
        for index, (prev, cur, rival) in enumerate(rows):
            cur_total = 0.0
            rival_total = 0.0
            for carrier in carriers:
                key = (carrier, prev)
                if key not in prefix_cache:
                    prefix_cache[key] = tokenizer.encode(
                        carrier + prev, add_special_tokens=False
                    )
                prefix_ids = template_ids + prefix_cache[key]
                if key not in base_cache:
                    base_cache[key] = logprob_of(model, template_ids, prefix_cache[key])
                base = base_cache[key]
                for word, accumulator in ((cur, "cur"), (rival, "rival")):
                    full = tokenizer.encode(
                        carrier + prev + word, add_special_tokens=False
                    )
                    if full[: len(prefix_cache[key])] != prefix_cache[key]:
                        # tokenizer merged across the seam; fall back to full-seq diff
                        score = logprob_of(model, template_ids, full) - base
                    else:
                        score = logprob_of(model, prefix_ids, full[len(prefix_cache[key]):])
                    if accumulator == "cur":
                        cur_total += score
                    else:
                        rival_total += score
            n = len(carriers)
            cur_mean = cur_total / n
            rival_mean = rival_total / n
            out.write(
                f"{prev}\t{cur}\t{rival}\t{cur_mean - rival_mean:.4f}"
                f"\t{cur_mean:.4f}\t{rival_mean:.4f}\t{n}\n"
            )
            if (index + 1) % 50 == 0:
                print(f"scored {index + 1}/{len(rows)}", file=sys.stderr)
                out.flush()
    print(f"done: {len(rows)} pairs", file=sys.stderr)


if __name__ == "__main__":
    main()
