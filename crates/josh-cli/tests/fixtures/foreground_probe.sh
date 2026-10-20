#!/bin/sh
set -eu

mode=$1
report=$2
pid=$$
# Query the controlling terminal directly: ps may be unavailable or denied
# process-list access even when terminal process-group control works.
groups=$(python3 -c 'import os; fd = os.open("/dev/tty", os.O_RDONLY); print(os.getpgrp(), os.tcgetpgrp(fd))')
pgid=${groups%% *}
tpgid=${groups#* }
printf 'pid=%s pgid=%s tpgid=%s\n' "$pid" "$pgid" "$tpgid" > "$report"

case "$mode" in
  producer)
    printf 'pipeline-payload\n'
    ;;
  consumer)
    cat
    ;;
  interrupt)
    resize_report=$3
    trap 'printf resized > "$resize_report"' WINCH
    trap 'exit 130' INT
    printf ready > "$resize_report.ready"
    while :; do
      sleep 1
    done
    ;;
  stop)
    kill -TSTP "$$"
    printf 'continued-unexpectedly\n' >> "$report"
    ;;
  dirty-terminal)
    stty -echo -icanon
    ;;
  terminal-modes)
    stty -a
    ;;
  *)
    printf 'unknown probe mode: %s\n' "$mode" >&2
    exit 64
    ;;
esac
