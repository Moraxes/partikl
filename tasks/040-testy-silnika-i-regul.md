---
id: 40
title: Testy silnika i reguł
summary: Reguła diagnostyczna vs brute force osobno od wyroczni default_rules
status: todo
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 36
- 37
- 38
- 39
created: 2026-11-21T12:39:03
---
Cel: reference_step powiela całą fizykę w f64, więc każda zmiana reguł psuje wyrocznię. Test silnika z regułą diagnostyczną (liczba sąsiadów, suma delta) vs brute force; wyrocznia tylko dla default_rules; test niezmienników (skończone wartości, cząstki w świecie, komplet id) dla dowolnych reguł. Następny krok: reguła diagnostyczna w WESL.
