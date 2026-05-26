#!/usr/bin/env bash
set -euo pipefail

unset FORCE_COLOR
unset NO_COLOR
exec env -u FORCE_COLOR -u NO_COLOR playwright "$@"
