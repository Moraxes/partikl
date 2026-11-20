---
id: 15
title: Siły w kolejności komórek
summary: compute_forces po cząstkach posortowanych komórkami, w kilku dużych ciągłych zakresach; mniej narzutu puli
status: todo
priority: soon
branch: main
part_of: 12
created: 2026-11-20T20:29:45
---
Cel: podnieść wykorzystanie wątków w `compute_forces` (ok. 11% przy macierzy 23, ok. 40% przy 4) i lokalność danych. Następny krok: iterować po cząstkach w kolejności komórek z tablic z sortowania (kolejne cząstki mają tych samych sąsiadów w L1), w zakresach ciągłych zamiast `par_iter_mut` w kolejności ECS; przyspieszenia do tablicy, potem przepisać do `Acceleration`.

## Do sprawdzenia
- Liczba i rozmiar zadań: mniej, większych przy małej gęstości; podział według liczby kandydatów (suma prefiksowa) zamiast według liczby cząstek, żeby wyrównać skupiska.
- rayon zamiast wykonawcy zadań Bevy (`mutexwait`, `cvwait`, tworzenie zadań to ok. 13% kroku przy macierzy 23).
- Rdzenie wydajne i energooszczędne: przy macierzy 4 wątek główny czeka 12% kroku na najwolniejsze wątki.
