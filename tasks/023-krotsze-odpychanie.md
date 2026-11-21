---
id: 23
title: Krótsze odpychanie
summary: Zasięg odpychania 2 i gładki kształt (1−d²/r²)²; tylko z rozsuwaniem lub uśrednianiem
status: todo
priority: later
branch: main
blocked_by:
- 44
relates:
- 24
- 25
created: 2026-11-20T23:12:14
edited: 2026-11-21T22:43:15
---
Cel: zasięg odpychania 2 zamiast 10 i kształt dzwonowy zamiast liniowego, np. siła `k·(1−d²/r²)²` (zależy tylko od d², 3 mnożenia). Następny krok: zrobić po rozsuwaniu lub uśrednianiu i zmierzyć sondą; samo zmniejszenie zasięgu pogorszy stabilność.

## Ryzyka
- Rdzeń skupiska ściśnie się do odstępu ~2: ok. 25× gęściej, więcej nakładających się sąsiadów, większa sztywność. Bez rozsuwania lub uśredniania będzie gorzej niż teraz.
- Przenikanie: przy dt=0,01 cząstka szybsza niż ~200 j/s przeskakuje strefę 2 j w jednym kroku; szybkie zderzenia przestają odpychać (cząstki przenikają się). W zabawce może być w porządku.
- Kształt dzwonowy jest gładszy na granicy zasięgu (mniej szarpnięć przy wejściu w zasięg), ale nie stabilniejszy: przy tej samej sile w zerze maksymalne nachylenie ok. 1,5× większe niż liniowe.
- Cząstki mają promień 2 (`--particle-size`): odstęp 2 to widocznie nakładające się kółka.
- Rozsuwanie z zasięgiem 2: komórki 20 to dużo kandydatów na parę; tu gęstsza siatka może się opłacić.

Aktualizacja (2026-11-21, review backlogu): pomiar 'sondą' (bench probe) już nie istnieje; po 044 (wymienny moduł reguł) zmianę reguły da się sprawdzić bez ruszania silnika, mierzyć bench gpu. Zależy od 044.
