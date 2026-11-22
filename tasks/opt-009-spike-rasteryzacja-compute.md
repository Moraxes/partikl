---
id: opt-009
title: Spike rasteryzacja compute
summary: 'Spike: rasteryzacja compute (sonnet)'
status: dropped
priority: later
branch: main
part_of: opt-001
relates:
- 72
created: 2026-11-22T01:12:31
worked: 2026-11-22T14:05:12
edited: 2026-11-22T14:05:12
links:
- kind: file
  target: .artifacts/patches/opt-009-raster-spike.patch
  at: 2026-11-22T14:05:12
  note: prototyp compute raster (bez zysku)
---
Model: sonnet (nowy przebieg compute, atomics, własny kompozyt do tekstury kamery; spike z bramką).

Cel: renderowanie 1M cząstek taniej. Cząstki mają kilka pikseli, a małe trójkąty marnują rasteryzer (render 1M ok. 7 ms). Nie przywiązywać się do AA i elips: obecny wygląd (analityczne AA, elipsa wydłużona z prędkością) to jedna realizacja celu 'gładki ruch'; wystarczy równie ładny efekt niższym kosztem (np. krótki odcinek last → pos jako rozmycie ruchu zamiast elipsy; wygląd oceni użytkownik).

Prototyp:
- przebieg compute po cząstkach; każdy wątek rysuje swój ślad do bufora pikseli (bbox promienia);
- wariant A: nieprzezroczysty, `atomicMax` na (warstwa 16 bit | typ/kolor);
- wariant B: addytywny, `atomicAdd` stałoprzecinkowego pokrycia, niezależny od kolejności (znika problem warstw z zadaniem 072);
- przebieg końcowy (fullscreen) do tekstury kamery.

Bramka: render 1M ≥30% szybciej (cargo bench --bench render -- --modes loop --sizes 500000,1000000) i akceptacja wyglądu przez użytkownika (galeria PNG: --gallery). Bez zysku: zamknąć z wynikiem (tq drop), nic w main. Wdrożenie po spike'u to osobne zadanie.

Wynik (2026-11-22): bramka niespełniona, zadanie porzucone, nic w main. Patch prototypu: .artifacts/patches/opt-009-raster-spike.patch (przełącznik PARTIKL_RASTER=1: przebieg compute `raster` na cząstkę → bufor pikseli u32 → jedno pełnoekranowe quad czytające bufor).
- Wariant A (atomicMax na warstwa16|gatunek, bbox z testem elipsy), bench render --modes loop, 1M, --quick: klatka ok. 14,7 ms vs 7,8–8,6 ms bazy (czyli ok. 80% wolniej). Wygląd jak render bez AA.
- Wariant ze spanami wierszy (bez testu per piksel) i atomicStore zamiast atomicMax: 11,8–13,2 ms; dalej wolniej od bazy, więc wąskim gardłem jest przepustowość zapisów (cząstka r=4 px = ok. 50 zapisów, 1M cząstek ≈ 50M zapisów), nie matematyka ani kolizje atomików.
- Koszt stały (clear 34 MB + kompozyt + krok) przy pustym przebiegu: ok. 1,9 ms klatki, więc narzut tła nie tłumaczy wyniku.
- Wariant B (atomicAdd, 3 kanały) potrzebuje jeszcze więcej zapisów, więc nie testowany. Ręczna rasteryzacja wygrałaby dopiero przy znacznie mniejszej stopie cząstki (kilka pikseli), czyli zmianie wyglądu.
