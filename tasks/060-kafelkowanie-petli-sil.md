---
id: 60
title: Kafelkowanie pętli sił
summary: Szkielet pętli (odczyt sąsiada, test zasięgu, sterowanie) to ok. 75% przebiegu sił; eksperyment z kafelkiem w workgroup
status: todo
priority: soon
branch: compute-shader-1
part_of: 54
relates:
- 58
created: 2026-11-21T18:09:50
---
Cel: po usunięciu odczytu macierzy z pętli (058) przebieg sił to ok. 463 µs przy 100k jednostajnych, z czego ok. 349 µs to szkielet: odczyt sąsiada (16 B z sorted_hot), test zasięgu, sterowanie pętlą run_at. Ok. połowa kandydatów leży poza zasięgiem (kwadrat 100×100 wokół koła r=40). Do sprawdzenia (M2 3.2): kafelek cell-centric, wątki jednej komórki ładują wspólnie sąsiadów do var<workgroup>, albo tańsze odrzucanie kandydatów (np. odrzucanie całych biegów komórek poza zasięgiem). Mierzyć A/B (AB_BENCH=gpu ab.sh, --quick i --types 3/6) i czasami przebiegów (gpu_forces_ns). Następny krok: prototyp kafelka dla jednej komórki na grupę i porównanie z szkieletem (349 µs).
