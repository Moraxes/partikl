---
id: 58
title: Profil GPU w Xcode
summary: 'Liczniki GPU (Instruments/Metal): czy pętla sił jest ograniczona obliczeniami czy opóźnieniami'
status: doing
priority: later
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
worked: 2026-11-21T18:09:33
edited: 2026-11-21T18:09:50
links:
- kind: commit
  target: 2d4081b
  at: 2026-11-21T18:09:33
  note: 'perf(gpu): keep the force coefficients of a particle''s type in registers'
---
Cel: potwierdzić lub obalić hipotezę, że pętla sił nie jest ograniczona pamięcią, zanim zainwestujemy w M2 3.1 (SoA/kompresja) i 3.3 (Morton). Xcode zainstalowany (/Applications/Xcode.app), ale licencja niezaakceptowana (sudo xcodebuild -license accept, robi użytkownik); xcode-select wskazuje na Command Line Tools, więc używać DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun xctrace … (bez sudo). Metal System Trace i GPU counters: nagranie z linii poleceń (xctrace record --template …), analiza liczników najwygodniej w GUI. Wynik: (1) Instruments z CLI działa headless: DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun xctrace record --template 'Metal System Trace' --instrument 'Metal GPU Counters' --output X.trace --time-limit 20s --no-prompt --launch -- <binarka benchu> …; eksport: xctrace export --input X.trace --toc oraz --xpath '/trace-toc/run[@number="1"]/data/table[@schema="…"]' (XML z kompresją id/ref). Dostajemy interwały GPU (metal-gpu-intervals), czas na shader i próbki PC (metal-shader-profiler-intervals, gpu-shader-profiler-interval, lista shaderów z zakresami PC). (2) Liczniki wydajności (ALU, pamięć, occupancy, limitery) NIE zbierają się z CLI: instrument ma domyślnie jeden licznik (RT Unit Active, same zera), a zestawu liczników nie da się ustawić przez --recording-options (opcje tylko Hangs/Time Profiler/Perf Overview). Wymaga szablonu zapisanego z GUI Instruments (xctrace --template ścieżka.tracetemplate) albo przechwycenia klatki w Xcode; patrz zadanie o szablonie liczników. (3) Pytanie ALU vs pamięć rozstrzygnięte ablacjami shadera sił (100k, czas GPU przebiegu sił, µs uniform/clusters/dense; baseline 600/881/958): bez pair_interact (odczyt sąsiada + test zasięgu zostają) 349/487/530; stały współczynnik zamiast odczytu macierzy 420/604/655; współczynnik z arytmetyki 454/655/712; macierz w pamięci workgroup 611/894/974 (bez zysku); kolumna w prywatnej tablicy z dynamicznym indeksem 698/1034/1119 (gorzej); kolumna w 2×vec4 + select 524/790/848; w 1×vec4 475/682/739. Odczyt macierzy w pętli (zależny łańcuch sąsiad → typ → macierz) kosztował ok. 23% przebiegu, pair_interact ok. 19% po poprawce; reszta (ok. 75%: odczyt sąsiada 16 B, test zasięgu, sterowanie pętlą) to szkielet, którego bez liczników nie umiem rozbić na opóźnienie odczytu i ALU. Granice tablic nie są winne: Bevy kompiluje nasze shadery bez sprawdzeń (ValidateShader::Disabled). Syntetyczni sąsiedzi (ablacja bez odczytu) wyszli wolniej przez zmianę dywergencji: niemiarodajne. (4) Wdrożone: współczynniki własnego typu w rejestrach (TypeClass: ≤4 typy vec4, ≤8 2×vec4, więcej: macierz); A/B 3 typy przebieg sił ×0,76…0,77 (krok GPU −20…−23%), 6 typów ×0,86…0,90, 10 typów bez zmian. Wniosek dla M2: 3.1 (SoA/kompresja) i 3.3 (Morton) nadal niepotwierdzone, a szkielet pętli (75%) sugeruje, że kafelkowanie w pamięci workgroup (3.2) może dać więcej niż zmiany układu danych.
