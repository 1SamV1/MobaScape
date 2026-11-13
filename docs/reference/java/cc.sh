#!/usr/bin/env bash
# MOBA dev: send lines to the client's TCP dev console.
#   dev/cc.sh cam
#   dev/cc.sh cam 2860 3060
#   dev/cc.sh zoom 1.3
set -eu
if [ "$#" -gt 0 ]; then
  printf '%s\n' "$*" | timeout 5 socat - TCP:127.0.0.1:43597
else
  timeout 5 socat - TCP:127.0.0.1:43597
fi
