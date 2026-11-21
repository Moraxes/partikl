---
id: 65
title: Sąsiedzi 8 B f16
summary: 'sorted_hot 8 B: pozycje f16 względem komórki, bez konwersji w pętli (opcjonalnie, po bramce)'
status: todo
priority: later
branch: compute-shader-1
part_of: 62
blocked_by:
- 64
created: 2026-11-21T19:19:16
---
Cel: usunąć konwersję f32→f16 z pętli i zmniejszyć sąsiada z 16 B do 8 B: pozycja w f16 względem początku komórki (±20, precyzja ok. 0,016), indeks komórki i typ spakowane razem; delta = (różnica indeksów komórek × rozmiar komórki, dokładna w f16) + (pozycja lokalna sąsiada − własna). crowding poza gorącym buforem (tylko OVERLAP_ADAPTIVE). Uwaga: rozpakowanie to operacje całkowite, a jednostka integer/conditional jest na 74% limitera (profil 061), więc zysk nie jest pewny; bez wąskiego gardła pamięci (L1 2,6%) oszczędność 8 B/kandydata sama nic nie da. Bramka: tylko jeśli wariant PAIR_F16 pokaże ≥8% i liczniki (tools/profile-gpu) pokażą wzrost F16 Utilization bez nasycenia integer/conditional. Następny krok: po decyzji w zadaniu o dokładności.
