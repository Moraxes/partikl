---
id: 60
title: Kafelkowanie pętli sił
summary: Szkielet pętli (odczyt sąsiada, test zasięgu, sterowanie) to ok. 75% przebiegu sił; eksperyment z kafelkiem w workgroup
status: doing
priority: soon
branch: compute-shader-1
part_of: 54
relates:
- 58
created: 2026-11-21T18:09:50
worked: 2026-11-21T19:01:57
edited: 2026-11-21T18:51:42
links:
- kind: commit
  target: cea9096
  at: 2026-11-21T18:32:09
  note: 'perf(gpu): process neighbour candidates four at a time'
- kind: commit
  target: 7b7b90b
  at: 2026-11-21T19:01:57
  note: 'perf(gpu): test the candidate range with one integer comparison'
---
Cel: po usunięciu odczytu macierzy z pętli (058) przebieg sił to ok. 463 µs przy 100k jednostajnych, z czego ok. 349 µs to szkielet: odczyt sąsiada (16 B z sorted_hot), test zasięgu, sterowanie pętlą run_at. Ok. połowa kandydatów leży poza zasięgiem (kwadrat 100×100 wokół koła r=40). Do sprawdzenia (M2 3.2): kafelek cell-centric, wątki jednej komórki ładują wspólnie sąsiadów do var<workgroup>, albo tańsze odrzucanie kandydatów (np. odrzucanie całych biegów komórek poza zasięgiem). Mierzyć A/B (AB_BENCH=gpu ab.sh, --quick i --types 3/6) i czasami przebiegów (gpu_forces_ns). Postęp (krok 1, commit perf(gpu): process neighbour candidates four at a time): zanim kafelek, tańsze ablacje szkieletu. (a) Przebieg zliczający kandydatów (10 × run_at dwa razy na wątek) to tylko ok. 3,5% przebiegu (464 → 448 µs, uniform). (b) Pętla czeka na opóźnienie zależnych odczytów (odczyt sąsiada → test zasięgu → gałąź), więc odczyty z wyprzedzeniem: rozwinięcie ×2 392/559/599 µs, ×4 385/542/578, ×8 389/535/570 (przebieg sił, 100k uniform/clusters/dense; wcześniej 464/669/726). Wdrożone ×4 (visit() na kandydata, ta sama kolejność akumulacji, tryb deterministyczny bez zmian): A/B przebieg sił ×0,83/0,81/0,80 (3 typy), ×0,84/0,80/0,80 (6 typów); razem z kolumną macierzy w rejestrach przebieg sił 600/881/958 → 385/541/577 µs (−36…−40%). 500k jednostajna: przebieg sił 4,3–4,6 ms → 2,66 ms. Pętla aplikacji headless (klatka / prędkość symulacji): 500k 4,9 ms / 100%, 800k 10,2 ms / 100% (było 77%), 1M 13,5 ms / 84% (było 65%).
Wniosek: pętla jest ograniczona opóźnieniem odczytów (ILP pomaga), więc kafelek w pamięci workgroup nie jest już pierwszym kandydatem. Następny krok: dwie cząstki z tej samej komórki w jednym wątku (blokowanie rejestrów: jeden odczyt sąsiada obsługuje dwa testy zasięgu; wątki z różnych komórek idą ścieżką pojedynczą), potem dopiero kafelek workgroup, jeśli liczniki (zadanie 061) wskażą ograniczenie pamięcią.

Profil liczników (061, HEAD cea9096): pętla sił jest ograniczona instrukcjami, nie pamięcią: F32 Limiter 84% (util. 65%), Integer and Conditional Limiter 74% (util. 53%), Instruction Throughput Limiter 85%; L1 Cache Utilization 2,6%, pamięć GPU ok. 15 GB/s, LLC 11%. Wniosek dla planu: kafelkowanie w workgroup (odczyty sąsiadów z pamięci współdzielonej) i kompresja pozycji (3.1) niczego nie dadzą, bo odczyty nie są wąskim gardłem; liczy się liczba instrukcji na kandydata. Kierunki: (1) blokowanie rejestrów (dwie cząstki z tej samej komórki w jednym wątku): koszt sterowania pętlą i adresowania (jednostka integer/conditional 53%) dzieli się na dwie cząstki, a jeden odczyt sąsiada obsługuje dwa testy; (2) mniej kandydatów poza zasięgiem (ok. połowa): powtórzyć GRID_REACH 3 po rozwinięciu pętli; (3) tańszy test zasięgu: jedno porównanie całkowite na bitach (dist_sq > 0 && dist_sq <= R²) zamiast dwóch zmiennoprzecinkowych; (4) algorytmiczne, wymaga zgody: niższy próg próbkowania max_candidates (512).
