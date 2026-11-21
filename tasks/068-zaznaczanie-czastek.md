---
id: 68
title: Zaznaczanie cząstek
summary: 'Zaznaczanie cząstki na GPU: klik wybiera cząstkę, sąsiedzi podświetleni (po usunięciu CPU zniknęło)'
status: todo
priority: later
branch: main
created: 2026-11-21T22:43:25
---
Cel: wrócić do zaznaczania cząstki kliknięciem i podświetlania jej sąsiadów, teraz bez encji na cząstkę. Pomysły: pick w compute (najbliższa cząstka do kursora po siatce, wynik w małym buforze) i readback jednej wartości; podświetlenie w render.wesl po indeksie wybranej cząstki i dystansie. Następny krok: zdecydować mechanizm (pick w compute + uniform wybranego id), napisać test jakościowy (klik zaznacza, sąsiedzi podświetleni, klik w tło chowa). Zob. dawne 003.
