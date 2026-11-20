---
id: 11
title: Zapis stanu benchmarku
summary: Stan symulacji po offsecie z cache zamiast symulowania 4096 kroków; pomiar przed/po na tym samym stanie
status: doing
priority: later
branch: main
created: 2026-11-20T19:56:05
worked: 2026-11-21T00:38:37
---
Tryb szybki (`--quick`, commit 6808821) za każdym razem symuluje 4096 kroków, żeby dojść do mierzonego stanu, a zmiana, która zmienia trajektorię (np. kolejność działań jak przy SIMD), przesuwa liczbę kandydatów przy każdej z trzech macierzy, więc porównanie przed/po miesza zmianę czasu ze zmianą stanu. Następny krok: zapisywać pełny stan (pozycje, `LastPosition`, typy) po offsecie w `.cache/`, jak `grid` robi z `.cache/grid-states`, i mierzyć od niego; `--regenerate` przy świadomej zmianie dynamiki.

## Dane
- Ten sam kod, dwa przebiegi `--quick` po 100 powtórzeń: stosunek median kroku ×0,99–1,02 na macierz, CI ok. ±1%; między przebiegami zostaje 1–2% dryfu niezależnie od liczby powtórzeń.
- Etap SIMD (trajektorie różne): w trybie szybkim CI stosunku po macierzach [0,38; 0,83], bezużyteczne.

## Ryzyka
- Pomiar od zapisanego stanu nie wykryje zmiany, która psuje dynamikę; pełny bench dalej symuluje od początku.
- Stan po 4096 krokach z jednej rewizji; po zmianie fizyki trzeba go wygenerować od nowa.
