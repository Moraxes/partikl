---
id: 33
title: reguly-czytaja-dane
summary: Reguły importują własne wiązania i same czytają macierze zamiast gotowego coeff
status: todo
priority: soon
branch: compute-shader-1
part_of: 29
created: 2026-11-21T12:38:33
---
Cel (poziom A): rules importują swoje wiązania (macierze, parametry) i robią matrix(a, b) same, więc druga macierz nie wymaga zmian w silniku. Następny krok: przenieść odczyt force_matrix z step_driver do default_rules.
