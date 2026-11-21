---
id: 56
title: Pomiar renderu
summary: 'Trwały pomiar kosztu renderowania w aplikacji: bez vsync, log czasu klatki, tryb bez symulacji'
status: done
priority: soon
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
worked: 2026-11-21T17:08:28
edited: 2026-11-21T17:08:28
links:
- kind: commit
  target: 3eae539
  at: 2026-11-21T17:08:28
  note: 'feat(bench): render bench without a window'
---
Cel: bench mierzy tylko compute, a pomiary renderu powstały z jednorazowych przełączników w aplikacji (okno wyskakuje na wierzch, a po utracie fokusu Bevy spowalnia je do 60 Hz: WinitSettings::game, unfocused reactive_low_power). Zamiast opcji aplikacji: bench renderu bez okna (offscreen). Harness z kamerą renderującą do Image (RenderTarget::Image, rozmiar fizyczny ekranu, np. 3600×2338), konfigurowalne MSAA/HDR, pomiar czasu klatki z poll(Wait) jak dla kroków; tryb sam render (po kilku krokach, żeby stan był w kolejności siatki) i render z symulacją. Wyniki do Parquet (kolumny jak gpu, scena/n/msaa/hdr), A/B przez ab.sh. Bez okna, vsync i fokusu; nie dotyczy kosztu prezentacji do kompozytora. Wynik: benches/render (cargo bench --bench render; AB_BENCH=render ab.sh), Harness::with_render(Offscreen { size, msaa, hdr }) i spawn_camera wspólny z aplikacją; kolumna frame_ns (format 1.4). 500k, jednostajna, świat 1800×1169, obraz 3600×2338 (ms klatki): msaa4 draw 5,5, draw+step 11,1; msaa wył. 4,8 / 10,2; --steps 0 (stan nieposortowany) draw 12,2 (zgodne z pomiarem w aplikacji sprzed eb49be5). Powtarzalność ok. 1%. Różnice względem aplikacji: bez prezentacji do kompozytora; MSAA w benchu kosztuje mniej (draw −0,6 ms) niż w pełnej klatce aplikacji (−1,6 ms), do wyjaśnienia przy zadaniu 057. Uwaga: ab.sh buduje 'przed' z worktree, więc A/B renderu działa dopiero dla rewizji z tym benchem.
