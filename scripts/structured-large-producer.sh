#!/usr/bin/env bash
set -euo pipefail

JOSH=${1:-target/debug/josh}
TIMEOUT=${TIMEOUT:-$(command -v timeout || command -v gtimeout)}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
pid_file=$tmp/producer.pid
completed=$tmp/completed
script=$tmp/probe.josh

cat >"$script" <<EOF
values = \$(python3 -u -c 'import os,signal,sys; signal.signal(signal.SIGPIPE, signal.SIG_DFL); open(sys.argv[1], "w").write(str(os.getpid())); [print(i) for i in range(10000000)]; open(sys.argv[2], "w").write("done")' '$pid_file' '$completed' | lines | filter (line => int(line) >= 0) | take 5)
printf (values.length)
EOF

if ! output=$($TIMEOUT 5 "$JOSH" "$script" 2>"$tmp/stderr"); then
    cat "$tmp/stderr" >&2
    exit 1
fi
[[ $output == 5 ]]
[[ ! -e $completed ]]
pid=$(cat "$pid_file")
if kill -0 "$pid" 2>/dev/null; then
    echo "producer $pid survived early downstream close" >&2
    exit 1
fi

echo "structured-large-producer: 5 values, producer stopped before completion, pid $pid reaped"
