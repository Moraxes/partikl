---
id: opt-009
title: Spike rasteryzacja compute
summary: 'Spike: rasteryzacja compute (sonnet)'
status: todo
priority: later
branch: main
part_of: opt-001
relates:
- 72
created: 2026-11-22T01:12:31
---
Model: sonnet (nowy przebieg compute, atomics, własny kompozyt do tekstury kamery; spike z bramką).

Cel: renderowanie 1M cząstek taniej. Cząstki mają kilka pikseli, a małe trójkąty marnują rasteryzer (render 1M ok. 7 ms). Nie przywiązywać się do AA i elips: obecny wygląd (analityczne AA, elipsa wydłużona z prędkością) to jedna realizacja celu 'gładki ruch'; wystarczy równie ładny efekt niższym kosztem (np. krótki odcinek last → pos jako rozmycie ruchu zamiast elipsy; wygląd oceni użytkownik).

Prototyp:
- przebieg compute po cząstkach; każdy wątek rysuje swój ślad do bufora pikseli (bbox promienia);
- wariant A: nieprzezroczysty, `atomicMax` na (warstwa 16 bit | typ/kolor);
- wariant B: addytywny, `atomicAdd` stałoprzecinkowego pokrycia, niezależny od kolejności (znika problem warstw z zadaniem 072);
- przebieg końcowy (fullscreen) do tekstury kamery.

Bramka: render 1M ≥30% szybciej (cargo bench --bench render -- --modes loop --sizes 500000,1000000) i akceptacja wyglądu przez użytkownika (galeria PNG: --gallery). Bez zysku: zamknąć z wynikiem (tq drop), nic w main. Wdrożenie po spike'u to osobne zadanie.
