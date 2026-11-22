---
id: opt-007
title: Sub-stepping sterowanie
summary: 'Sub-stepping: klawisz, flaga, HUD (haiku)'
status: doing
priority: later
branch: main
part_of: opt-001
blocked_by:
- opt-006
created: 2026-11-22T01:12:31
worked: 2026-11-22T02:02:23
links:
- kind: commit
  target: dea7273
  at: 2026-11-22T02:02:23
  note: 'feat(gpu): substeps key S and --substeps flag'
---
Model: haiku (klawisz + flaga + HUD według istniejącego wzorca).

Cel: sterowanie sub-steppingiem (silnik: opt sub-stepping silnik, zasób `Substeps(u32)`).

Kroki:
1. Klawisz S (sprawdź kolizje z src/ui/mod.rs i kluczami opcji reguł `//@option ... key=`) przełącza cyklicznie 1 → 2 → 4 → 1. Wzór: obsługa klawisza B/A/H/F w src/ui/mod.rs.
2. Linia w HUD z bieżącym K (np. 'substeps 2'), pokazywana tylko dla K > 1.
3. Flaga startowa `--substeps <K>` w src/args.rs oraz w benchu gpu (benches/gpu/main.rs); wartości spoza {1,2,4} odrzucać komunikatem (wzór walidacji innych flag).
4. Test UI jakościowy w src/ui/tests.rs (klawisz zmienia zasób; bez asercji na wartości analogowe).
5. Pomiar: bench gpu --quick --substeps 1/2/4, liczby w tym zadaniu.
