#!/bin/bash
# Starts a built Event Studio on a Mac and has it draw its windows into pictures: the editor
# and the floating timer in each language, and the floating timer's other displays. Each run
# opens the real window (placing the floating timer above everything, following the pointer,
# asking which app is in front), takes the picture and ends, so a start that fails or stops
# on the way shows up here. Nothing is saved: a run with --screenshot never writes settings.
# The mouse and the keyboard are not used.
#
# Usage: checks/verify-macos.sh <the program, e.g. "dist/macos/Event Studio.app/Contents/MacOS/EventStudio"> <folder for the pictures>
set -uo pipefail
PROGRAM=${1:?the program to check}
SHOTS=${2:?a folder for the pictures}
mkdir -p "$SHOTS"
failed=0

# Runs the program until it ends, a minute at most, and looks for its picture.
draw() {
  local name=$1
  shift
  "$PROGRAM" "$@" --screenshot "$SHOTS/$name.png" &
  local pid=$!
  for _ in $(seq 1 60); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 1
  done
  if kill -0 "$pid" 2>/dev/null; then
    kill -9 "$pid"
    echo "FAIL  $name  (still running after a minute)"
    failed=1
    return
  fi
  wait "$pid"
  local code=$?
  if [ "$code" -eq 0 ] && [ -s "$SHOTS/$name.png" ]; then
    echo "PASS  $name"
  else
    echo "FAIL  $name  (exit $code, picture $([ -s "$SHOTS/$name.png" ] && echo there || echo missing))"
    failed=1
  fi
}

for language in ko en zh ja; do
  draw "editor-$language" --lang "$language" --demo circle
  draw "floating-$language" --lang "$language" --overlay --demo circle,warning,hover
done
draw "editor-time-tab" --lang ko --demo bar,warning,timetab
draw "floating-bar" --lang ko --overlay --demo bar
draw "floating-number-over" --lang ko --overlay --demo number,over

if [ "$failed" -ne 0 ]; then
  # What the program wrote about a start that failed (report.rs).
  for log in "$HOME/Library/Application Support/Event Studio/error-log.txt" "$(dirname "$PROGRAM")/error-log.txt" "${TMPDIR:-/tmp}/EventStudio-error-log.txt"; do
    [ -f "$log" ] && { echo "--- $log"; cat "$log"; }
  done
fi
exit "$failed"
