---
id: 1
title: Błędy ładowania specyfikacji
summary: Result zamiast panik w loading.rs, IO osobno od generowania
status: done
priority: soon
branch: main
relates:
- 42
created: 2026-11-20T18:44:47
worked: 2026-11-21T23:42:05
edited: 2026-11-21T22:43:15
---
`src/loading.rs` panikuje (`expect(&format!(..))`, `unwrap` przy tworzeniu pliku) i miesza generowanie specyfikacji z zapisem `.ron`. Następny krok: przenieść IO do `spec_io.rs` zwracającego `Result` (otwarcie, parsowanie, walidacja, zapis), `generate_particle_spec` zostawić czyste, bez IO.

## Testy
- Walidacja macierzy sił (zły rozmiar wiersza → `MalformedInteractionError`).
- Testy `SpatialGrid` już są (`src/sim/grid/tests.rs`); `triangular_kernel` zniknął przy SIMD.

Aktualizacja (2026-11-21, review backlogu): testy SpatialGrid/triangular_kernel zniknęły z CPU; zostaje walidacja macierzy (zły rozmiar wiersza). Wczytywanie .ron jest też częścią 042 (powiązane).
