---
id: opt-001
title: Reszta M2 optymalizacji
summary: 'Reszta M2 optymalizacji GPU: cel 1M cząstek płynnie'
status: todo
priority: soon
branch: main
created: 2026-11-22T01:12:24
---
Cel: symulacja 1M cząstek tak płynna, jak się da, z zapasem na droższe reguły. Stan (M5, bench gpu quick 100k): siły ok. 85% czasu GPU (375/525/560 µs uniform/clusters/dense), budowa siatki ok. 15% (count 22, scan 8, scatter ok. 40 µs), render 1M ok. 7 ms na klatkę. Profil 061: pętla sił ograniczona instrukcjami (F32 84%, integer 74%), nie pamięcią (L1 2,6%); przy dużych n koszt sił ∝ min(kandydaci, max_candidates=512). Plan z sesji 2026-11-22 rozbity na podzadania; kolejność wynika z blokad (tq tree).

Podział modeli (decyzja): haiku bierze zadania mechaniczne z jednoznaczną specyfikacją i wzorcem w kodzie (klawisz + flaga + HUD, uruchomienie benchów i zapis wyników). Sonnet bierze wszystko, co dotyka shaderów silnika, układu buforów, przebiegów compute albo wymaga oceny pomiaru (fuzja siatki, przycinanie, Morton, sub-stepping silnik, interpolacja renderu, rasteryzacja compute). Model jest w pierwszej linii treści każdego podzadania ('Model: haiku' / 'Model: sonnet'). Haiku: gdy pomiar wyjdzie poza oczekiwany zakres albo test jest czerwony z niejasnej przyczyny, zatrzymać się i zgłosić, nie zgadywać.

Zasady wspólne dla podzadań:
- Pomiar: cargo bench --bench gpu -- --quick raz przed zmianą (baseline z HEAD) i raz po; duże n raz na koniec punktu: cargo bench --bench gpu -- --sizes 500000,1000000 (bez --quick, bo --quick nadpisuje rozmiary). Render: cargo bench --bench render -- --modes loop --sizes 500000,1000000. Naprzemienne A/B: tools/bench-ab/ab.sh na koniec serii.
- Regresji wydajności nie naprawiać bez zgody użytkownika: zgłosić z danymi. Testy można naprawiać samemu. Spike'i mają bramkę: bez zysku zamknąć z wynikiem w zadaniu i bez commitu w main.
- Wyniki w treści zadania, przebiegi do .artifacts/bench (commit w .artifacts, tq link -k data). Commity w main: conventional, stopka 'Task: opt-NNN'.
- Wszystko headless; okna aplikacji nie uruchamiać bez zgody. Wygląd: cargo bench --bench render -- --gallery <katalog> (PNG), ocena przez użytkownika.
- Przed zakończeniem: cargo test (testy GPU headless, wyrocznia f64, testy UI) i cargo clippy --all-targets bez nowych ostrzeżeń.
