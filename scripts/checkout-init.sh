#!/bin/bash

set -xe

target="$1"
shift 1

st="$(git blame Cargo.lock \
  | sort -h \
  | sed -rne 's/^([0-9a-f]+).*/\1/p' \
  | uniq -c \
  | sort -h \
  | tail -n1 \
  | sed -re 's/ *[0-9]+ *([0-9a-f]+).*/\1/')"

git="$(git rev-parse --git-dir)"
tmp="$(realpath "$git/tmp")"
git clone "$git" "$tmp"

function cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

pushd "$tmp"

git bisect start HEAD "$st"
git bisect run sh -c '[ ! -d crates ] || ! grep -q profile\.docker Cargo.toml'

commit="$(git rev-parse --short refs/bisect/bad)"

popd

echo "Selected commit: $commit"

[[ -d "$target" ]] || mkdir "$target"
git archive "$commit:./" | tar -C "$target" -x
