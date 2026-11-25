# Benchmarki w partikl

Komendy uruchamiaj z katalogu głównego repo. Benche są dwa: `gpu` (krok symulacji, domyślny) i `render` (rysowanie bez okna). Nie uruchamiaj aplikacji z oknem.

## Szybki pomiar w trakcie pracy

1. Baseline, przed zmianą: `cargo bench --bench gpu -- --quick`
2. Po zmianie ta sama komenda.
3. Porównanie (nazwy plików względem `.artifacts/bench/`, ścieżkę wypisuje bench w linii `wyniki:`):

   ```sh
   node tools/bench-ab/compare.js '[{"before":"gpu-A.parquet","after":"gpu-B.parquet"}]'
   ```

`--quick` to 100k cząstek, macierz 23 i 100 pomiarów na sceny `uniform`, `clusters`, `dense`. Nadpisuje `--sizes` i `--seed`. Po zbudowaniu jeden przebieg trwa kilka sekund.

## Jak nie tracić czasu

- W trakcie pracy wystarczy jeden przebieg po zmianie przeciw baseline.
- Zawężaj zakres, np. `--scenes uniform`.
- Naprzemienne A/B rób dopiero na koniec: `tools/bench-ab/ab.sh [rundy=3] [argumenty benchu]`.
  - Porównuje HEAD z drzewem roboczym. `AB_BASE=<rev>` zmienia bazę (po commicie zmian jest to konieczne).
  - Dla renderu dodaj `AB_BENCH=render`.
  - Pierwsze budowanie bazy w `.cache/ab/` trwa kilka minut.
- `ab.sh` zawsze dodaje `--quick`. Duże n mierz osobno i na koniec: `cargo bench --bench gpu -- --sizes 500000,1000000`.
- Argumenty przekazane do `ab.sh` trafiają na obie strony, więc baza też musi je obsługiwać.

## Interpretacja i porządek

- Patrz na czas GPU (`gpu_step_ns`, `gpu_forces_ns`). `step_ns` zawiera narzut klatki Bevy.
- Przy pracy nad funkcją ignoruj regresję do 10%. Większą zgłoś z danymi i zapytaj; regresji wydajności nie naprawiaj bez zgody użytkownika.
- Optymalizujemy pod duże n (9000 i więcej).
- Usuń pliki z przebiegów próbnych. Dokumentujące pomiary commituj w `.artifacts` (conventional commits), linkuj przez `tq link <id> <ścieżka> -k data`, a wyniki wpisz w treść zadania. Gałęzi `artifacts` nie wypychaj.

## Rola zadania 053 („Zapisane stany benchu”)

**Zaimplementowane** (`src/gpu/state_file.rs`, `benches/gpu/main.rs`, `tools/bench-ab/ab.sh`):

- **Sceny syntetyczne (`uniform`, `clusters`, `dense`):** generowane z ziarna i przywracane przed każdym pomiarem (zadanie 052), więc obie strony A/B liczą ten sam stan.
- **Scena `evolved` z `--state`:** zapisuje stan po `--steps` krokach (domyślnie 512) do binarnego pliku `.bin` w `--state-dir` (domyślnie `.cache/gpu-states`; magic `PRTKST01`, metadane `StateMeta`, rewizja UTF-8 i bajty `GpuParticle`). W kolejnych uruchomieniach stan jest odczytywany bezpośrednio z pliku (bez rozbiegu 512 kroków) i przywracany przed każdym powtórzeniem.
  - Szybki pomiar na zapisanych skupiskach: `cargo bench --bench gpu -- --quick --state` (wymusza przeładowanie flagą `--regenerate`, samo wygenerowanie bez pomiaru przez `--generate-states`).
  - Porównanie A/B na tych samych zapisanych skupiskach: `tools/bench-ab/ab.sh 3 --state` (rewizja bazowa generuje stany do `.cache/ab/states`, a obie strony ładują ten sam plik; pamiętaj, że rewizja bazowa musi zawierać obsługę `--state` z zadania 053).
