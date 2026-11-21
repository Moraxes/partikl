---
id: 70
title: Ostrzeżenie init_gpu
summary: init_gpu ma 8 argumentów (clippy too_many_arguments)
status: done
priority: later
branch: main
created: 2026-11-21T22:43:25
worked: 2026-11-22T00:43:11
---
Cel: usunąć jedyne ostrzeżenie clippy w kodzie GPU; init_gpu w src/gpu/mod.rs przyjmuje 8 parametrów systemu. Następny krok: zgrupować zasoby w SystemParam (np. GpuInitParams) albo rozbić system.
