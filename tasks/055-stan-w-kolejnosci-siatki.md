---
id: 55
title: Stan w kolejności siatki
summary: scatter zapisuje całą cząstkę, siły piszą do state[j]; render rysuje spójnie przestrzennie
status: done
priority: soon
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
worked: 2026-11-21T16:44:13
edited: 2026-11-21T16:44:13
links:
- kind: commit
  target: eb49be5
  at: 2026-11-21T16:44:13
  note: 'perf(gpu): keep the particle state in grid order'
---
Cel: bufor stanu zawsze w kolejności siatki z poprzedniego kroku, więc render (i kafelkowanie) widzi cząstki spójnie przestrzennie. Zmiany: sorted_ids → sorted_state (pełne cząstki, 32 B) w scatterze i sort_cells; forces czyta me z sorted_state[j], próbkowanie po me.id, zapisuje state[j]; Harness::run sortuje odczyt po id (testy bez zmian). Weryfikacja: testy GPU, bench (bez regresji powyżej ok. 10%, inaczej zgłosić), render 500k przed/po. Wynik: sorted_state (pełne cząstki) zamiast sorted_ids; siły czytają me z sorted_state[j] i piszą state[j]; Harness::run sortuje po id. 500k, M5, release, bez vsync, wymuszone ciągłe odświeżanie okna (średni czas klatki, przed → po): sam render (po 5 krokach) 10,8 → 4,2 ms; pełna klatka z symulacją ok. 20,7 → 8,7 ms; z MSAA wył. 15,4 → 7,1 ms (MSAA nadal ok. 18% klatki); 1M: sam render 20,2 → 7,0 ms. Compute: siły bez zmian, budowa siatki +30% (scatter pisze o 32 B więcej: 100k 64 → 68–77 µs, 500k 370 → 480 µs, ok. +2% kroku), GPU krok przy 100k −2…+1,5%. Uwaga na pomiary: Bevy spowalnia okno bez fokusu do 60 Hz (WinitSettings::game: unfocused reactive_low_power 1/60 s), co psuło część wcześniejszych pomiarów (stałe 16,66 ms); pomiary w tym zadaniu z wymuszonym Continuous. Na 1M klatka ok. 140 ms przed i po: ogranicza symulacja (krok ok. 11 ms > 10 ms budżetu), nie render; patrz zadanie o zegarze kroków. Przełączniki pomiarowe (nie w repo): skrypt probe.py w scratchpadzie sesji; trwała wersja: zadanie 056.
