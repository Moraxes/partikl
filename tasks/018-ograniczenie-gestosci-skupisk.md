---
id: 18
title: Ograniczenie gęstości skupisk
summary: 'Mocniejsze odpychanie albo limit przyciągania: najgorszy przypadek (1220 kandydatów) bliżej typowego; zmienia wygląd'
status: todo
priority: later
branch: main
part_of: 12
created: 2026-11-20T20:29:45
---
Cel: ograniczyć koszt najgorszego przypadku. W skupiskach macierzy 4 odstęp to ok. 3,4 przy promieniu odpychania 10: odpychanie `100·(10−d)` przegrywa ze współczynnikami do 500. Następny krok: `REPULSION_STRENGTH` 100 → np. 1000 (albo limit siły przyciągania na cząstkę), zmierzyć kandydatów i czas `--quick` dla 32 macierzy, obejrzeć wygląd.

## Ryzyka
- Zmienia wygląd: skupiska większe i rzadsze; do akceptacji wizualnej przez użytkownika.
- Sztywniejsze odpychanie przy dt = 0,01 może rozregulować całkowanie (Verlet), sprawdzić stabilność.
- Zmienia klasyfikację macierzy trybu szybkiego (wolny/typowy/szybki wzrost), wybór `QUICK_SEEDS` do powtórzenia.
