---
id: 57
title: Wygładzanie krawędzi
summary: MSAA wyłączone + analityczne AA w fragment shaderze; pomiar i ocena wyglądu
status: doing
priority: soon
branch: compute-shader-1
part_of: 54
blocked_by:
- 55
- 56
created: 2026-11-21T16:30:35
worked: 2026-11-21T17:10:29
---
Cel: MSAA 4× kosztuje ok. 20% klatki przy 500k z działającą symulacją. Sprawdzić po zadaniu kolejności rysowania, czy różnica nadal jest; jeśli tak, wyłączyć MSAA i wygładzić krawędzie elips w fragment shaderze (smoothstep po 1 − dot(corner, corner), alpha blending bez zapisu głębi). Ocena wyglądu na ekranie (zrzuty przed/po). Następny krok: pomiar po zadaniu kolejności i pomiaru renderu.
