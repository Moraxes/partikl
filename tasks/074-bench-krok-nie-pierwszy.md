---
id: 74
title: 'Bench: krok nie-pierwszy'
status: todo
priority: later
branch: main
created: 2026-11-22T01:36:52
---
Bench gpu w scenach uniform/clusters/dense wywołuje set_state przed każdym pomiarem, więc czasy GPU pochodzą z kroku recount (po opt-002 pokazują count 17–22 µs i grid gorszy o ok. 2%). Zmierzyć krok stacjonarny (np. rozgrzewka 1 kroku po set_state). Przy okazji: tools/bench-ab/ab.sh na bashu 3 (macOS) wywala 'bench_args[@]: unbound variable' bez dodatkowych argumentów.
