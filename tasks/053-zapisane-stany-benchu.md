---
id: 53
title: Zapisane stany benchu
summary: Readback stanu po K krokach do .cache z rewizji bazowej; A/B od tych samych stanów
status: todo
priority: later
branch: compute-shader-1
blocked_by:
- 52
created: 2026-11-21T15:00:57
---
Cel: realistyczne skupiska z prawdziwej dynamiki, te same po obu stronach A/B (jak dawne --state benchu sim). Stan po K krokach z rewizji AB_BASE zapisany w .cache/ab/states, ładowany przez obie strony. Następny krok: po stanach syntetycznych; format pliku stanu (GpuParticle jako bytes + nagłówek).
