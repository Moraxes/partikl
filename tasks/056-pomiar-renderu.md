---
id: 56
title: Pomiar renderu
summary: 'Trwały pomiar kosztu renderowania w aplikacji: bez vsync, log czasu klatki, tryb bez symulacji'
status: todo
priority: soon
branch: compute-shader-1
part_of: 54
created: 2026-11-21T16:30:35
---
Cel: bench mierzy tylko compute, a pomiary renderu powstały z jednorazowych przełączników. Trwała opcja uruchomienia aplikacji (np. --profile-frames): AutoNoVsync, log średniego czasu klatki, wyłączenie symulacji (tryb samego renderu), przełączniki MSAA/HDR do porównań. Następny krok: po zadaniu stanu w kolejności siatki.
