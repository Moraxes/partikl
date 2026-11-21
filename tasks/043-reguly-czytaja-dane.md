---
id: 43
title: Reguły czytają dane
summary: Reguły importują własne wiązania i same czytają macierze zamiast gotowego coeff
status: done
priority: soon
branch: main
part_of: 35
created: 2026-11-21T12:39:03
worked: 2026-11-21T23:43:29
edited: 2026-11-21T23:43:29
---
Cel (poziom A): rules importują swoje wiązania (macierze, parametry) i robią matrix(a, b) same, więc druga macierz nie wymaga zmian w silniku. Następny krok: przenieść odczyt force_matrix z step_driver do default_rules.


Wynik: step_driver nie zna macierzy ani RuleParams. Kontrakt reguł: RuleAccum, accum_init, RuleSelf, prepare_self (dane własnej cząstki raz na cząstkę, np. kolumna macierzy w rejestrach), pair_interact(me, other, delta, dist_sq, rule_self, weight, accum), integrate_particle(me, accum). Reguły importują `rule_params` i macierz same; odczyt macierzy z wariantami TYPES_AT_MOST_* w rules/matrix.wesl (wspólny dla default i diagnostic). Bufor i wiązanie force_matrix zostają w silniku (alokacja generyczna to 046). Commit 3c1175d, 51 testów OK.
