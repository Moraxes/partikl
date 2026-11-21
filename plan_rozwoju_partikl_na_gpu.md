# Plan rozwoju `partikl`: Architektura GPU, optymalizacja i reguły jako dane

**Zakres:** platforma desktopowa (Bevy `0.20`, Rust Edition `2024`, natywny `wgpu` / WGSL).

---

## 0. Aktualny stan projektu (po ostatnich zmianach)

Repozytorium (174 commity, stan na `403a215`) dysponuje wysoce zoptymalizowanym silnikiem CPU oraz rozbudowanym zapleczem pomiarowym:

* **Architektura modułów:** czysty podział na `src/sim/` (`mod.rs`, `options.rs`, `components.rs`, `forces.rs`, `grid.rs`, `tests.rs`), `src/ui/`, `render.rs`, `spec.rs`, `loading.rs`, `config.rs`, `args.rs`, `lib.rs` i `main.rs`.
* **Rdzeń symulacji CPU:**
  * Płaska siatka przestrzenna odbudowywana co krok przez **counting sort**, komórki o rozmiarze $R/2$ z sąsiedztwem $5 \times 5$, prekomputowana tablica sąsiednich komórek oraz spłaszczone współczynniki sił.
  * Wektoryzacja SIMD (`wide`, typ `f32x8`), jednoprzejściowe wypełnianie danych bez alokacji, przesuwanie zawiniętych zakresów sąsiadów zamiast zawijania każdej pary, rotacja z prędkości bez trygonometrii.
  * Równoległość wielowątkowa z dynamicznym balansowaniem zakresów pracy, liczenie bliskich par raz dla gęstych stanów, deterministyczne sumowanie zakresów oraz próbkowanie sąsiadów powyżej $512$ kandydatów.
* **Runtime'owe opcje zachowań (`src/sim/options.rs`):**
  * Przełączane w czasie rzeczywistym tryby obsługi bliskiego dystansu / kolizji (`Overlap`): `SPRING` (domyślny), `RESOLVE` (bezpośrednie rozwiązywanie nakładania) oraz `ADAPTIVE_RANGE` (adaptacyjny promień oddziaływania zależny od lokalnego tłoku).
  * Flaga `AVERAGE` (normalizacja wypadkowej siły przez liczbę oddziałujących sąsiadów).
  * Komponent `Crowding` śledzący lokalne zagęszczenie per cząstka, odseparowany od domyślnej ścieżki sił (`!AVERAGE && OVERLAP == SPRING`) poprzez monomorfizację systemów ECS (`forces_variant::<const AVERAGE: bool, const OVERLAP: u8>()`).
* **Warsztat benchmarkowy (`benches/` i `tools/`):**
  * Zapis wyników do **Apache Parquet** (`zstd`), naprzemienne szybkie testy przed/po z możliwością wskazania dowolnej rewizji bazowej przez `AB_BASE`, raportowanie na branchu `worklog` oraz interaktywny wizualizator `bench-viz`.

---

## 1. Założenia strategiczne architektury GPU

1. **Model „GPU-first” zamiast utrzymywania dwóch równoległych silników:**
   Równoczesne rozwijanie obecnego silnika CPU (SIMD `f32x8`, deduplikacja par, monomorficzne systemy ECS) i silnika GPU jest zbyt kosztowne i spowolni eksperymentowanie z nowymi modelami zachowań. GPU staje się jedynym aktywnym silnikiem produkcyjnym. Obecny silnik CPU zostaje zamrożony jako punkt odniesienia w `benches/`, a do weryfikacji poprawności matematycznej służy minimalna (~50 linijek), czysto skalarna wyrocznia referencyjna + automatyczne testy headless `wgpu` w `cargo test`.
2. **Zero-copy na ścieżce symulacja -> obraz:**
   Cząstki przestają istnieć jako encje ECS. Ich stan żyje w płaskich buforach `ShaderStorageBuffer` w układzie *ping-pong* (odczyt z kroku $t$, zapis do $t+1$), a `render.rs` rysuje je przez GPU Instancing bezpośrednio z bufora wyjściowego compute shadera — bez kopiowania stanu cząstek na CPU.
3. **Natywny `RenderApp` + `RenderGraph` w Bevy `0.20`:**
   Bezpośrednia integracja z potokiem renderowania Bevy zamiast zewnętrznych wrapperów compute zapewnia pełną kontrolę nad barierami pamięci, `dispatchWorkgroupsIndirect`, specjalizacją pipeline'ów (`ShaderDefVal`) oraz odporność na kolejne aktualizacje Bevy.
4. **Separacja UI i telemetrycznego `Readback`:**
   Warstwa UI (`bevy_egui` + `egui_plot`) nie czyta surowych buforów cząstek — korzysta wyłącznie z zasobu metryk i próbek zasilanego asynchronicznie przez wbudowany w Bevy komponent `Readback` (opóźnienie $1\text{--}2$ klatki, niezauważalne dla oka) oraz zapisuje konfigurację wysyłaną na GPU.

---

## 2. Milestone 1: Wydajny port na GPU z separacją silnika siatki od pliku reguł

**Cel:** Postawić kompletny, szybki i czytelny silnik GPU obsługujący obecny model fizyki (`ParticleSpec`) oraz runtime'owe opcje (`SimOptions`), osiągnąć parytet funkcjonalny z CPU, zapewnić natychmiastowy hot reload reguł w WGSL i wpiąć pomiary GPU w istniejący ekosystem Parquet / `bench-viz`.

### 2.1. Selekcja optymalizacji z CPU (co przenosimy, a co odrzucamy)

| Technika z obecnego CPU | Decyzja w M1 | Uzasadnienie pod GPU |
| :--- | :--- | :--- |
| Płaska siatka przestrzenna odbudowywana przez **counting sort** | **Przenosimy** | Kanoniczny wzorzec GPU: *histogram* (`atomicAdd`) -> *prefix sum* -> *scatter*. |
| Przechowywanie cząstek w kolejności siatki | **Przenosimy** | Kluczowe dla *memory coalescing* i trafień w cache L1/L2 GPU. |
| Próbkowanie sąsiadów powyżej $512$ kandydatów | **Przenosimy** | Krytyczna ochrona przed *warp/subgroup divergence* w gęstych skupiskach. |
| Komórki $R/2$ i sąsiedztwo $5 \times 5$ | **Przenosimy** (konfigurowalne $3 \times 3$ / $5 \times 5$) | Zmniejsza liczbę fałszywych kandydatów w gęstych stanach. |
| Spłaszczona macierz `force_coeffs` | **Przenosimy** | Idealnie mieści się w *constant cache* GPU jako Uniform/Storage Buffer. |
| Przesuwanie zawiniętych zakresów sąsiadów (torus) | **Przenosimy** | Offset torusa liczony raz na komórkę sąsiednią zamiast w wewnętrznej pętli dla każdej pary. |
| Rotacja z prędkości bez trygonometrii | **Przenosimy** | Prosta macierz $2 \times 2$ z znormalizowanego wektora `vel` w vertex shaderze. |
| Warianty `SimOptions` (`SPRING`, `RESOLVE`, `ADAPTIVE_RANGE`, `AVERAGE`) i `Crowding` | **Przenosimy (w modelu GPU)** | Zamiast $6$ osobnych systemów ECS używamy jednego pipeline'u specjalizowanego przez `ShaderDefVal` (lub *uniform control flow* bez *warp divergence*), z opcjonalnym buforem `Crowding` aktywnym tylko gdy potrzebny. |
| Liczenie bliskich par raz + deterministyczne sumowanie zakresów | **Odrzucamy** | Brak natywnych `atomicAdd` na $f_{32}$ w WGSL; stosujemy model **gather** (każdy wątek liczy siłę wyłącznie dla swojej cząstki). |
| Ręczne SIMD (`wide`, $f_{32} \times 8$, stałe `const`) | **Odrzucamy** | GPU jest maszyną SIMT — piszemy czysty, skalarny WGSL dla pojedynczej cząstki. |
| Dynamiczne balansowanie zakresów pracy | **Odrzucamy** | Zastępuje je sprzętowy scheduler workgroupów GPU. |
| Prekomputowana tablica sąsiednich komórek | **Odrzucamy** | Na GPU taniej wyliczać indeksy $(c_x + d_x) \bmod W$ w rejestrach niż czytać je z VRAM. |

### 2.2. Struktura buforów i wyrównanie pamięci

Wszystkie struktury współdzielone między Rustem a WGSL oznaczamy `#[derive(ShaderType)]` (`encase` wbudowane w Bevy), eliminując błędy wyrównania pamięci:

* **`SimUniforms` (Uniform Buffer):** `world_size`, `cell_size`, `grid_dims: vec2<u32>`, `num_particles`, `num_types`, `dt`, `interaction_radius`, `max_candidates` ($512$), `step_index`, `overlap_mode`, `average_forces`.
* **`InteractionMatrix` (Uniform lub Read-Only Storage Buffer):** spłaszczona tablica `force_coeffs` o rozmiarze $T \times T$, aktualizowana wyłącznie przy zmianie konfiguracji.
* **`ParticlesPing` / `ParticlesPong` (Storage Buffers):** na etapie M1 wyrównana do $16$ bajtów struktura `Particle { pos: vec2<f32>, vel: vec2<f32>, species: u32, _pad: vec3<u32> }` (w M2 rozbijana na SoA).
* **`CrowdingPing` / `CrowdingPong` (opcjonalny Storage Buffer `array<f32>`):** odpowiednik komponentu `Crowding`, bindowany/aktualizowany tylko wtedy, gdy aktywna jest opcja wymagająca śledzenia tłoku (`ADAPTIVE_RANGE` lub diagnostyka), dzięki czemu nie obciąża wariantu domyślnego.
* **Bufory siatki (Storage Buffers):**
  * `cell_counts: array<atomic<u32>>` (rozmiar $W \cdot H$),
  * `cell_offsets: array<u32>` (wynik *prefix sum*, długość $W \cdot H + 1$),
  * `cell_write_cursors: array<atomic<u32>>` (kopia `cell_offsets` inkrementowana podczas *scattera*).
* **`StatsBuffer` (mały Storage Buffer + Bevy `Readback`):** suma energii kinetycznej, maksymalna prędkość/siła, średnie `Crowding`, licznik wykrytych `NaN`/`Inf` (kluczowy do debugowania shaderów) oraz wycinek np. $512$ próbek cząstek do wykresów fazowych.

### 2.3. Przebiegi w `RenderGraph` (1 krok symulacji + render)

1. **`clear_grid`:** zerowanie `cell_counts` (`encoder.clear_buffer`).
2. **`count_cells`:** każdy wątek wyznacza indeks komórki dla cząstki $i$ w buforze `in` i wykonuje `atomicAdd(&cell_counts[cell], 1u)`.
3. **`prefix_sum`:** wielopoziomowy, hierarchiczny **Blelloch Prefix Sum** w pamięci `var<workgroup>` (bezpieczny, pozbawiony ryzyka zakleszczeń *decoupled look-back*). Dla siatki $128 \times 128$ ($16\,384$ komórek) wystarczają $2\text{--}3$ szybkie przebiegi (scan bloków -> scan sum bloków -> dodanie offsetów bloków). Kopiuje wynik do `cell_offsets` i `cell_write_cursors`.
4. **`scatter_particles`:** każdy wątek pobiera slot `dst = atomicAdd(&cell_write_cursors[cell], 1u)` i przepisuje stan cząstki z `in[i]` do `sorted[dst]`.
   * *Uwaga o determinizmie:* kolejność cząstek wewnątrz jednej komórki zależy od harmonogramu `atomicAdd`, więc przez niełączność $f_{32}$ trajektorie nie są deterministyczne bit-w-bit (fizycznie pozostają identyczne). To świadomy kompromis dla wydajności M1; opcjonalny tryb deterministyczny bit-w-bit dodajemy w M2.
5. **`compute_and_integrate`:** każdy wątek bierze swoją cząstkę z `sorted[i]`, wyznacza komórkę $(c_x, c_y)$, iteruje po sąsiedztwie $5 \times 5$ wyliczając indeksy i przesunięcia torusa w locie, stosuje próbkowanie powyżej progu $512$ kandydatów, wywołuje funkcje z modułu reguł, integruje ruch i zapisuje wynik do bufora wyjściowego (`out[i]`).
6. **`reduce_stats` (co $k$ klatek):** lekki pass agregujący metryki do `StatsBuffer` i zlecający asynchroniczny `Readback` na CPU.
7. **Render pass (`render.rs`):** pojedyncze wywołanie *instanced draw* czytające `pos`, `vel` i `species` bezpośrednio z bufora `out`, z macierzą obrotu $2 \times 2$ liczoną z `vel` bez trygonometrii.

### 2.4. Separacja silnika siatki od pliku reguł (`rules.wgsl`)

Już w M1 rozdzielamy „hydraulikę” siatki od fizyki modelu za pomocą modułów **WESL** / `#import` w Bevy `0.20` oraz włączamy feature **`file_watcher`**:

* `assets/shaders/engine/` — stabilny silnik siatki: `types.wgsl`, `grid_build.wgsl`, `prefix_sum.wgsl`, `step_driver.wgsl`, `render.wgsl`.
* `assets/shaders/rules/default_rules.wgsl` — wymienny moduł reguł eksportujący kontrakt:
  * `struct RuleAccum { ... }` (np. `force: vec2<f32>`, `neighbor_count: f32`, `overlap_corr: vec2<f32>`),
  * `fn accum_init() -> RuleAccum`,
  * `fn pair_interact(self_p: Particle, other_p: Particle, delta: vec2<f32>, dist_sq: f32, accum: ptr<function, RuleAccum>)`,
  * `fn integrate_particle(self_p: Particle, accum: RuleAccum, dt: f32) -> Particle`.
* **Specjalizacja opcji bez narzutu na ścieżkę domyślną:** zamiast pisać $6$ oddzielnych systemów jak na CPU, klucz pipeline'u compute w Bevy zawiera maskę aktywnych opcji (`ShaderDefVal` dla `OVERLAP_MODE` oraz `AVERAGE_FORCES`). Gdy w UI zmienisz opcję, Bevy buduje/pobiera z cache skompilowany wariant shadera bez martwego kodu w pętli wewnętrznej.
* **Hot reload:** zapisanie zmian w `rules.wgsl` przeładowuje pipeline w ułamku sekundy bez rekompilacji Rusta i bez przerywania działania aplikacji.

### 2.5. Weryfikacja, benchmarki (`AB_BASE`) i podstawowe UI

* **Testy headless `wgpu` w `cargo test`:** weryfikacja $1$ kroku GPU względem skalarnej wyroczni CPU (tolerancja $10^{-4}$) dla wszystkich wariantów `SimOptions` (`SPRING`, `RESOLVE`, `ADAPTIVE_RANGE`, `AVERAGE`) oraz testy niezmienników (`nan_count == 0`, zawijanie torusa, zachowanie liczby cząstek po *scatterze*).
* **Benchmarki GPU w Parquet + `AB_BASE`:** dodanie benchmarku GPU wykorzystującego *GPU timestamp queries* z `wgpu` (osobne czasy dla `count`, `prefix_sum`, `scatter`, `forces`), zapisującego wyniki do Parquet (`zstd`). Pozwoli to używać `AB_BASE` w `tools/` do porównywania kolejnych usprawnień GPU względem ustalonego commita bazowego GPU oraz względem baseline'u CPU w `bench-viz`.
* **Minimalne UI (`src/ui`):** podpięcie `bevy_egui` + `egui_plot` zasilanych z `Readback` (przełączniki `SimOptions`, czas kroków GPU, energia kinetyczna, średnie `Crowding`, wykres fazowy, alert `NaN`).

**Definition of Done (M1):**
* Działająca symulacja $100\,000+$ cząstek na GPU w $60\text{ FPS}$ z zero-copy instancingiem i pełnym wsparciem dla `SimOptions`.
* Zmiana wzoru w `rules.wgsl` widoczna na ekranie w $< 1\text{ s}$ przez `file_watcher`.
* Przechodzące testy headless `wgpu` oraz raporty GPU w `bench-viz` z obsługą `AB_BASE`.

---

## 3. Milestone 2: Optymalizacje typowo pod GPU

**Cel:** Usunąć wąskie gardła charakterystyczne dla GPU — przede wszystkim **przepustowość pamięci VRAM** oraz **rozbieżność wątków/komórek** — weryfikując każdą zmianę osobno w `bench-viz` (z `AB_BASE`) na stanach rzadkich, średnich i bardzo gęstych.

### 3.1. Rozbicie na SoA i izolacja danych opcjonalnych (priorytet #1)

Lekcja z commitu `403a215` (*utrzymywanie pracy specyficznej dla opcji poza domyślną ścieżką sił*) ma na GPU bezpośredni odpowiednik w układzie pamięci:

* **`NeighborHot` (czytany w pętli wewnętrznej dla każdego sąsiada):**
  * Wariant bazowy $12\text{ B}$ / $16\text{ B}$: `pos: vec2<f32>, species: u32`.
  * Wariant skompresowany $8\text{ B}$: pozycja znormalizowana do $u_{16}$/$f_{16}$ w przestrzeni świata (lub relatywna względem narożnika komórki) + `species` w $u_{16}$. Redukuje ruch w VRAM podczas liczenia sił nawet o $50\text{--}65\%$.
* **`ParticleCold` (czytany i zapisywany raz na cząstkę w fazie integracji i renderu):** `vel: vec2<f32>` oraz ewentualne pola prywatne (jak `crowding: f32`).
* **Izolacja buforów opcjonalnych:** bufor `Crowding` nie jest czytany w pętli sąsiadów, a w wariancie domyślnym (`SPRING` + `!AVERAGE`) nie jest w ogóle aktualizowany.

### 3.2. Dyspozycja *cell-centric* (`dispatchWorkgroupsIndirect`) + *tiling* w `var<workgroup>` (priorytet #2)

* **Problem podejścia $1\text{ wątek} = 1\text{ cząstka}$ z M1:** pojedyncza grupa robocza (np. $64$ wątki) może obejmować cząstki z kilku różnych komórek, przez co wątki czytają różne zakresy sąsiadów i nie mogą efektywnie dzielić pamięci współdzielonej.
* **Rozwiązanie (*cell-centric*):**
  1. Po `prefix_sum` lekki compute pass przegląda `cell_counts` i generuje płaską tablicę kafelków pracy `WorkTile { cell_idx: u32, start: u32, count: u32 }`, dzieląc każdą niepustą komórkę na porcje po maks. `WORKGROUP_SIZE` (np. $64$) cząstek oraz zapisując liczbę kafelków do bufora argumentów dla `dispatchWorkgroupsIndirect`.
  2. W `compute_and_integrate` jedna grupa robocza odpowiada dokładnie za jeden kafelek z **jednej komórki**.
  3. Wszystkie $64$ wątki w grupie współdzielą dokładnie te same $25$ komórek sąsiednich. Wspólnie (po $1$ sąsiedzie na wątek) wczytują paczkę $64$ sąsiadów z `NeighborHot` do ultraszybkiej pamięci na chipie **`var<workgroup>`**, wykonują `workgroupBarrier()`, a następnie każdy wątek liczy interakcje z całej paczki bez dotykania VRAM.
* **Zachowanie kontraktu z M1:** cała logika kafelków i `var<workgroup>` pozostaje zamknięta wewnątrz `engine/step_driver.wgsl` — interfejs `pair_interact` w `rules.wgsl` nie ulega zmianie.

### 3.3. Indeksowanie komórek krzywą Mortona (Z-order curve)

* Zastąpienie liniowego indeksu komórki $c_y \cdot W + c_x$ przeplotem bitów (**Morton code** / Z-order) przy potęgowych wymiarach siatki (lub kafelkowaniu blokami $8 \times 8$).
* Sąsiednie komórki w osi $Y$ leżą blisko siebie w przestrzeni adresowej VRAM, co podnosi współczynnik trafień w cache L2 GPU dla sąsiedztwa $5 \times 5$ i $3 \times 3$.

### 3.4. Sub-stepping, strojenie siatki i wizualizacje 100% GPU

* **Sub-stepping w jednym Command Encoderze:** możliwość zakolejkowania $K$ kroków fizyki (np. $2\text{--}8$) na jedną klatkę obrazu bez synchronizacji z CPU i z mniejszym $\Delta t$ dla stabilności numerycznej (szczególnie przydatne dla trybu `RESOLVE`).
* **Strojenie parametrów pod sprzęt (weryfikowane przez `AB_BASE`):**
  * Porównanie **komórki $R$ ($3 \times 3$)** vs **komórki $R/2$ ($5 \times 5$)** w pełnym przekroju gęstości (przy niskiej gęstości $9$ odczytów metadanych może wygrać z $25$).
  * Dobór `WORKGROUP_SIZE` ($32$ / $64$ / $128$).
* **Wizualizacje bez readbacku:**
  * Mapy ciepła (gęstość, `Crowding`, energia, diagramy fazowe o wysokiej rozdzielczości) rysowane bezpośrednio przez compute shader do tekstury GPU i wyświetlane w UI.
  * Podgląd wektorów sił na cząstkach jako opcjonalny pass *instanced lines* czytający wprost z bufora GPU (zamiast `bevy_gizmos`).
* **Opcjonalny tryb deterministyczny bit-w-bit:** przełącznik zastępujący atomowy *scatter* deterministycznym **GPU Radix Sortem** po indeksie komórki (do testów regresji trajektorii i powtórek).

**Definition of Done (M2):**
* Zmierzone w `bench-viz` (względem `AB_BASE` z M1) wyraźne przyspieszenie na stanach średnich i gęstych (cel: płynne $60\text{ FPS}$ dla $500\,000\text{--}1\,000\,000$ cząstek na GPU klasy średniej).
* Pełne zachowanie separacji między `engine/` a `rules.wgsl`.

---

## 4. Milestone 3: Reguły jako dane — wsparcie dla poziomów A, B i C

**Cel:** Uczynić z `partikl` kompletne środowisko do projektowania modeli zachowań, w którym definicja symulacji — parametry, wzory matematyczne oraz schemat stanu i cyklu życia cząstek — zapisana jest jako **dane** (ewolucja obecnych plików `.ron` + edytor w UI) i automatycznie kompilowana do wydajnego WGSL, uogólniając koncepcję wariantów `SimOptions` na dowolne modele.

### 4.1. Architektura `ModelSpec` -> Kompilator WGSL -> Dynamiczny Pipeline

Zamiast ciężkiego `rust-gpu` (wymagającego `nightly` i psującego szybki workflow), budujemy w stabilnym Ruście `2024` lekki **kompilator modeli do WGSL**:

1. **Format `ModelSpec` (`.ron`):** rozszerzenie obecnego `ParticleSpec` o deklaratywne sekcje:
   * `schema` (Poziom C: pola cząstki `shared`/`private`, cykl życia, wiązania),
   * `tables` (Poziom A: parametry skalarne, macierze $T \times T$, krzywe LUT 1D),
   * `rules` (Poziom B: wyrażenia/AST interakcji, akumulatory, integracja i mapowanie wyglądu),
   * opcjonalny *escape hatch* `raw_wgsl: Option<PathBuf>` dla zupełnie niestandardowych shaderów pisanych ręcznie.
2. **Walidator i planer pamięci (Layout Planner):**
   * Wylicza bezpieczne wyrównanie i automatycznie rozdziela pola cząstki na strumienie SoA (`NeighborHot` vs `ParticleCold`) zgodnie z regułami WGSL.
   * Sprawdza typy wyrażeń (`f32`, `vec2<f32>`, `u32`, `bool`), zakresy indeksów i nazwy pól przed kompilacją w `wgpu`/`naga`, zwracając czytelne błędy z numerem linii/wyrażenia do panelu UI.
3. **Generator WGSL (Codegen):**
   * Generuje struktury buforów, kod *scattera* kopiujący zadeklarowane strumienie SoA, wyspecjalizowane funkcje reguł (`pair_interact`, `integrate_particle`) bez nieużywanych gałęzi oraz opcjonalny pass redukcji statystyk.
   * Łączy wygenerowany moduł z niezmiennym silnikiem siatki z M2 (*cell-centric* + `var<workgroup>`).
4. **Menedżer hot-swapu pipeline'ów:**
   * Klasyfikuje każdą zmianę w `ModelSpec` i wykonuje minimalną wymaganą operację:
     * **Zmiana Poziomu A:** zapis Uniform/Storage Buffera w bieżącej klatce ($0\text{ ms}$ pauzy, bez rekompilacji).
     * **Zmiana Poziomu B:** asynchroniczna rekompilacja samego compute pipeline'u i podmiana w locie z **zachowaniem aktualnego stanu cząstek w VRAM**.
     * **Zmiana Poziomu C:** realokacja buforów GPU + rekompilacja pipeline'ów (z automatyczną migracją wspólnych pól jak `pos`/`vel`/`species` lub czystym resetem).

### 4.2. Poziom A: Parametry, macierze i krzywe sił (LUT)

* **Dowolna liczba nazwanych parametrów skalarnych** (np. `friction`, `beta`, `max_speed`, `temp`) z metadanymi dla UI (`min`, `max`, skala liniowa/logarytmiczna) — automatycznie generowane suwaki w `bevy_egui`.
* **Wiele macierzy typ $\times$ typ** (nie tylko pojedyncza `force_coeffs`, lecz np. `attraction`, `radius`, `repulsion`, `alignment`) — edytowane jako interaktywne siatki/heatmapy w UI.
* **Krzywe oddziaływań $F(r)$ jako LUT 1D (Lookup Tables):** tablica np. $128\text{--}256$ próbek $f_{32}$ (lub tekstura 1D ze sprzętową interpolacją liniową) definiowana punktami kontrolnymi w `.ron` i edytowana myszą w `egui_plot`. Pozwala badać dowolne profile potencjałów (Lennard-Jones, wielostudnie, strefy martwe) bez zmiany kodu shadera.

### 4.3. Poziom B: Wzory matematyczne i logika zachowań (mini-DSL / AST)

* **Czytelny język wyrażeń** osadzony w `.ron`, parsowany do AST i kompilowany $1:1$ do WGSL:
  * Typy: `f32`, `vec2`, `u32`, `bool`.
  * Kontekst `pair_interact`: `self.*`, `other.*`, `delta`, `dist`, `dist_sq`, `r_norm`, `dir`, odczyty `matrix(name, self.species, other.species)` oraz `lut(name, r_norm)`.
  * Bezpieczne prymitywy matematyczne (`safe_norm`, `clamp`, `smoothstep`, `mix`, `select`) eliminujące ryzyko dzielenia przez zero i `NaN`.
* **Rozszerzalne akumulatory (uogólnienie `AVERAGE` i `Crowding`):**
  * W definicji reguł można zadeklarować dowolne zmienne akumulowane w pętli po sąsiadach (np. `force: vec2`, `avg_vel: vec2`, `neighbor_count: f32`, `overlap_corr: vec2`, `heat_flux: f32`, `min_dist: f32`) z operacjami redukcji (`sum`, `min`, `max`).
  * Obecne opcje z `src/sim/options.rs` stają się dzięki temu zwykłymi definicjami w danych:
    * `AVERAGE` -> dzielenie `accum.force` przez `max(accum.neighbor_count, 1.0)`,
    * `ADAPTIVE_RANGE` -> skalowanie promienia oddziaływania przez `self.crowding`,
    * `RESOLVE` -> akumulacja wektora korekcji kolizji w `pair_interact` i zastosowanie go w `integrate`.
  * Otwiera to też natychmiast drogę do **boidów** (alignment/cohesion/separation), uproszczonych modeli **SPH**, dyfuzji ciepła/substancji czy synchronizacji fazowej (model Kuramoto).
* **Programowalne reguły integracji i renderowania:**
  * `integrate`: pełny dostęp do `self.*`, `accum.*`, `dt` oraz deterministycznego PRNG per cząstka (`rand_f32()` z hasha `(id, step_index)`) do ruchów Browna i szumu termicznego.
  * `render`: wyrażenia mapujące dowolne pola cząstki na kolor, rozmiar i jasność (np. wizualizacja `crowding`, energii lub ładunku zamiast wyłącznie `species`).

### 4.4. Poziom C: Zmiany strukturalne stanu, cykl życia i topologia

Najbardziej złożone mechanizmy GPU realizujemy jako trzy ortogonalne, włączane deklaratywnie moduły (zero narzutu, gdy model z nich nie korzysta):

1. **Dynamiczny schemat pól cząstki (`schema.fields`):**
   * Każde pole w `.ron` deklaruje typ (`f32`, `vec2`, `u32`) oraz klasę dostępu:
     * **`shared` (gorące):** widoczne dla sąsiadów (`other.charge`, `other.heading`, `other.phase`) — kompilator pakuje je do `NeighborHot` i uwzględnia w `var<workgroup>` tilingu z M2 (z ostrzeżeniem w UI, gdy rozmiar `NeighborHot` przekracza budżet pamięci współdzielonej).
     * **`private` (zimne):** używane tylko przez samą cząstkę w `integrate` i `render` (np. `self.crowding`, `self.energy`, `self.age`) — trafiają do `ParticleCold`, nie obciążając przepustowości pętli sąsiadów.
   * Kompilator automatycznie generuje layout buforów, kod *scattera*, inicjalizację początkową oraz histogramy wybranego pola w `Readback` + `egui_plot`.
2. **Dynamiczna liczba cząstek i reakcje (`schema.lifecycle`):**
   * **Mutacja typu ($N = \text{const}$):** dostępna bezpośrednio w Poziomie B/C (`self.species = ...`), zero dodatkowych przebiegów.
   * **Śmierć i narodziny/podział (`lifecycle: true`):**
     * Prealokacja buforów do sufitu `max_particles` (np. $2\times$ lub $4\times$ początkowe $N$).
     * W fazie `integrate` cząstka może oznaczyć się jako martwa (`kill()`) albo zgłosić utworzenie potomka (`spawn(...)`) przez atomowe dopisanie do `spawn_queue` (`atomicAdd(&spawn_count, 1u)` z ochroną przed przepełnieniem `max_particles`).
     * Dodatkowy przebieg **stream compaction** (ponowne użycie istniejącego modułu Blelloch Prefix Sum po masce `alive`) + doklejenie zawartości `spawn_queue`, aktualizujące `active_particle_count` dla `dispatchWorkgroupsIndirect` w kolejnym kroku.
3. **Trwałe wiązania między cząstkami (`schema.bonds` — opcjonalna warstwa grafowa):**
   * Dla modeli tworzących łańcuchy, błony lub struktury wielocząstkowe:
     * Każda cząstka otrzymuje stabilne `particle_id: u32`, a faza *scatter* aktualizuje tablicę `id_to_sorted_idx[id] = dst`.
     * Bufor wiązań o stałym maksymalnym stopniu wierzchołka `MAX_BONDS` (np. $4$ lub $6$ slotów na cząstkę: `(target_id, rest_length, strength)`), bez dynamicznych alokacji na GPU.
     * Dedykowany compute pass `resolve_bonds` przed `integrate`, liczący siły wiązań przez odczyt `id_to_sorted_idx[target_id]` oraz opcjonalne zrywanie lub tworzenie wiązań.

### 4.5. Warsztat iterowania i diagnostyki (UI + `rerun`)

* **Hot reload plików `.ron` przez `file_watcher`:** zapis pliku modelu aktualizuje parametry (A), przeładowuje logikę bez resetu stanu (B) lub przebudowuje bufory (C).
* **Panel inspekcji w `bevy_egui`:**
  * podgląd wygenerowanego kodu WGSL i błędów walidacji DSL,
  * edytor parametrów, macierzy i krzywych LUT w `egui_plot`,
  * detektor anomalii (natychmiastowy alert i pauza przy `nan_count > 0` z numerem kroku).
* **Opcjonalny eksport do `rerun`:** streamowanie serii czasowych i próbek fazowych z `Readback` do zewnętrznego viewera `rerun` (oś czasu z przewijaniem wstecz do głębszej analizy emergentnych wzorców).

**Definition of Done (M3):**
* Co najmniej $3$ istotnie różne modele zdefiniowane wyłącznie jako pliki `.ron` (bez zmiany kodu Rusta ani ręcznego WGSL):
  1. Klasyczne *particle life* z krzywymi LUT i opcjami odpowiedników `SPRING`/`RESOLVE`/`ADAPTIVE_RANGE`/`AVERAGE` (test Poziomu A + B),
  2. Boidy lub model z orientacją/dyfuzją i polami `shared` (test Poziomu B + C),
  3. Model z energią, rozmnażaniem/śmiercią lub wiązaniami (test Poziomu C).
* Edycja parametrów i krzywych w czasie rzeczywistym w $60\text{ FPS}$ oraz przeładowywanie logiki z pliku `.ron` w $< 1\text{ s}$.

---

## 5. Macierz ryzyk i rekomendowana kolejność prac

| Ryzyko | Etap | Działanie zapobiegawcze |
| :--- | :--- | :--- |
| Ciche błędy wyrównania pamięci Rust <-> WGSL | M1, M3 | Wyłączne użycie `#[derive(ShaderType)]` (`encase`) w M1; automatyczne testy layoutu i rozmiarów buforów w generatorze M3. |
| „Zatrucie” symulacji przez pojedyncze `NaN` | M1--M3 | Stały atomowy licznik `nan_count` w `StatsBuffer` + bezpieczne prymitywy (`safe_norm`, `eps`) w silniku i generatorze DSL. |
| Rozmnożenie wariantów shaderów przy rosnącej liczbie opcji | M1, M2 | Specjalizacja przez `ShaderDefVal` z leniwym cache'owaniem pipeline'ów w Bevy (zero narzutu w wariancie domyślnym, brak duplikacji kodu). |
| Przeinżynierowanie DSL w M3 zanim powstaną realne potrzeby | M3 | Najpierw w M1/M2 przetestować $2\text{--}3$ nowe modele ręcznie w `rules.wgsl` z `file_watcher` — dopiero powtarzalne wzorce przenieść do AST/DSL w M3. |
| Przekroczenie budżetu `var<workgroup>` przy wielu polach `shared` | M2, M3 | Planer pamięci w M3 kontroluje bajty na sąsiada i przy zbyt ciężkim `NeighborHot` automatycznie zmniejsza rozmiar kafelka lub wyłącza tiling. |

### Rekomendowany rytm wdrożenia (przekrój pionowy)

1. **M1a:** Szkielet `RenderApp` + `RenderGraph` z buforami *ping-pong*, zerową fizyką (ruch jednostajny) i zero-copy instancingiem w `render.rs`.
2. **M1b:** Pełna siatka na GPU (`count` -> Blelloch `prefix_sum` -> `scatter`) + `step_driver.wgsl` z wyodrębnionym `rules.wgsl` (`file_watcher`), obsługa `SimOptions` przez `ShaderDefVal`, testy headless `wgpu` przeciwko wyroczni skalarnej CPU oraz benchmark GPU -> Parquet z obsługą `AB_BASE`.
3. **M1c:** `reduce_stats` + Bevy `Readback` + panel sterowania i wykresów `bevy_egui` / `egui_plot`.
4. **M2 (iteracyjnie, commit po commicie z pomiarami względem `AB_BASE`):** SoA (`NeighborHot` / `ParticleCold` + izolowany `CrowdingBuffer`) -> *cell-centric* `dispatchWorkgroupsIndirect` + `var<workgroup>` tiling -> krzywa Mortona -> sub-stepping i wizualizacje 100% GPU (mapa ciepła / wektory sił) + opcjonalny GPU Radix Sort.
5. **M3 (warstwa po warstwie):** Poziom A (rozszerzony `.ron` + macierze + LUT 1D w UI) -> Poziom B (AST wyrażeń + akumulatory uogólniające `AVERAGE`/`RESOLVE` + generator WGSL) -> Poziom C (dynamiczne pola `shared`/`private`, następnie `lifecycle` / *stream compaction*, a na końcu opcjonalne `bonds`).