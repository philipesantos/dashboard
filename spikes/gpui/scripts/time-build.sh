#!/usr/bin/env bash
# Times a cargo build, clean and then incremental after touching one source file.
#   time-build.sh <label> <file to touch> -- <cargo build arguments...>
# Prints: label clean_s=… incremental_s=…
set -euo pipefail
label=$1 touch_file=$2
shift 2
[ "$1" = "--" ] && shift
now() { perl -MTime::HiRes=time -e 'printf "%.3f", time'; } # BSD date has no %N
s=$(now); cargo build "$@"; e=$(now)
clean=$(awk -v s="$s" -v e="$e" 'BEGIN { print e - s }')
sleep 1; touch "$touch_file"
s=$(now); cargo build "$@"; e=$(now)
incremental=$(awk -v s="$s" -v e="$e" 'BEGIN { print e - s }')
printf '%s clean_s=%.0f incremental_s=%.1f\n' "$label" "$clean" "$incremental"
