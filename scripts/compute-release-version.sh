#!/usr/bin/env bash
set -euo pipefail

requested="${1:-}"
if [[ -n "$requested" ]]; then
  printf '%s\n' "$requested"
  exit 0
fi

prefix="$(TZ=Asia/Taipei date +'%Y.%m')"
last="$(
  git tag --list "${prefix}.*" |
    awk -F. -v prefix="$prefix" '
      BEGIN { max = 0 }
      ($1 "." $2) == prefix && $3 ~ /^[0-9]+$/ {
        if (($3 + 0) > max) {
          max = $3 + 0
        }
      }
      END {
        if (max > 0) {
          print max
        }
      }
    '
)"

if [[ -z "$last" ]]; then
  printf '%s.1\n' "$prefix"
else
  printf '%s.%s\n' "$prefix" "$((last + 1))"
fi
