---
id: 1
title: Błędy ładowania specyfikacji
summary: Result zamiast panik w loading.rs, IO osobno od generowania
status: todo
priority: soon
branch: main
created: 2026-11-20T18:44:47
---
`src/loading.rs` panikuje (`expect(&format!(..))`, `unwrap` przy tworzeniu pliku) i miesza generowanie specyfikacji z zapisem `.ron`. Następny krok: przenieść IO do `spec_io.rs` zwracającego `Result` (otwarcie, parsowanie, walidacja, zapis), `generate_particle_spec` zostawić czyste, bez IO.

## Testy
- Walidacja macierzy sił (zły rozmiar wiersza → `MalformedInteractionError`).
- Testy `SpatialGrid` już są (`src/sim/grid/tests.rs`); `triangular_kernel` zniknął przy SIMD.
