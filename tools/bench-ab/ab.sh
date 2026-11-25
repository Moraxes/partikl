#!/usr/bin/env bash
# Porównanie przed/po w trybie `--quick`: kod z HEAD („przed”) i drzewo robocze („po”),
# uruchamiane na przemian, żeby dryf warunków (temperatura, obciążenie) rozłożył się na obie
# strony. Wyniki lądują w `.artifacts/bench/` jak zwykłe przebiegi; na końcu porównanie per
# macierz (`compare.js`) i linia JSON z plikami do wpisania w `.artifacts/perf-stages.json`
# (`quick[].rounds`).
#
#   tools/bench-ab/ab.sh [rundy=3] [dodatkowe argumenty benchu…]
#
# `AB_BASE=<rewizja>`: „przed” z tej rewizji zamiast HEAD (np. porównanie kilku commitów naraz).
# `AB_BENCH=<nazwa>`: inny bench niż `gpu` (domyślny i jedyny po usunięciu benchów CPU).
#
# `--state` (dla `gpu`): przed pomiarem kod „przed” generuje stany sceny `evolved` po `--steps`
# krokach do `.cache/ab/states`, a obie strony ładują te same stany przed każdym powtórzeniem.
# Pozwala porównywać kod na realistycznych skupiskach także przy zmianie fizyki (zmian samej
# dynamiki ten tryb nie ocenia).
# Wynik per liczba cząstek i wariant opcji.
#
# „Przed” budowane w osobnym worktree `.cache/ab/worktree` z własnym katalogiem `target`
# (wspólny `target` z innym worktree myli cargo: biblioteka z innej ścieżki uchodzi za
# aktualną). Pierwsze budowanie trwa kilka minut, kolejne tylko `partikl`.
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
rounds=${1:-3}
shift || true
base="$root/.cache/ab/worktree"
target="$root/.cache/ab/target"
out="$root/.artifacts/bench"
rev=$(git rev-parse "${AB_BASE:-HEAD}")
bench=${AB_BENCH:-gpu}

if [ ! -d "$base" ]; then
  git worktree add -q --detach "$base" "$rev"
else
  git -C "$base" checkout -q --detach "$rev"
fi

bench_bin() { # katalog, katalog target
  (cd "$1" && CARGO_TARGET_DIR="$2" cargo bench --bench "$bench" --no-run 2>&1) \
    | grep -o "([^)]*/deps/$bench-[0-9a-f]*)" | tr -d '()' | tail -1
}
before=$(bench_bin "$base" "$target")
after=$(bench_bin "$root" "$root/target")
[ -n "$before" ] && [ -n "$after" ] || { echo "budowanie nie powiodło się" >&2; exit 1; }

# Ostatni plik przebiegu w katalogu wyników (nazwy z czasem startu rosną).
latest() { ls "$out"/"$bench"-*.parquet | sort | tail -1 | xargs basename; }

bench_args=("$@")
run_args=("$@")

has_state=0
has_state_dir=0
has_regenerate=0
for arg in ${bench_args[@]+"${bench_args[@]}"}; do
  case "$arg" in
    --state) has_state=1 ;;
    --state-dir|--state-dir=*) has_state_dir=1 ;;
    --regenerate) has_regenerate=1 ;;
  esac
done

if [ "$bench" = "gpu" ] && [ "$has_state" -eq 1 ]; then
  states="$root/.cache/ab/states"
  gen_args=()
  run_args=()
  for arg in ${bench_args[@]+"${bench_args[@]}"}; do
    gen_args+=("$arg")
    if [ "$arg" != "--regenerate" ]; then
      run_args+=("$arg")
    fi
  done
  gen_args+=(--generate-states)
  if [ "$has_regenerate" -eq 0 ]; then
    gen_args+=(--regenerate)
  fi
  if [ "$has_state_dir" -eq 0 ]; then
    gen_args+=(--state-dir "$states")
    run_args+=(--state-dir "$states")
  fi
  (cd "$base" && "$before" --quick "${gen_args[@]}") \
    || { echo "„przed” ($(git rev-parse --short "$rev")) nie generuje stanów: --state wymaga rewizji z zapisem stanu (053)" >&2; exit 1; }
  echo "stany z $(git rev-parse --short "$rev") w $states" >&2
fi

json="["
for ((r = 1; r <= rounds; r++)); do
  # Kolejność w rundzie na zmianę, żeby żadna strona nie szła stale pierwsza.
  if ((r % 2)); then order="before after"; else order="after before"; fi
  for side in $order; do
    if [ "$side" = before ]; then
      (cd "$base" && "$before" --quick --out-dir "$out" ${run_args[@]+"${run_args[@]}"} >/dev/null 2>&1)
      b=$(latest)
    else
      "$after" --quick --out-dir "$out" ${run_args[@]+"${run_args[@]}"} >/dev/null 2>&1
      a=$(latest)
    fi
    sleep 1 # osobne sekundy w nazwach plików
  done
  echo "runda $r: przed $b, po $a" >&2
  json+="{\"before\":\"$b\",\"after\":\"$a\"},"
done
json="${json%,}]"

node "$root/tools/bench-ab/compare.js" "$json"
echo "$json"
