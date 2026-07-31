#!/usr/bin/env bash
# 立法院公報詢答語料的一鍵管線：抓取 → 萃取 → bigram 統計。
#
# 用法：
#   scripts/corpus/build-ly-corpus.sh                 # 全量近 2 屆（數小時，可中斷續傳）
#   LIMIT=50 scripts/corpus/build-ly-corpus.sh        # 試跑 50 份，約 2 分鐘
#   TERMS=11 LIMIT=200 scripts/corpus/build-ly-corpus.sh
#
# 中斷後直接重跑即可，已下載的檔案會跳過。

set -euo pipefail

cd "$(dirname "$0")/../.."

TERMS="${TERMS:-10,11}"
LIMIT="${LIMIT:-0}"
WORK="${WORK:-tmp/ly-gazette}"
CORPUS="${CORPUS:-tmp/ly-speech.txt}"
OUT="${OUT:-tmp/ly-bigrams.tsv}"
MIN_COUNT="${MIN_COUNT:-3}"
MIN_DOC_COUNT="${MIN_DOC_COUNT:-2}"
LEXICON="${LEXICON:-normalized/smart-mandarin.tsv}"

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

step "1/3 抓取公報純文字（屆 ${TERMS}${LIMIT:+，上限 ${LIMIT}}）"
node scripts/corpus/fetch-ly-gazette.mjs --terms "$TERMS" --out "$WORK" --limit "$LIMIT"

step "2/3 萃取詢答語料"
node scripts/corpus/extract-ly-speech.mjs --input "$WORK/docs" --output "$CORPUS"

if [ ! -f "$LEXICON" ]; then
  step "跳過 3/3"
  echo "找不到 ${LEXICON}。這是 build 產生的檔案，先跑一次 release build 再執行："
  echo "  cargo run --release -- prepare-release"
  echo "語料已經備妥在 ${CORPUS}，之後可單獨執行："
  echo "  cargo run --release -- build-bigram-stats --input ${CORPUS} --lexicon ${LEXICON} \\"
  echo "    --document-boundary line --min-count ${MIN_COUNT} --min-doc-count ${MIN_DOC_COUNT} \\"
  echo "    --output ${OUT} --stats ${OUT%.tsv}-stats.tsv --review ${OUT%.tsv}-review.tsv"
  exit 0
fi

step "3/3 統計 bigram 候選"
cargo run --release -- build-bigram-stats \
  --input "$CORPUS" \
  --lexicon "$LEXICON" \
  --document-boundary line \
  --min-count "$MIN_COUNT" \
  --min-doc-count "$MIN_DOC_COUNT" \
  --output "$OUT" \
  --stats "${OUT%.tsv}-stats.tsv" \
  --review "${OUT%.tsv}-review.tsv"

step "完成"
cat <<EOF
語料      ${CORPUS}
抽樣      ${CORPUS%.txt}-sample.txt   ← 先看這個，確認語域符合預期
萃取統計  ${CORPUS%.txt}-stats.json
bigram    ${OUT}（$(wc -l < "$OUT" | tr -d ' ') 行）
人工複核  ${OUT%.tsv}-review.tsv

還沒有動到 sources/。確認品質後再決定要不要新增 source。
EOF
