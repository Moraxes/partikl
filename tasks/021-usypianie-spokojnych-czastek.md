---
id: 21
title: Usypianie spokojnych cząstek
summary: 'Bez zmian: po 4096 krokach żadna cząstka nie jest spokojna, skupiska ruchliwe w środku'
status: done
priority: later
branch: main
part_of: 12
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:39:05
edited: 2026-11-20T21:39:05
---
Wynik: bez zmian w kodzie, nie ma czego usypiać. Po 4096 krokach przy n=9000 żadna cząstka nie ma prędkości poniżej `VELOCITY_THRESHOLD` ani poniżej 1 j/s, także w skupiskach; skupiska gęstych macierzy są ruchliwe w środku.

## Pomiar (sonda, pozycje i prędkości po 4096 krokach)
| macierz | cząstek w skupiskach (ponad 100 w zasięgu) | prędkość w skupiskach p10 / med / p90 [j/s] |
|---|---|---|
| 23 | 0 | — (wszystkie: 40 / 98 / 163) |
| 6 | 4619 | 6,3 / 16,9 / 56,0 |
| 4, przed próbkowaniem (`33c6386`) | 5246 | 251 / 835 / 1287 |
| 4, z próbkowaniem (`f2ea0ba`) | 5608 | 238 / 548 / 886 |

Przy macierzy 6 cząstki w skupiskach są najwolniejsze, ale i tak 6–56 j/s (ok. 0,06–0,56 j na krok), a ich praca i tak jest mała (próg par liczonych raz, bez próbkowania). Próbkowanie sąsiadów (zadanie 019) nie zwiększyło ruchu w skupiskach macierzy 4, raczej go zmniejszyło.

## Pierwotny plan
Cel: mniej pracy przy statycznych skupiskach (szybkie macierze). Następny krok: zmierzyć, jaki odsetek cząstek przy macierzy 4 po offsecie 4096 ma prędkość poniżej `VELOCITY_THRESHOLD` i stały zbiór sąsiadów; jeśli duży, przeliczać ich siły co k kroków (siła z poprzedniego przeliczenia).

## Ryzyka
- Budzenie: cząstka uśpiona musi reagować na nadlatujące cząstki (np. przelicz, gdy w jej komórkach zmieniła się liczba cząstek).
- Nierówna praca między krokami (co k-ty krok droższy) może dawać szarpanie animacji.
