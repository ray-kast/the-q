#!/usr/bin/env bash

set -eo pipefail
cd "$(dirname "$0")"

AFL_PKG=''
AFL_BIN=''
AFL_PROFILE='default'

afl::build() {
  cargo afl build --manifest-path="$AFL_PKG/Cargo.toml" --bin="$AFL_BIN" "$@"
}

afl::fuzz() {
  local in="$1" out="$2" profile="$3"
  shift 3

  cargo afl fuzz -i"$in" -o"$out" "$@" -- "$(file::bin "$profile")"
}

afl::run() {
  local case="$1"
  shift 1

  cargo afl run --manifest-path="$AFL_PKG/Cargo.toml" --bin="$AFL_BIN" <"$case"
}

afl::tmin() {
  local in="$1" out="$2" profile="$3"
  shift 3

  cargo afl tmin -i"$in" -o"$out" "$(file::bin "$profile")"
}

afl::cmin() {
  local in="$1" out="$2" profile="$3"
  shift 3

  cargo afl cmin -i"$in" -o"$out" -Tall "$(file::bin "$profile")"
}

dir::in() { echo -n "$AFL_PKG/in/$AFL_BIN"; }
dir::out() { echo -n "$AFL_PKG/out/$AFL_BIN"; }
dir::crashes() { echo -n "$(dir::out)/$AFL_PROFILE/crashes"; }
dir::min() { echo -n "$AFL_PKG/min/$AFL_BIN"; }

file::bin() { echo -n "$AFL_PKG/target/$1/$AFL_BIN"; }

USAGE_CMD=''
usage() {
  local desc="$1"
  shift 1

  echo "Usage: afl.sh${USAGE_CMD:+ $USAGE_CMD} $desc"
  exit 255
}

main::usage() {
  usage '<command> [args...]
Valid commands: clean, fuzz, seed, run'
}

main() {
  local cmd="$1"
  shift 1 || main::usage

  USAGE_CMD="$cmd"

  case "$cmd" in
    clean) clean "$@" ;;
    fuzz) fuzz "$@" ;;
    seed) seed "$@" ;;
    *) main::usage ;;
  esac
}

clean::usage() {
  usage '<package> <binary>'
}

clean() {
  AFL_PKG="$1"
  AFL_BIN="$2"
  shift 2 || clean::usage

  rm -rf "$(dir::out)" "$(dir::min)"
}

fuzz::usage() {
  usage '[-Cd] <package> <binary>'
}

fuzz() {
  local profile='release'
  local target_dir='release'
  local crash_explore=0

  while getopts Cd opt; do
    case "$opt" in
      C) crash_explore=1 ;;
      d)
        profile="dev"
        target_dir="debug"
        ;;
      *) fuzz::usage ;;
    esac
  done

  shift $(( OPTIND - 1 ))

  AFL_PKG="$1"
  AFL_BIN="$2"
  shift 2 || fuzz::usage

  local afl_flags=() in_dir out_dir
  in_dir="$(dir::in)"
  out_dir="$(dir::out)"

  set -x

  if (( crash_explore )); then
    afl_flags+=('-C')
    in_dir="$(dir::crashes)"
    out_dir="$(dir::min)"
  fi

  mkdir -p "$out_dir"

  afl::build --profile="$profile"
  afl::fuzz "$in_dir" "$out_dir" "$target_dir" "${afl_flags[@]}" "$@"
}

seed::usage() {
  usage '<package> <binary>'
}

seed() {
  AFL_PKG="$1"
  AFL_BIN="$2"
  shift 2 || seed::usage

  afl::build

  local stage tmp tmp2 len try found id

  mkdir -p "$(dir::out)"
  stage="$(mktemp -d "$(dir::out)"/seed.XXXX.d)"
  tmp="$(mktemp "$(dir::out)"/seed.XXXX.bin)"
  tmp2="$(mktemp "$(dir::out)"/seed.XXXX.bin)"

  for len in {0..10}; do
    echo -n "Trying cases of length 2^$len"
    found=0

    for try in {0..1024}; do
      echo -n '.'

      head -c"$(( 1 << len ))" /dev/urandom >"$tmp"
      if afl::tmin "$tmp" "$tmp2" debug >/dev/null 2>/dev/null
      then
        id=0
        while [[ -e "$stage/$id.bin" ]]; do id="$(( id + 1 ))"; done
        cp "$tmp2" "$stage/$id.bin"

        found="$(( found + 1 ))"
        if (( found >= 8 )); then break; fi
      else
        echo -n 'x'
      fi
    done

    echo
  done

  rm -rf "$tmp" "$tmp2"
  afl::cmin "$stage" "$(dir::in)" debug
  rm -rf "$stage"
}

main "$@"
