---
id: 25
title: Rozsuwanie nakładających się cząstek
summary: Poprawka pozycji par bliżej niż zasięg odpychania zamiast sprężyny
status: doing
priority: soon
branch: main
blocked_by:
- 22
created: 2026-11-20T23:12:22
worked: 2026-11-20T23:44:23
edited: 2026-11-20T23:20:42
links:
- kind: commit
  target: 00f9005
  at: 2026-11-20T23:44:23
  note: 'feat: overlap resolution by position correction as an overlap mode'
---
Cel: tryb `Overlap::Resolve` (drugi z trzech trybów nakładania, obok sprężyny i zasięgu od gęstości 026, jeden klawisz cyklicznie). Po całkowaniu i zawinięciu pary bliżej niż `REPULSION_RADIUS` rozsuwane poprawką pozycji (po pół wnikania, poprawki sumowane, potem stosowane: wynik niezależny od kolejności); sprężyna odpychania wyłączona w jądrze. Następny krok: przebiegi 3×3 komórek w siatce (komórka 20 ≥ zasięg 10), system z run_if.

## Uwagi
- Bez osobnej siatki: dodatkowe przebiegi liczone raz w `with_reach`. Gęstsza siatka dopiero, gdy pomiar pokaże wąskie gardło.
- Poprawka samej pozycji dodaje prędkość (Verlet); przesunięcie też `LastPosition` jej nie dodaje: sprawdzić, co stabilniejsze.
- Skupisko przy n=100000 rozleje się w dysk upakowany co 10 j: inny wygląd gwiazd.
- Pomiar jak w uśrednianiu sił.
