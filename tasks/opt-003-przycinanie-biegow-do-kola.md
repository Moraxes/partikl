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
---
Model: sonnet (matematyka zakresów komórek i szew torusa w shaderze, wpływ na próbkowanie).

Cel: dziś każdy wiersz sąsiedztwa to pełne 5 komórek. Monte Carlo (komórka R/2, REACH 2): przycięcie zakresu do koła o promieniu R wokół dokładnej pozycji cząstki daje średnio 25 → 21,6 komórki (−14% kandydatów); przy REACH 3 (komórki R/3, 7 wierszy) −27% względem dziś (pole obszaru / pole koła: 1,72 dla REACH 2 przycięte, 1,46 dla REACH 3, 1,34 dla REACH 4).

Zakres:
- step_driver.wesl `run_at(me_pos, cell, dy, seg)`: pionowa luka do wiersza `d = max(0, row_lo − y, y − row_hi)`; gdy `d ≥ R` bieg jest pusty; `hx = sqrt(R² − d²)`, `lo/hi = floor((x ∓ hx)/cs.x)` przycięte do `cell.x ± REACH`; szew torusa jak dziś. Obie pętle (liczenie kandydatów i właściwa) wołają tę samą funkcję, więc próbkowanie `every`/`k` pozostaje spójne.
- Wariant drugi: REACH 3 z przycinaniem (`REACH` w shaderze i rozmiar komórki w `GpuSim::geometry`). Mierzyć oba warianty; wcześniej (054) samo GRID_REACH 3 bez przycinania wypadło gorzej (więcej biegów, nie mniej pracy), teraz może się opłacać.
- Zysk czasu tam, gdzie kandydatów < 512 (100k); powyżej progu praca ta sama, ale mniej próbek pada poza zasięg, więc spada wariancja sił (patrz opt próg próbkowania).
- Przycinanie nie zmienia wyniku bez próbkowania: test wyroczni f64 przechodzi bez zmiany tolerancji.

Weryfikacja: testy GPU, bench gpu --quick uniform/clusters/dense, --sizes 500000,1000000; raport przy 3 i 6 typach (--types).
