#!/usr/bin/env bash

dir="$(realpath "$1")"
exe="$(realpath "$2")"
shift 2

set -eo pipefail
cd "$(dirname "$0")"

tmin="$dir/../../tmin"
cmin="$dir/../../cmin"

for d in "$dir"/*; do
  out="$tmin/$(basename "$d")"
  mkdir -p "$out"

  for f in "$d"/crashes/id:*; do
    cargo afl tmin -i"$f" -o"$out/$(basename "$f")" -- "$exe"
  done
done

cargo afl cmin -i"$tmin" -o"$cmin" -C -- "$exe"
