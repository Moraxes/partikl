---
id: 69
title: Aktualizacja planu GPU
summary: Plan rozwoju mówi o SimUniforms i overlap_mode w uniformach; po 041 jest EngineUniforms i RuleParams, opcje to flagi
status: done
priority: later
branch: main
created: 2026-11-21T22:43:25
worked: 2026-11-22T00:43:11
edited: 2026-11-22T00:43:07
---
Cel: plan_rozwoju_partikl_na_gpu.md zgodny z kodem po refaktorze 035 (EngineUniforms/RuleParams, RuleSet, flagi wariantów, stan w kolejności siatki). Następny krok: przejść sekcje o uniformach i opcjach; zrobić po 046, żeby nie poprawiać dwa razy.

Wynik (2026-11-22): zamiast aktualizować, plan_rozwoju_partikl_na_gpu.md usunięty (w historii gita; ostatnia wersja w commicie poprzedzającym usunięcie), bo w większości zrealizowany. Co z niego zostało:
- M1c, nie ma: pass reduce_stats i StatsBuffer (energia, max prędkość, nan_count, próbki do wykresów) z Readback; panel z wykresami (egui_plot) i alert NaN z pauzą. UI jest własne (src/ui), bez egui.
- M2, nie zrobione: krzywa Mortona dla indeksu komórki; mapy ciepła i wektory sił liczone na GPU; sub-stepping jako osobny tryb mniejszego dt (kilka kroków na klatkę jest, przez StepBudget). Odrzucone pomiarem: kafelek cell-centric z var<workgroup> i dispatch indirect (060: pętla ograniczona instrukcjami, nie pamięcią), sąsiedzi 8 B w f16 (067). Tryb deterministyczny jest (sort_cells), bez radix sorta.
- M3 poziom A: zrobione częściowo (RuleParams i macierze z .ron bez rekompilacji, 042). Brak: dowolne nazwane parametry z metadanymi i suwakami w UI, wiele macierz typ×typ jako heatmapy, krzywe LUT 1D.
- M3 poziom B: nie ma ModelSpec/DSL/AST ani generatora WGSL; reguły to ręcznie pisany moduł WESL wymieniany plikiem (--rules, 044) z opcjami deklarowanymi w nagłówku (045). Do zrobienia: akumulatory deklarowane w danych, PRNG per cząstka, reguły wyglądu (render), walidator i layout planner, hot swap poziomów A/B/C.
- M3 poziom C: jest schemat pól modelu shared/private w buforach reguł (046). Brak: lifecycle (kill/spawn, stream compaction), wiązania (bonds), histogramy pól w UI.
- M3 warsztat: brak panelu inspekcji wygenerowanego kodu i eksportu do rerun. Definition of Done M3 (3 modele wyłącznie w .ron) niespełnione.
Powiązane luźne zadania: 062 (f16), 066 (próg próbkowania w locie), 068 (zaznaczanie cząstek).
