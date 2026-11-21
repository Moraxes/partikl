---
id: 46
title: Schemat cząstki
summary: Minimalny silnikowy Particle; pola modelu shared/private w buforach reguł
status: doing
priority: soon
branch: main
part_of: 35
blocked_by:
- 39
- 45
created: 2026-11-21T12:39:03
worked: 2026-11-22T00:08:15
---
Cel (poziom C): Particle silnika to pos, last, species, id. Pola modelu (crowding i przyszłe) w buforach deklarowanych przez reguły: shared do Neighbour (kopiowane w scatterze), private tylko dla integrate i renderu. Rust zna rozmiar w bajtach, bufory alokowane generycznie; miejsce na planer pamięci z M3. Następny krok: crowding jako pierwsze pole private/shared.
