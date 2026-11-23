#!/usr/bin/env bash
# Sonda stabilności: ślad symulacji (bench render --snapshots, headless) dla macierzy opcji reguł.
#
#   tools/stability-probe/probe.sh [-n SIZES] [-s STEPS] [-e EVERY] [-d OUTDIR] [-p PARAMS] AXIS...
#
# AXIS to `opcja=wybór1,wybór2,...`; sonda przechodzi po iloczynie kartezjańskim osi, np.
#   tools/stability-probe/probe.sh average_forces=on,off overlap=spring,resolve,adaptive_range
# -n  rozmiary po przecinku (domyślnie 100000,1000000)    -s  kroków (2000)
# -e  odstęp zrzutów w krokach (500)                      -p  RON dla --params (np. "(repulsion_radius: 10.0)")
# -d  katalog wyników (domyślnie .cache/stability-probe/<data>); w nim <wariant>_n<N>/ (stats.csv,
#     PNG) i <wariant>_n<N>.log, a na końcu tabela summary.txt.
# Kolumny tabeli: maks. |v|, energia kinetyczna i średnia liczba kandydatów w ostatnim zrzucie oraz
# maksymalna energia w całym śladzie; NaN oznacza przerwany przebieg. Bez okna aplikacji.
set -u
cd "$(dirname "$0")/../.."

sizes=100000,1000000; steps=2000; every=500; out=""; params=""
while getopts "n:s:e:d:p:" o; do
  case $o in
    n) sizes=$OPTARG ;; s) steps=$OPTARG ;; e) every=$OPTARG ;; d) out=$OPTARG ;; p) params=$OPTARG ;;
    *) exit 2 ;;
  esac
done
shift $((OPTIND - 1))
[ $# -gt 0 ] || { sed -n 2,12p "$0"; exit 2; }
[ -n "$out" ] || out=.cache/stability-probe/$(date +%Y-%m-%d-%H-%M-%S)
mkdir -p "$out"

cargo bench --bench render --no-run -q 2>&1 | tail -3
bin=$(ls -t target/release/deps/render-* 2>/dev/null | grep -v '\.d$' | head -1)
[ -n "$bin" ] || { echo "brak binarki benchu render" >&2; exit 1; }

# Iloczyn kartezjański osi: wypisuje linie "nazwa1=a nazwa2=b ...".
product() {
  if [ $# -eq 0 ]; then echo ""; return; fi
  local axis=$1; shift
  local name=${axis%%=*} choice
  local rest; rest=$(product "$@")
  for choice in $(echo "${axis#*=}" | tr , ' '); do
    while IFS= read -r line; do echo "$name=$choice $line"; done <<<"$rest"
  done
}

summary=$out/summary.txt
printf '%-44s %9s %10s %10s %10s %8s\n' variant n v_max E_final E_max cand >"$summary"
for n in $(echo "$sizes" | tr , ' '); do
  product "$@" | while IFS= read -r combo; do
    [ -n "$combo" ] || continue
    name=$(echo "$combo" | sed 's/[a-z_]*=//g; s/ *$//; s/ /_/g')_n$n
    args=(); for kv in $combo; do args+=(--option "$kv"); done
    [ -n "$params" ] && args+=(--params "$params")
    echo "== $name"
    "$bin" --bench --snapshots "$out/$name" --sizes "$n" --gallery-steps "$steps" \
      --snapshot-every "$every" "${args[@]}" >"$out/$name.log" 2>&1 || echo "FAIL $name (patrz $out/$name.log)"
    [ -f "$out/$name/stats.csv" ] && awk -F, -v name="$name" -v n="$n" '
      NR > 1 { nan += $2; v = $3; e = $4; c = $5; if (e > emax) emax = e }
      END { printf "%-44s %9d %10.0f %10.3g %10.3g %8.0f%s\n", name, n, v, e, emax, c, nan ? "  NaN!" : "" }' \
      "$out/$name/stats.csv" >>"$summary"
  done
done
cat "$summary"
