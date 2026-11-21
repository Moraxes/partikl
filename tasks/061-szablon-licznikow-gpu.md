---
id: 61
title: Szablon liczników GPU
summary: Szablon Instruments z zestawem liczników wydajności, żeby xctrace zbierał ALU/pamięć/occupancy
status: done
priority: later
branch: compute-shader-1
part_of: 54
created: 2026-11-21T18:09:50
worked: 2026-11-21T18:51:42
edited: 2026-11-21T18:51:42
links:
- kind: commit
  target: 1313b69
  at: 2026-11-21T18:51:42
  note: 'feat(tools): record and summarize GPU counters of the gpu bench'
---
Cel: liczniki wydajności GPU (limitery ALU i pamięci, occupancy) nie zbierają się z CLI (patrz 058). Użytkownik zapisuje z GUI Instruments szablon (Metal System Trace + Metal GPU Counters z wybranym zestawem liczników, np. Performance Limiters), a xctrace record --template ścieżka.tracetemplate nagrywa headless bench; eksport tabeli gpu-counter-value (parser z id/ref w scratchpadzie sesji, do przeniesienia do tools/). Następny krok: użytkownik zapisuje szablon; potem skrypt tools/profile-gpu.sh.

Ustalenia (2026-11-21): (1) Repozytorium rudybear/metal-ai-skill (skill Claude Code do debugowania Metal z CLI) niczego nie wnosi w sprawie liczników: opisuje xctrace record/export, MTL_HUD_ENABLED, METAL_CAPTURE_ENABLED, walidację; nie ma konfiguracji zestawów liczników ani nic o wgpu/WGSL. (2) Szablon Metal System Trace leży w /Applications/Xcode.app/Contents/Applications/Instruments.app/Contents/Packages/GPU.instrdst/Contents/Templates/ (binarny plist NSKeyedArchiver; słownik opcji nagrywania to obiekt 1202: counterprofile, counterdevice, shaderprofiler, counterscounterprofile, countersshaderprofiler, counterscounterdevice). Próby edycji tych kluczy (profile 1–5) nie zmieniły nagrania: tabela metal-gpu-counter-profile zostaje counter-profile=0, a zbierany jest jeden licznik (RT Unit Active, zera); szablon domyślnie ma też instancję z profilem 3 + shader profiler bez liczników. Instrument 'Metal GPU Counters' dodany przez --instrument ma profil 0. Wniosek: zestaw liczników wybiera się w innym miejscu archiwum albo tylko w GUI. Następny krok: zrzut ekranu okna Instruments (menu File i Recording Options dokumentu Metal System Trace) od użytkownika; jeśli GUI nie oferuje zestawu liczników na tym GPU (M5), zadanie zamknąć jako niewykonalne i dalej używać ablacji shadera.

Aktualizacja: użytkownik zapisał szablon z GUI: ~/Library/Application Support/Instruments/Templates/metal-gpu-counters.tracetemplate (widoczny dla xctrace jako 'metal-gpu-counters'; ustawia counter-profile=13, counter-device=4294968546 = akcelerator tego Maca, shader-profiler 0/1). Nagranie bez sudo kończy się błędem 'Failed to start the recording: Could not set the recording priority' (ślad bez liczników). Następny krok: nagranie z sudo uruchomione przez użytkownika (xctrace potrzebuje uprawnień do liczników), potem eksport gpu-counter-info / gpu-counter-value (parser agg.py z id/ref) i tools/profile-gpu.sh.

Wynik: szablon zapisany przez użytkownika w GUI (tools/profile-gpu/metal-gpu-counters.tracetemplate, counter-profile 13) + nagranie z sudo (bez sudo: 'Could not set the recording priority') daje 88 liczników (zajętość, limitery i wykorzystanie F32/F16/integer/control flow, L1, LLC, MMU, przepustowość). Skrypty: tools/profile-gpu/profile.sh (sudo) i counters.py (podsumowanie próbek z zajętością compute > 5%; ok. 3 min na 6 s śladu, bo XML ma 1,4 GB). macOS: Developer mode wyłączony (DevToolsSecurity -status), użytkownik w grupach admin i _developer; sudo wymaga hasła.

Pomiar HEAD (cea9096), bench gpu 500k jednostajne, mediana próbek zajętych: Kernel Occupancy 22,7% (p90 44%); ALU Utilization 62%; F32 Limiter 84% / Utilization 65%; Integer and Conditional Limiter 74% / Utilization 53%; Instruction Throughput Limiter 85% / Utilization 35%; Control Flow 23% / 10%; L1 Cache Limiter 2,6% / Utilization 2,6%; Buffer L1 Miss Rate 18,5%; L1 Read Bandwidth 189 GiB/s; GPU Bandwidth 14,7 GB/s (mediana, max 160); LLC Utilization 11%; Register Pressure Influence 0; L1 Cache Pressure Influence 0; Memory Request Stalls Influence 0 (mediana). Wniosek: pętla sił jest ograniczona przepustowością instrukcji (jednostki F32 i integer/conditional), a pamięć prawie nie pracuje. Obłożenie 23% jest niskie, ale to nie rejestry ani pamięć (wpływy 0); ILP z 060 pomagał, bo jednostki ALU czekały na zależności.
