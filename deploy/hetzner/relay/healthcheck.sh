#!/bin/sh
set -eu

progress_file=/tmp/relay-progress
[ -f "$progress_file" ] || exit 1

now=$(date +%s)
if modified=$(stat -c %Y "$progress_file" 2>/dev/null); then
    :
else
    modified=$(stat -f %m "$progress_file")
fi
age=$((now - modified))
[ "$age" -ge 0 ] && [ "$age" -lt 45 ]
