#!/bin/sh
set -eu

mode=$1
report=$2
pid=$$
pgid=$(ps -o pgid= -p "$pid" | tr -d ' ')
tpgid=$(ps -o tpgid= -p "$pid" | tr -d ' ')
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
