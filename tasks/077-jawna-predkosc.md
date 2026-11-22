---
id: 77
title: Jawna prędkość
summary: 'Particle: vel zamiast last (leapfrog); bez rescale i dt_ratio'
status: todo
priority: soon
branch: main
created: 2026-11-22T18:37:33
---
Cel: prędkość jako pole cząstki zamiast (pos − last)/dt: pełna precyzja prędkości przy podkrokach (dziś rozdzielczość ok. 0,012 j/s przy dt 0,01, gorsza ×K), zmiana dt bez przebiegu rescale, reguły zależne od prędkości (boidy) czytają vel wprost. Stabilność bez zmian (ta sama metoda w dokładnej arytmetyce), narzut oczekiwany ~0 (Particle zostaje 24 B). Implementuje użytkownik, Claude prowadzi. Kolejność: (1) Particle/GpuParticle + initial_particles, (2) całkowanie w default_rules (tarcie kwadratowe, velocity_threshold do omówienia; diff do przeglądu przed 3), (3) wyrocznia f64, (4) wrap, render, stats, (5) usunięcie rescale/dt_ratio/When::Rescale, (6) A/B ab.sh.
