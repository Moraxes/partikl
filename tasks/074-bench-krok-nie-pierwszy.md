---
id: 74
title: 'Bench: krok nie-pierwszy'
status: todo
priority: later
branch: main
created: 2026-11-22T01:36:52
worked: 2026-11-22T12:56:19
edited: 2026-11-22T12:56:19
links:
- kind: commit
  target: 8e32d8b
  at: 2026-11-22T12:56:19
  note: 'fix(bench): time a steady-state step and fix ab.sh on bash 3'
---
Bench gpu w scenach uniform/clusters/dense wywołuje set_state przed każdym pomiarem, więc czasy GPU pochodzą z kroku recount (po opt-002 pokazują count 17–22 µs i grid gorszy o ok. 2%). Zmierzyć krok stacjonarny (np. rozgrzewka 1 kroku po set_state). Przy okazji: tools/bench-ab/ab.sh na bashu 3 (macOS) wywala 'bench_args[@]: unbound variable' bez dodatkowych argumentów.


Rozwiązanie: po set_state jeden krok rozgrzewkowy (step(1), czas GPU odebrany i odrzucony) przed pomiarem; znaczniki GPU są tylko w pierwszym kroku klatki, więc mierzony takt jest już stanu ustalonego. Najmniej inwazyjne: zmiana tylko w benches/gpu/main.rs. ab.sh: `${bench_args[@]+"${bench_args[@]}"}` (bash 3).

Wyniki (n=100000, --quick, M5): GPU krok uniform 89,5 µs, clusters 75,8, dense 74,0; count 0,0 µs (wcześniej 22,5 µs z recountu), forces/scatter bez zmian rzędu. Poprzedni "GPU krok" ~392 µs w ab.sh był zawyżony przez recount. ab.sh 1 przechodzi na bash 3.2.57.
