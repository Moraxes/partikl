---
id: opt-003
title: Przycinanie biegów do koła
summary: Przycinanie biegów sąsiedztwa do koła (sonnet)
status: doing
priority: soon
branch: main
part_of: opt-001
created: 2026-11-22T01:12:30
worked: 2026-11-22T01:36:59
edited: 2026-11-22T01:43:34
links:
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-43-05.parquet
  at: 2026-11-22T01:43:27
- kind: data
  target: .artifacts/patches/opt-003-prune-reach2.patch
  at: 2026-11-22T01:43:27
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-42-45.parquet
  at: 2026-11-22T01:43:34
---
Model: sonnet (matematyka zakresów komórek i szew torusa w shaderze, wpływ na próbkowanie).

Cel: dziś każdy wiersz sąsiedztwa to pełne 5 komórek. Monte Carlo (komórka R/2, REACH 2): przycięcie zakresu do koła o promieniu R wokół dokładnej pozycji cząstki daje średnio 25 → 21,6 komórki (−14% kandydatów); przy REACH 3 (komórki R/3, 7 wierszy) −27% względem dziś (pole obszaru / pole koła: 1,72 dla REACH 2 przycięte, 1,46 dla REACH 3, 1,34 dla REACH 4).

Zakres:
- step_driver.wesl `run_at(me_pos, cell, dy, seg)`: pionowa luka do wiersza `d = max(0, row_lo − y, y − row_hi)`; gdy `d ≥ R` bieg jest pusty; `hx = sqrt(R² − d²)`, `lo/hi = floor((x ∓ hx)/cs.x)` przycięte do `cell.x ± REACH`; szew torusa jak dziś. Obie pętle (liczenie kandydatów i właściwa) wołają tę samą funkcję, więc próbkowanie `every`/`k` pozostaje spójne.
- Wariant drugi: REACH 3 z przycinaniem (`REACH` w shaderze i rozmiar komórki w `GpuSim::geometry`). Mierzyć oba warianty; wcześniej (054) samo GRID_REACH 3 bez przycinania wypadło gorzej (więcej biegów, nie mniej pracy), teraz może się opłacać.
- Zysk czasu tam, gdzie kandydatów < 512 (100k); powyżej progu praca ta sama, ale mniej próbek pada poza zasięg, więc spada wariancja sił (patrz opt próg próbkowania).
- Przycinanie nie zmienia wyniku bez próbkowania: test wyroczni f64 przechodzi bez zmiany tolerancji.

Weryfikacja: testy GPU, bench gpu --quick uniform/clusters/dense, --sizes 500000,1000000; raport przy 3 i 6 typach (--types).

## Wynik (sonnet, 2026-11-22): REGRESJA, kod niezacommitowany

Implementacja: `run_at(me_pos, cell, dy, seg)` w step_driver.wesl z przycinaniem do koła (luka pionowa d, hx = sqrt(R²-d²) z zapasem 1e-3, zakres przycięty do cell.x ± REACH, szew torusa jak dziś; obie pętle wołają to samo). cargo test: 67 zielonych bez zmiany tolerancji wyroczni, także z REACH 3.

Pomiar (M5, ab.sh 3 rundy, --quick 100k, GPU forces µs, przed → po):
- uniform (272 kand.): 376.7 → 398.0 (x1.056)
- clusters: 528.9 → 646.4 (x1.222)
- dense: 563.6 → 676.1 (x1.200)
Wariant REACH 3 (GRID_REACH 3 + REACH 3, komórki R/3): forces x1.050 / x1.237 / x1.227, krok GPU uniform x1.079 (siatka droższa) — gorszy od REACH 2, odrzucony.
Duże n (bez --quick, krok GPU µs, przed → po): 500k uniform 3244→3452, clusters 3227→3864, dense 3320→4031; 1M uniform 7119→8390, clusters 6945→8251, dense 9421→12568 (forces 1M dense 8402→11543, +37%).

Przyczyny: (1) koszt ALU przycinania (sqrt, floor, dzielenia na bieg, dwa razy: liczenie i pętla) przekracza zysk także poniżej progu 512 (uniform +5,6%); (2) powyżej progu przycięta liczba kandydatów jest mniejsza, więc `every` maleje i zamiast 512 próbek w dużej części poza zasięgiem odwiedzane jest ok. 512 gęstszych w zasięgu — więcej wywołań pair_interact (kosztownych), mniejsza wariancja, ale większy koszt.
Decyzja: bez commitu w main (regresji nie naprawiać bez zgody). Patch: .artifacts/patches/opt-003-prune-reach2.patch. Opcje do decyzji: zostawić jako wariant za flagą; połączyć z opt-005 (próg próbkowania obniżony tak, by stała liczba wywołań pair_interact); porzucić.
