---
id: 26
title: Zasięg odpychania od gęstości
summary: 'Trzeci tryb nakładania: zasięg odpychania pary rośnie z wygładzoną gęstością, sztywność stała'
status: todo
priority: soon
branch: main
blocked_by:
- 22
relates:
- 23
- 25
created: 2026-11-20T23:20:35
---
Cel: trzeci tryb obsługi nakładania (obok sprężyny i rozsuwania, ten sam klawisz przełącza cyklicznie): zasięg odpychania pary `r_ij = max(r_i, r_j)` rośnie z gęstością wokół cząstki, siła na jednostkę wnikania bez zmian. Gęsta gwiazda puchnie (większe odstępy, mniej sąsiadów, słabsze przyciąganie), sztywność pary się nie zmienia. Następny krok: wygładzona gęstość jako stan cząstki, funkcja r(N̄) z ograniczeniem, sonda.

## Gęstość jako stan
- N z przejścia sił kroku t, zasięg używany w kroku t+1 (zasięg pary trzeba znać przed policzeniem siły).
- Zamiast surowego N z poprzedniego kroku: wygładzona `N̄ ← N̄ + α(N − N̄)`, z α na tyle małym, że zasięg zmienia się wolniej niż ruch cząstek (opóźnione sprzężenie zwrotne bez tego może oscylować).
- Alternatywa bez opóźnienia: osobne przejście liczące N przed siłami; mierzyć tylko, jeśli wygładzanie nie wystarczy (koszt rzędu kolejnego przejścia po kandydatach).

## Ograniczenia
- r_max < `INTERACTION_RADIUS` i najlepiej ≤ 20 (wtedy poza zasięgiem przyciągania, które zaczyna się od 20).
- Jądro: zasięg per para zamiast stałej (dodatkowy odczyt i max na wektor); `--quick` z trybem domyślnym bez zmian.
- Pomiar: sonda n=100000, `spec-2026-11-20-21-48-03.ron`, jak 024 i 025.
