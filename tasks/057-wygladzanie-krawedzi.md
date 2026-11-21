---
id: 57
title: Wygładzanie krawędzi
summary: MSAA wyłączone + analityczne AA w fragment shaderze; pomiar i ocena wyglądu
status: done
priority: soon
branch: compute-shader-1
part_of: 54
blocked_by:
- 55
- 56
created: 2026-11-21T16:30:35
worked: 2026-11-21T17:31:14
edited: 2026-11-21T17:16:18
links:
- kind: commit
  target: 09e68e3
  at: 2026-11-21T17:16:18
  note: 'perf(gpu): MSAA off by default, optional analytic edge antialiasing'
- kind: commit
  target: e9e61fe
  at: 2026-11-21T17:31:14
  note: 'feat(bench): gallery of visual comparisons in the render bench'
---
Cel: MSAA 4× kosztuje ok. 20% klatki przy 500k z działającą symulacją. Sprawdzić po zadaniu kolejności rysowania, czy różnica nadal jest; jeśli tak, wyłączyć MSAA i wygładzić krawędzie elips w fragment shaderze (smoothstep po 1 − dot(corner, corner), alpha blending bez zapisu głębi). Ocena wyglądu na ekranie (zrzuty przed/po). Wynik: MSAA 4× nie wygładzał elips (okrąg wycina discard, jedna decyzja na piksel): na nieruchomych cząstkach MSAA zmieniał 2 piksele wobec 90 przy analitycznym AA; kosztował ok. 12% renderu. Domyślnie MSAA wył., HDR + tonemapping bez zmian (wygląd identyczny). 500k jednostajna, bench renderu (ms draw / draw+step): msaa4 hdr 5,63 / 11,49 → msaa wył. 4,94 / 10,42. --antialias (analityczne AA: pokrycie z fwidth, quad +25%, alpha blending): 5,78 / 11,49, czyli koszt jak dawny MSAA, ale z realnym wygładzeniem; przy 100k tylko +0,15 ms. LDR (bez Hdr kamera nie tonemapuje): 4,43 / 9,77, ale kolory żywsze (biały 255 zamiast 196), więc zmiana wyglądu, do decyzji użytkownika; tonemapping wył. przy HDR: 4,51 draw (też surowe kolory). Zrzuty: bench render --screenshots DIR (PNG). Flaga aplikacji --antialias.
