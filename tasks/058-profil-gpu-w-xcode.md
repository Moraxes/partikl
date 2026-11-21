---
id: 58
title: Profil GPU w Xcode
summary: 'Liczniki GPU (Instruments/Metal): czy pętla sił jest ograniczona obliczeniami czy opóźnieniami'
status: todo
priority: later
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
edited: 2026-11-21T16:56:31
---
Cel: potwierdzić lub obalić hipotezę, że pętla sił nie jest ograniczona pamięcią, zanim zainwestujemy w M2 3.1 (SoA/kompresja) i 3.3 (Morton). Xcode zainstalowany (/Applications/Xcode.app), ale licencja niezaakceptowana (sudo xcodebuild -license accept, robi użytkownik); xcode-select wskazuje na Command Line Tools, więc używać DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcrun xctrace … (bez sudo). Metal System Trace i GPU counters: nagranie z linii poleceń (xctrace record --template …), analiza liczników najwygodniej w GUI. Następny krok: użytkownik akceptuje licencję; sprawdzić xctrace list templates.
