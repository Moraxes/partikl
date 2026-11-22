---
id: 80
title: Ctrl+C w testach GPU
summary: 'Bevy przechwytuje SIGINT w procesie testów: cargo test nie daje się przerwać'
status: todo
priority: soon
branch: main
created: 2026-11-22T21:54:14
---
Cel: Ctrl+C przerywa cargo test z testami GPU. Harness headless (src/gpu/headless.rs) buduje aplikację z DefaultPlugins, które instalują TerminalCtrlCHandlerPlugin; ten przechwytuje SIGINT w procesie testów (w logach: 'Skipping installing Ctrl+C handler as one was already installed'), więc proces przeżywa Ctrl+C, a przy wiszących testach (np. błąd kompilacji shadera: harness czeka 120 s na kroki) zostaje w tle i trzyma GPU. Następny krok: wyłączyć TerminalCtrlCHandlerPlugin w harnessie (DefaultPlugins.build().disable::<…>()), sprawdzić, że Ctrl+C kończy cargo test; rozważyć krótszy deadline na kroki, gdy pipeline zgłosił błąd kompilacji (szybki fail zamiast 120 s).
