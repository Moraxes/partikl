---
id: 75
title: Napraw timer forces
status: todo
priority: soon
branch: main
created: 2026-11-22T13:46:28
---
Znaczniki czasu ostatniego przebiegu (forces) w bench gpu/timing.rs są niewiarygodne: zaniżają koszt ~6–9x na M5 (ślad TSDUMP: ticki poza osią czasu). Wall step_ns jest wiarygodny. Zbadać przyczynę (kolejność zapytań, MAX_QUERIES, resolve) i naprawić, albo wymusić ostrzeżenie o niemonotonicznych tickach. Do czasu naprawy oceniać opt-y po wall.
