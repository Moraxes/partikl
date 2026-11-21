---
id: 49
title: Wyścig ładowania shaderów
summary: Sporadyczny błąd module not found default_rules przy starcie
status: todo
priority: later
branch: main
relates:
- 28
created: 2026-11-21T13:33:33
---
Cel: przy ładowaniu wbudowanych shaderów (load_shader_library!) step_driver bywa kompilowany, zanim moduł default_rules jest zarejestrowany: w logu błąd module not found, po chwili pipeline się kompiluje. Nieszkodliwe, ale myli przy pracy nad regułami. Następny krok: odtworzyć (kilka uruchomień testów GPU), potem kolejność ładowania albo kolejkowanie pipeline'ów po załadowaniu wszystkich modułów.
