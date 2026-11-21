---
id: 51
title: Odchudzenie bench common
summary: benches/common tylko dla benchu gpu; własne argumenty gpu
status: todo
priority: soon
branch: compute-shader-1
part_of: 50
created: 2026-11-21T15:00:57
---
Cel: usunąć z benches/common resztki benchów CPU (--n, --offsets, --matrices, Sample, Progress, format_table, quantile, QUICK_N, opis QUICK_SEEDS pod CPU) i dać benchowi gpu spójne argumenty (--sizes, --steps, --batch, --quick, warianty). Bez zmiany formatu wyników (A/B i bench-viz działają dalej). Następny krok: lista użyć w benches/gpu/main.rs.
