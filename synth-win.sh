#!/usr/bin/env bash
set -e

SF2=${SF2:-/usr/share/sounds/sf2/FluidR3_GM.sf2}
MIDI_PORT=${MIDI_PORT:-loopMIDI Port}
WIN_TARGET_DIR="$(cmd.exe /c echo %LOCALAPPDATA% 2>/dev/null | tr -d '\r')\\mtrak\\target"

cargo.exe build --target-dir "$WIN_TARGET_DIR"

if command -v fluidsynth.exe >/dev/null; then
    fluidsynth.exe -m winmidi -o "midi.winmidi.device=$MIDI_PORT" -i "$(wslpath -w "$SF2")" >/dev/null 2>&1 &
    trap 'taskkill.exe /IM fluidsynth.exe /F >/dev/null 2>&1' EXIT
fi

CMD="\$env:COLORTERM='${COLORTERM:-truecolor}'; Start-Process -Wait -FilePath '$WIN_TARGET_DIR\\debug\\mtrak.exe' -WorkingDirectory '$(wslpath -w "$PWD")'"
if [ $# -gt 0 ]; then
    CMD+=" -ArgumentList $(printf "'%s'," "$@" | sed 's/,$//')"
fi
powershell.exe -NoProfile -Command "$CMD"
