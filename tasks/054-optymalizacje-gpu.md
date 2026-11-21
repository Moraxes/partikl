---
id: 54
title: Optymalizacje GPU
summary: 'Render ok. 50% klatki przy 500k; największy tani zysk: kolejność rysowania; obliczenia bez tanich zysków'
status: todo
priority: soon
branch: compute-shader-1
created: 2026-11-21T16:30:34
---
Cel: płynne 60 fps dla 500k–1M cząstek. Pomiary (M5, release, aplikacja bez vsync, 500k): klatka ok. 21 ms = symulacja ok. 9–10 ms + render ok. 11 ms; sam render 1k: 1,6 ms, 100k: 3,6, 500k: 10,8, 1M: 20,2 ms. Rysowanie w kolejności siatki: 11,3 → 3,8 ms (górna granica: stan posortowany raz, symulacja wyłączona). MSAA wyłączone przy działającej symulacji: 20,7 → 16,3 ms na klatkę (3 powtórzenia naprzemiennie); sam render od MSAA nie zależy. Bez efektu: HDR wył., 4 zamiast 6 wierzchołków (indeksowany quad), jeden trójkąt na cząstkę (11,3 ms), obliczenia kształtu/discard/odczyt stanu w vertex shaderze (łącznie < 2 ms), rozmiar cząstki (256× pola = +7 ms), rozmiar grupy roboczej 32/128/256 (±1%), GRID_REACH 3 (−2% jednostajny, +5% skupiska) i 4 (gorzej), posortowany stan dla obliczeń (−5% przy 100k, 0 przy 500k). Wniosek: pętla sił (ok. 90% czasu GPU) nie wygląda na ograniczoną pamięcią, więc punkty M2 3.1 (SoA/kompresja) i 3.3 (Morton) mogą dać mało; do potwierdzenia licznikami GPU. Nie sprawdzone: instancing z buforem stanu jako vertex buffer (stały fetch zamiast odczytu storage). Następny krok: podzadania.
