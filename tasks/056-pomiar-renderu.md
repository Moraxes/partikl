---
id: 56
title: Pomiar renderu
summary: 'Trwały pomiar kosztu renderowania w aplikacji: bez vsync, log czasu klatki, tryb bez symulacji'
status: doing
priority: soon
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
worked: 2026-11-21T17:08:28
edited: 2026-11-21T16:56:31
links:
- kind: commit
  target: 3eae539
  at: 2026-11-21T17:08:28
  note: 'feat(bench): render bench without a window'
---
Cel: bench mierzy tylko compute, a pomiary renderu powstały z jednorazowych przełączników w aplikacji (okno wyskakuje na wierzch, a po utracie fokusu Bevy spowalnia je do 60 Hz: WinitSettings::game, unfocused reactive_low_power). Zamiast opcji aplikacji: bench renderu bez okna (offscreen). Harness z kamerą renderującą do Image (RenderTarget::Image, rozmiar fizyczny ekranu, np. 3600×2338), konfigurowalne MSAA/HDR, pomiar czasu klatki z poll(Wait) jak dla kroków; tryb sam render (po kilku krokach, żeby stan był w kolejności siatki) i render z symulacją. Wyniki do Parquet (kolumny jak gpu, scena/n/msaa/hdr), A/B przez ab.sh. Bez okna, vsync i fokusu; nie dotyczy kosztu prezentacji do kompozytora. Następny krok: kamera z Image jako celem w Harness (bez PrimaryWindow init_gpu nie spawnuje kamery).
