---
id: 10
title: Optymalizacja kroku symulacji
summary: Krok ×0,23, compute_forces ×0,14 względem punktu wyjścia (9 etapów)
status: done
priority: soon
branch: main
created: 2026-11-20T18:45:15
worked: 2026-11-20T18:45:15
links:
- kind: report
  target: .artifacts/reports/perf-report.md
  at: 2026-11-20T18:45:15
  note: etapy optymalizacji
- kind: data
  target: .artifacts/perf-stages.json
  at: 2026-11-20T18:45:15
  note: manifest etapów (pliki z .artifacts/bench)
---
Wynik: płaska siatka, siły z danych siatki, tablica sąsiedztwa, wszystkie rdzenie, SIMD f32x8, stałe SIMD jako const, kopia siatki w jednym przejściu, przesunięcie przebiegów przez szew. Szczegóły i tabele w raporcie.
