---
id: 70
title: Ostrzeżenie init_gpu
summary: init_gpu ma 8 argumentów (clippy too_many_arguments)
status: todo
priority: later
branch: main
created: 2026-11-21T22:43:25
---
Cel: usunąć jedyne ostrzeżenie clippy w kodzie GPU; init_gpu w src/gpu/mod.rs przyjmuje 8 parametrów systemu. Następny krok: zgrupować zasoby w SystemParam (np. GpuInitParams) albo rozbić system.
