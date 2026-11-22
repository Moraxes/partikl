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
edited: 2026-11-22T13:04:18
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
- kind: data
  target: bench/gpu-2026-11-22-13-02-40.parquet
  at: 2026-11-22T13:04:18
- kind: data
  target: bench/gpu-2026-11-22-13-02-49.parquet
  at: 2026-11-22T13:04:18
---
Model: sonnet (indeksowanie komórek i dzielenie biegów, ocena wyniku pomiaru).

Spike z bramką, spodziewany wynik zerowy albo ujemny: pętla sił czyta wiersz sąsiedztwa jako jeden ciągły bieg komórek (`cell_offsets[first]..[last]`), co działa tylko przy indeksie wierszowym; z kafelkami wiersz rozpada się na kilka biegów, więc rośnie sterowanie pętlą, a profil 061 pokazał pętlę ograniczoną instrukcjami (L1 2,6%).

Zakres (po opt przycinanie, bo oba zmieniają `run_at`):
- Kolejność komórek w kafelkach 8×8 (nie Morton na potęgach 2: wymiary siatki nie są potęgami, dopełnienie zepsułoby torus). types.wesl `cell_index`: indeks kafelkowy z węższym ostatnim kafelkiem w wierszu i niższym ostatnim pasem.
- `run_at` zwraca do 3 części (granica kafelka + szew torusa), pętle iterują po `seg < 3`.
- Bramka: siły ≥5% szybciej przy 100k i 1M, bez regresji clusters/dense. Bez zysku: zamknąć zadanie (tq drop) z wynikiem, nic nie commitować w main (zmiany w osobnej gałęzi/worktree).

## Wynik (2026-11-22): brak zysku, zadanie porzucone

Spike: indeks kafelkowy 8x8 (types.wesl `cell_linear`, węższy ostatni kafelek/niższy ostatni pas, bez dopełnienia) + `run_at` z do 3 częściami (szew torusa + granica kafelka), pętle `seg < 3`. Na wersji bez przycinania (opt-003 porzucone). Testy: cargo test 80/80 zielone. Gałąź `spike/opt-004`, commit f50e533 (worktree ../partikl-opt004), nie mergowana.

Pomiar (M5): quick 100k, ab.sh 3 rundy, mediany po/przed, siły: uniform x1.001, clusters x0.993, dense x0.982 (GPU krok 1.000/0.989/0.986; scatter i suma pref. bez zmian). Krok zegarowy 100k wyszedł gorszy (x1.32/1.18/1.13), przy równym czasie GPU: niezbadane, nieistotne wobec braku zysku w siłach.
Duże n, jeden przebieg (siły µs, przed -> po): 500k uniform 475.5->469.0, clusters 323.4->313.9, dense 297.2->291.1; 1M uniform 1460.4->1448.0 (-0.8%), clusters 867.7->846.1 (-2.5%), dense 777.6->741.1 (-4.7%). GPU 1M: 2141->2132, 1595->1559, 1509->1463.
Bramka (>=5% siły przy 100k i 1M): niespełniona, przy 100k zysk <=2%. Zgodnie z oczekiwaniem: dodatkowe sterowanie podziału biegów zjada zysk z lokalności. Dane: .artifacts/bench (13-02-40..13-03-13 ab, 13-03-44 przed 1M, 13-03-53 po 1M).
