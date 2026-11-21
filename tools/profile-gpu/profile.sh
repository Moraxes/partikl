#!/usr/bin/env bash
# Nagranie liczników GPU (Instruments: Metal System Trace + Metal GPU Counters) dla benchu GPU,
# bez okna. xctrace potrzebuje uprawnień administratora do zbierania liczników ("Could not set the
# recording priority" bez sudo), więc:
#
#   sudo tools/profile-gpu/profile.sh [argumenty benchu gpu…]
#
# Domyślnie: --sizes 500000 --scenes uniform --measured 40 --no-gpu-timing (jeden przebieg compute
# na klatkę, jak w aplikacji). Wynik: /tmp/gpu-counters.trace (czytelny dla wszystkich); podsumowanie:
#   python3 tools/profile-gpu/counters.py /tmp/gpu-counters.trace
#
# Binarkę benchu buduje się wcześniej bez sudo (`cargo bench --bench gpu --no-run`); skrypt bierze
# najnowszą. Bench działa jako root, więc wyniki idą do /tmp, nie do repo.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}
bin=$(ls -t "$root"/target/release/deps/gpu-* | grep -v '\.d$' | head -1)
out=${TRACE_OUT:-/tmp/gpu-counters.trace}
rm -rf "$out" /tmp/gpu-counters-out
args=("$@")
[ ${#args[@]} -gt 0 ] || args=(--sizes 500000 --scenes uniform --measured 40 --no-gpu-timing)
xcrun xctrace record --template "$root/tools/profile-gpu/metal-gpu-counters.tracetemplate" \
  --output "$out" --time-limit 30s --no-prompt \
  --launch -- "$bin" "${args[@]}" --out-dir /tmp/gpu-counters-out
chmod -R a+rX "$out" /tmp/gpu-counters-out 2>/dev/null || true
echo "ślad: $out"
