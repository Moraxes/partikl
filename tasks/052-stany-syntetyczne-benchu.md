---
id: 52
title: Stany syntetyczne benchu
summary: Jednostajny, skupiony i gęsty stan generowany wprost, niezależny od fizyki
status: todo
priority: soon
branch: compute-shader-1
part_of: 50
blocked_by:
- 51
created: 2026-11-21T15:00:57
---
Cel: mierzyć silnik w kontrolowanej gęstości niezależnie od reguł (M3 zmienia trajektorie): stany jednostajny, średnio skupiony i bardzo gęsty (zbite plamy, ponad próg próbkowania 512 kandydatów), generowane z ziarna. Kolumna density w wynikach; --quick: 100k × 3 gęstości w wariancie domyślnym aplikacji (average). Następny krok: generator plam w benches/common i kolumna w schemacie.
