---
id: 28
title: GPU compute port
summary: M1 portu symulacji na GPU wg plan_rozwoju_partikl_na_gpu.md (bez UI)
status: doing
priority: soon
branch: compute-shader-1
created: 2026-11-21T00:51:19
worked: 2026-11-21T12:37:45
links:
- kind: commit
  target: 4195c6a
  at: 2026-11-21T12:37:45
  note: 'fix(gpu): make the headless harness fail loudly and run any step count'
- kind: commit
  target: 70c66bb
  at: 2026-11-21T12:37:45
  note: 'build(gpu): embed the shaders in the binary, hot reload behind a dev feature'
---
Cel: M1a (szkielet GPU, render instanced) i M1b (siatka + siły na GPU, testy, bench). Next step: moduł src/gpu z buforami ping-pong.
