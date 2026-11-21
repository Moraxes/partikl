---
id: 55
title: Stan w kolejności siatki
summary: scatter zapisuje całą cząstkę, siły piszą do state[j]; render rysuje spójnie przestrzennie
status: doing
priority: soon
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
worked: 2026-11-21T16:30:35
---
Cel: bufor stanu zawsze w kolejności siatki z poprzedniego kroku, więc render (i kafelkowanie) widzi cząstki spójnie przestrzennie. Zmiany: sorted_ids → sorted_state (pełne cząstki, 32 B) w scatterze i sort_cells; forces czyta me z sorted_state[j], próbkowanie po me.id, zapisuje state[j]; Harness::run sortuje odczyt po id (testy bez zmian). Weryfikacja: testy GPU, bench (bez regresji powyżej ok. 10%, inaczej zgłosić), render 500k przed/po. Następny krok: sorted_state w bindings i buforach.
