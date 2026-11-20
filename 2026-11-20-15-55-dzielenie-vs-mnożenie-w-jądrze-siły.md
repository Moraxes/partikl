---
commit: ffe6723
---

# Dzielenie vs mnożenie w jądrze siły

Eksperyment, kod bez zmian (zostaje A). Dzielenie i pierwiastek są na M5 tanie: B daje ok. 2%
`compute_forces` tylko przy małym n, przybliżony `1/√x` jest wolniejszy.

## Metoda
Warianty `Kernel::add` w `src/sim/forces.rs` przełączane stałą, pełny bench `sim` dla każdego
tuż po sobie. Czas `compute_forces` na kandydata względem A, średnia geometryczna po macierzach
z 95% CI. Błąd względny sił względem referencji f64 (mediana / max), n=3000, 5 stanów.

- A: `dx / d`, `dy / d` i `/ KERNEL_WIDTH`, 3 dzielenia.
- B: `s = siła / d`, `s·dx`, `s·dy`, `· (1 / KERNEL_WIDTH)`, 1 dzielenie.
- C1/C2: bez `sqrt` i dzielenia: `r ≈ 1/√d²` z `vrsqrteq_f32` i 1 lub 2 krokami Newtona
  (`vrsqrtsq_f32`), `d = d²·r`, `s = siła·r`. Tylko aarch64.

## Wynik

| wariant | `compute_forces`/kandydat vs A | krok vs A | błąd (mediana / max) |
|---|---|---|---|
| A | 1 | 1 | 1,7e-7 / 8,6e-4 |
| B | 0,978 [0,965; 0,991]; n=9000 0,993 [0,981; 1,004] | 0,994 [0,979; 1,009] | 1,8e-7 / 8,5e-4 |
| C1 | 1,034 [1,021; 1,046] | 1,017 | 1,8e-5 / 3,1e-3 |
| C2 | 1,044 [1,029; 1,059] | 1,039 | 2,2e-7 / 9,0e-4 |

## Uwagi
- Wcześniejszy przebieg tego porównania (na ec4d863) jest nieważny: pętla stała wtedy na
  `memcpy` stałych z `Lanes::splat` (naprawione w 1c07039) i dzielenia ginęły w tym koszcie.
- W `wide` 1.7.1 `recip` i `recip_sqrt` na NEON to dokładne dzielenie, nie przybliżenie.
