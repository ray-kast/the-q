#!/usr/bin/env bash

set -eo pipefail
cd "$(dirname "$0")"

echo "Spawning worker 1"
tmux new-session -d -s afl -n afl-main ./afl.sh fuzz "$@" -M0-main

scheds=(exploit coe rare explore)

for (( i=1; i * 2 < "$(nproc)"; i++ )); do
  sleep 1

  sched="${scheds[$(( (i - 1) % ${#scheds[@]} ))]}"
  echo "Spawning worker $(( i + 1 )) using $sched"

  tmux new-window -t afl: -- ./afl.sh fuzz "$@" -S"$i"-"$sched" -p"$sched" -b"$(( i * 2 ))"
  tmux rename-window -t afl:"$i" afl-"$i"
done
