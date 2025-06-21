#!/usr/bin/env bash

dir="$(realpath "$1")"
exe="$(realpath "$2")"
shift 2

set -eo pipefail
cd "$(dirname "$0")"

tmin="$dir/../../tmin"
cmin="$dir/../../cmin"

rm -rf -- "$tmin"

for d in "$dir"/*; do
  [[ -d "$d" ]] || continue

  out="$tmin/$(basename "$d")"
  mkdir -p "$out"

  for f in "$d"/crashes/id:*; do
    [[ -f "$f" ]] || continue

    cargo afl tmin -i"$f" -o"$out/$(basename "$f")" -- "$exe"
  done
done

rm -rf -- "$cmin"

cargo afl cmin -i"$tmin" -o"$cmin" -C -- "$exe"
