---
id: opt-004
title: Spike kafelki Mortona
summary: 'Spike: kafelkowy indeks komórek (sonnet)'
status: todo
priority: later
branch: main
part_of: opt-001
blocked_by:
- opt-003
created: 2026-11-22T01:12:30
worked: 2026-11-22T13:04:09
links:
- kind: commit
  target: 800810f
  at: 2026-11-22T13:04:09
  note: 'data(bench): opt-004 tiled cell index A/B (quick x3, 500k/1M)'
- kind: data
  target: bench/gpu-2026-11-22-13-03-44.parquet
  at: 2026-11-22T13:04:09
- kind: commit
  target: f50e533
  at: 2026-11-22T13:04:09
- kind: data
  target: bench/gpu-2026-11-22-13-03-53.parquet
  at: 2026-11-22T13:04:17
---
Model: sonnet (indeksowanie komórek i dzielenie biegów, ocena wyniku pomiaru).

Spike z bramką, spodziewany wynik zerowy albo ujemny: pętla sił czyta wiersz sąsiedztwa jako jeden ciągły bieg komórek (`cell_offsets[first]..[last]`), co działa tylko przy indeksie wierszowym; z kafelkami wiersz rozpada się na kilka biegów, więc rośnie sterowanie pętlą, a profil 061 pokazał pętlę ograniczoną instrukcjami (L1 2,6%).

Zakres (po opt przycinanie, bo oba zmieniają `run_at`):
- Kolejność komórek w kafelkach 8×8 (nie Morton na potęgach 2: wymiary siatki nie są potęgami, dopełnienie zepsułoby torus). types.wesl `cell_index`: indeks kafelkowy z węższym ostatnim kafelkiem w wierszu i niższym ostatnim pasem.
- `run_at` zwraca do 3 części (granica kafelka + szew torusa), pętle iterują po `seg < 3`.
- Bramka: siły ≥5% szybciej przy 100k i 1M, bez regresji clusters/dense. Bez zysku: zamknąć zadanie (tq drop) z wynikiem, nic nie commitować w main (zmiany w osobnej gałęzi/worktree).
