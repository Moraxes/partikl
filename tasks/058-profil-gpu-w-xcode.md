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
edited: 2026-11-21T16:56:31
links:
- kind: commit
  target: 2d4081b
  at: 2026-11-21T18:09:33
  note: 'perf(gpu): keep the force coefficients of a particle''s type in registers'
---
Cel: potwierdzić lub obalić hipotezę, że pętla sił nie jest ograniczona pamięcią, zanim zainwestujemy w M2 3.1 (SoA/kompresja) i 3.3 (Morton). Xcode zainstalowany (/Applications/Xcode.app), ale licencja niezaakceptowana (sudo xcodebuild -license accept, robi użytkownik); xcode-select wskazuje na Command Line Tools, więc używać DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun xctrace … (bez sudo). Metal System Trace i GPU counters: nagranie z linii poleceń (xctrace record --template …), analiza liczników najwygodniej w GUI. Następny krok: użytkownik akceptuje licencję; sprawdzić xctrace list templates.
