---
id: 40
title: Testy silnika i reguł
summary: Reguła diagnostyczna vs brute force osobno od wyroczni default_rules
status: done
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 36
- 37
- 38
- 39
created: 2026-11-21T12:39:03
worked: 2026-11-21T20:14:10
edited: 2026-11-21T20:14:10
links:
- kind: commit
  target: 759e842
  at: 2026-11-21T20:14:03
  note: 'test(gpu): test the grid engine with diagnostic rules, split tests by topic'
---
Cel: reference_step powiela całą fizykę w f64, więc każda zmiana reguł psuje wyrocznię. Test silnika z regułą diagnostyczną (liczba sąsiadów, suma delta) vs brute force; wyrocznia tylko dla default_rules; test niezmienników (skończone wartości, cząstki w świecie, komplet id) dla dowolnych reguł. Wynik (759e842): rules/diagnostic_rules.wesl (suma wag, suma wag·delta i suma wag·coeff do pól cząstki: crowding, last, bity f32 w _pad), wybór zestawu reguł przez zasób RuleSet {Default, Diagnostic} (część ForcesVariant, flaga shadera RULES_DIAGNOSTIC, warunkowy import WESL: dwa @if(…) import, bez @else, bo import nie przyjmuje @else); to wersja tymczasowa, 044 zastąpi ją wymiennym modułem reguł. Testy silnika z wyrocznią brute force f64 (torus, wszystkie pary): każdy sąsiad w zasięgu, współczynnik dla 1/2/4/5/8/9/13 typów, szew torusa (cztery narożniki, wartość policzona ręcznie), duży świat z większymi komórkami, próbkowanie powyżej 512 kandydatów (stan 20k: średnio 609 sąsiadów, suma w 0,01%, błąd cząstki do 11%, próg 25%), niezmienniki dla reguł diagnostycznych. Wyrocznia reference_step zostaje tylko dla reguł domyślnych. Testy rozbite: src/gpu/tests/{mod,engine,default_rules,render}.rs, wspólny assert_invariants. Sprawdzone uszkodzeniem shadera: brak przesunięcia torusa wywala 5 testów, stała waga próbkowania test gęstego stanu, zły współczynnik trzy testy współczynnika. 45 testów, wszystkie przechodzą.
