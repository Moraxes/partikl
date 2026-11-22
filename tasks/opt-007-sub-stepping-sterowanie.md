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
worked: 2026-11-22T02:02:26
edited: 2026-11-22T02:03:45
links:
- kind: commit
  target: dea7273
  at: 2026-11-22T02:02:23
  note: 'feat(gpu): substeps key S and --substeps flag'
- kind: commit
  target: 32ca34f
  at: 2026-11-22T02:02:26
  note: 'chore: opt-007 bench runs (substeps key)'
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-02-01-51.parquet
  at: 2026-11-22T02:03:45
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-02-01-56.parquet
  at: 2026-11-22T02:03:45
---
Model: haiku (klawisz + flaga + HUD według istniejącego wzorca).

Cel: sterowanie sub-steppingiem (silnik: opt sub-stepping silnik, zasób `Substeps(u32)`).

Kroki:
1. Klawisz S (sprawdź kolizje z src/ui/mod.rs i kluczami opcji reguł `//@option ... key=`) przełącza cyklicznie 1 → 2 → 4 → 1. Wzór: obsługa klawisza B/A/H/F w src/ui/mod.rs.
2. Linia w HUD z bieżącym K (np. 'substeps 2'), pokazywana tylko dla K > 1.
3. Flaga startowa `--substeps <K>` w src/args.rs oraz w benchu gpu (benches/gpu/main.rs); wartości spoza {1,2,4} odrzucać komunikatem (wzór walidacji innych flag).
4. Test UI jakościowy w src/ui/tests.rs (klawisz zmienia zasób; bez asercji na wartości analogowe).
5. Pomiar: bench gpu --quick --substeps 1/2/4, liczby w tym zadaniu.

## Wynik

Zrobione w dea7273 (klawisz S, flaga, HUD, testy); pomiar w 32ca34f (`.artifacts/bench`).

Zmiany:
- `gpu::SUBSTEP_CYCLE = [1, 2, 4]` i `Substeps::next()` (wartość spoza cyklu idzie do najbliższej większej, jak `SamplingCap::next`). Klawisz S w `ui::handle_keyboard_input`; `S` dodane do `RESERVED_KEYS`, więc opcja reguł nie może wziąć tego klawisza.
- `--substeps K` w `src/args.rs`: `parse_substeps` przyjmuje tylko 1, 2, 4 (inne wartości: błąd clap). `init_gpu` ustawia `Substeps` z flagi.
- HUD: linia `substeps K  S` tylko dla K > 1 (`hud_text` dostał argument `substeps`). `update_text` ma `#[allow(clippy::too_many_arguments)]`, jak `handle_keyboard_input`.
- Bench gpu miał już `--substeps` (opt-006) i kolumnę `substeps`; nic nie dodano. Bench przyjmuje 1..=8 (`MAX_SUBSTEPS`), a nie tylko {1, 2, 4}; to świadoma różnica względem punktu 3 (zawężenie benchu do {1,2,4} nie zostało zrobione).
- Testy: `s_cycles_the_substeps`, `substeps_next_handles_values_off_the_cycle`, `hud_shows_substeps_only_above_one` (src/ui/tests.rs), `substeps_flag_accepts_only_one_two_or_four` (src/args/tests.rs). Brak asercji na wartości analogowe. `cargo test`: 77 przechodzi. `cargo clippy --all-targets`: te same 10 ostrzeżeń lib co przed zmianą (żadnych nowych).

Pomiar (M5, `cargo bench --bench gpu -- --quick --substeps K`, 100k, headless; pliki w `.artifacts/bench`):

| scena | K | krok µs (takt) | ×K=1 | GPU µs (1 krok) | siatka µs | siły µs |
|---|---|---|---|---|---|---|
| uniform | 1 | 482,9 | 1,00 | 461,8 | 84,1 | 377,2 |
| uniform | 2 | 893,5 | 1,85 | 461,2 | 84,2 | 376,8 |
| uniform | 4 | 1710,6 | 3,54 | 461,5 | 84,4 | 376,9 |
| clusters | 1 | 643,9 | 1,00 | 601,4 | 71,8 | 529,2 |
| clusters | 2 | 1211,4 | 1,88 | 602,2 | 72,2 | 529,4 |
| clusters | 4 | 2326,7 | 3,61 | 600,3 | 72,1 | 527,5 |
| dense | 1 | 672,8 | 1,00 | 631,8 | 69,2 | 562,4 |
| dense | 2 | 1269,2 | 1,89 | 632,2 | 69,0 | 563,0 |
| dense | 4 | 2449,4 | 3,64 | 632,3 | 69,2 | 562,9 |

Koszt taktu rośnie jak K (×1,9 / ×3,6 w clusters i dense, ×1,85 / ×3,5 w uniform); koszt pojedynczego kroku GPU jest stały. Zgodne z opt-006 (×1,85/×3,5, ×1,9/×3,7, ×1,9/×3,7). Bez regresji przy K = 1 (482,9 µs vs 485 µs w opt-006).

Pliki: `.artifacts/bench/gpu-2026-11-22-02-01-51.parquet` (K=1), `gpu-2026-11-22-02-01-56.parquet` (K=2), `gpu-2026-11-22-02-02-03.parquet` (K=4).
