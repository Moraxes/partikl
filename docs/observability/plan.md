# Observability stanu symulacji: plan (makrostruktury, zakres 1+2)

Plan dla nowej sesji, spisany 2026-11-27. Całość traktujemy jako dodanie **observability**: narzędzi,
które z samego stanu (pozycje, prędkości, gatunki, id) mówią, jakie struktury są w symulacji, co je
trzyma i jak bardzo są różnorodne. Fizyki (reguł) to przedsięwzięcie nie zmienia.

- Okresowość w czasie jest odłożona: [okresowosc-w-czasie.md](okresowosc-w-czasie.md).
- Oddziaływania dalekiego zasięgu w strukturach są odłożone (sekcja 9); do observability nie są
  potrzebne.

Cel: dla danej specyfikacji (macierz i parametry) zmierzyć skłonność symulacji do tworzenia
makrostruktur i ich różnorodność. Fazy (siatki jąder, globalne stada, łańcuchy obiegające torus)
liczymy osobno od obiektów (struktur z brzegiem). Przy okazji sprawdzamy hipotezy o tym, co trzyma
łańcuchy i siatki.

## 1. Twarde warunki

- **Węzłowa struktura shaderów zostaje nienaruszona.** Reguły składają się z czystych funkcji
  w `src/gpu/shaders/rules/nodes.wesl` (domeny: para, post, całkowanie); akumulacja to zawsze ważona
  suma, normalizacja to osobny węzeł post z jawnym (suma, licznik). Wszystko, co observability doda
  po stronie GPU (bufor etykiet do rysowania, mapa `slot_of`, ewentualne redukcje), idzie do
  silnika albo renderu jako osobne przebiegi i osobne moduły, w tym samym stylu: czyste funkcje,
  jawna kolejność operacji. Kontrakt reguł (`default_rules.wesl`, nagłówek) się nie zmienia.
- **Analiza bez lustra fizyki tam, gdzie się da.** Geometria, grafy, klasy i deskryptory liczą się
  z samego stanu. Jedyny element zależny od reguł to rozkład sił (3.6); korzysta z istniejącego
  lustra CPU i jego samosprawdzenia względem GPU.

## 2. Kontekst dla nowej sesji

### Model sił

Reguły: `src/gpu/shaders/rules/default_rules.wesl`; stałe: `src/config.rs`; specyfikacje
użytkownika: `spec-*.ron` w katalogu głównym (w `.gitignore`).

- **Przyciąganie i odpychanie z macierzy** działa przez jądro trójkątne `kernel_middle ±
  kernel_width` = 30 ± 10, czyli tylko w pierścieniu 20–40 (`hat`, `nodes.wesl:21`). **Poniżej 20
  macierz nie daje żadnej siły** („martwa strefa”). Siła to suma `coeff · h · kierunek` dzielona
  przez `Σh + 2`, razy `attraction_scale` = 10. Współczynniki w specyfikacjach sięgają ±500, więc
  przyspieszenie do ok. 5000 j/s².
- **Separacja:** sprężyny od wszystkich gatunków bliżej niż 3,5, suma nasycana
  (`separation_saturation` = 4), najwyżej ok. 140 j/s².
- **Rozsuwanie** nakładających się (bliżej niż 1) to poprawka prędkości; opcje `overlap`
  i `overlap_damp`.
- **Boidy:** wyrównanie (`alignment_rate` = 1/s) i spójność (`cohesion_stiffness` = 1/s²,
  najwyżej ok. 40 j/s²) tylko z własnym gatunkiem w `vision_radius` = 40. Napęd `drive_rate` = 0
  (wyłączony). `fov_cos` jest w parametrach, ale reguły go nie używają (`in_view`, `nodes.wesl:41`,
  nie jest wołane).
- Świat to torus. W testach 3840×2160 i 100 tys. cząstek (`MS_WORLD`, `MS_N` w `macro_stats.rs`),
  krok 0,01 s.
- Próbkowanie sąsiadów: powyżej 256 kandydatów cząstka odwiedza co `every`-tego, z przesunięciem
  rotującym co krok (`src/gpu/shaders/engine/step_driver.wesl:111`).

### Infrastruktura pomiarów (`src/gpu/tests/`)

- `Harness` (`src/gpu/headless.rs`): headless GPU. `Harness::new(options, spec, initial, world)`
  przyjmuje dowolny stan początkowy; `run(steps)` zwraca stan posortowany po id; `set_substeps`,
  `set_max_candidates` (`u32::MAX / 2` = bez próbkowania).
- `macro_stats.rs`: `Grid` (siatka komórek na torusie, `for_neighbours`), `par_rows` (liczenie
  równoległe), `matrices()`, zmienne środowiskowe `MACRO_*` (`MACRO_PARAMS` nadpisuje parametry),
  wyniki w `.artifacts/`.
- `force_scales.rs`: **lustro sił na CPU** (`forces`, linia 54) z samosprawdzeniem względem GPU
  (`check_mirror`, linia 216). Odwzorowuje domyślne opcje; `overlap_damp=force` trzeba by dodać.
- `clusters.rs`: sonda hipergęstych skupisk (tysiące cząstek w promieniu separacji).
- `record.rs`: nagrywanie klipów bez okna (ffmpeg); pomaga wybrać specyfikacje z ciekawymi
  strukturami.
- Pomiary GPU są `#[ignore]` i idą przez
  `gate-run --resource cargo --resource gpu -- cargo test --release --quiet <nazwa> -- --ignored --nocapture`
  (jeden GPU, jeden katalog `target`).

### Aplikacja (pod etap 5)

- Cząstki rysuje `ParticleMaterial` (`src/gpu/material.rs`, wiązania 0–2) z shaderem
  `src/gpu/shaders/engine/render.wesl`; vertex shader czyta stan wprost z bufora (kolejność siatki,
  pole `id` dostępne).
- Lewy przycisk myszy przeciąga widok (z zablokowanym kursorem), kółko to zoom, spacja to pauza
  (`SimState::Paused`), klawisz H przełącza HUD (`src/ui/mod.rs`).
- Readback bufora: `ReadbackOnce::buffer` (`request_readback`, `src/gpu/headless.rs:530`).
- Bevy ma przycięte funkcje (`Cargo.toml`); `bevy_gizmos` nie jest włączone.

### Konwencje

Komentarze i dokumentacja po polsku; commity po polsku z prefiksem (`feat:`, `test:`, `docs:`,
`refactor:`); `rustfmt.toml` (2 spacje); hook `tools/git-hooks/pre-commit` (fmt i clippy
`-D warnings`), jeśli zainstalowany.

### Obserwacje, od których to wyszło

- Łańcuchy naprzemiennych jąder dwóch gatunków; jądra się nie stykają.
- Pola „zalane” trójkątną siatką jąder; węzły niekoniecznie jednego gatunku.
- Hipergęste jądra (`clusters.rs`).

## 3. Metody: krótka teoria

### 3.1 Dwie skale: jądra i wiązania

Macierz działa tylko w pierścieniu 20–40, więc jądra trzymane przyciąganiem stoją co najmniej ok. 20
od siebie i się nie stykają. Styk (odległość rzędu separacji, 3,5) wyznacza tylko wnętrze jądra.
Między jądrami wiązaniem jest stała odległość w zasięgu oddziaływania albo wspólny ruch.

### 3.2 DBSCAN i union-find

*DBSCAN:* grupowanie po gęstości. Punkt jest rdzeniowy, jeśli ma co najmniej `minPts` sąsiadów
w promieniu `eps`. Grupa to punkty rdzeniowe połączone łańcuchem takich sąsiedztw plus ich sąsiedzi
z brzegu; reszta to szum (gaz). Warunek `minPts` nie pozwala pojedynczym cząstkom gazu skleić dwóch
grup.
*Union-find (zbiory rozłączne):* każdy element wskazuje na rodzica, korzeń reprezentuje zbiór,
połączenie zbiorów to podpięcie korzenia pod korzeń. Ze skracaniem ścieżek operacja kosztuje
praktycznie stały czas.
*U nas:* `eps` ≈ 5, wszystkie gatunki naraz. Jądro opisują środek, prędkość, skład gatunkowy,
promień i lista id członków. W hipergęstych jądrach pomaga siatka o boku `eps/√2`: wszystkie punkty
jednej komórki są w zasięgu siebie nawzajem, więc gęstą komórkę łączy się hurtem (znany trik
szybkich wersji DBSCAN).

### 3.3 Wiązania między jądrami

*Stała odległość:* para jąder w zasięgu (środki bliżej niż ok. 45) jest związana, gdy względne
wahania odległości σ_d/⟨d⟩ w oknie τ są małe (rzędu 10%). Podobnie działa kryterium Lindemanna
w fizyce ciała stałego: kryształ topi się, gdy drgania przekraczają ok. 10–15% odległości między
sąsiadami.
*Wspólny ruch (boidy):* wyrównanie nie trzyma odległości, tylko upodabnia prędkości; działa jak
lepkość. Stado to ciecz w ruchu: członkowie się przetasowują, ale lecą razem. Wiązanie ruchowe to
para w zasięgu widzenia (40) z podobną prędkością przez większość okna.
*Siły:* do każdej krawędzi dopisujemy wkłady z rozkładu sił (3.6), osobno dla każdego typu
oddziaływania i osobno dla każdego końca.

### 3.4 Grafy bliskości: Delaunay ⊇ Gabriel ⊇ RNG ⊇ MST

*Triangulacja Delaunaya:* trójkąty, w których okręgu opisanym nie leży żaden inny punkt; krawędzie
to naturalni sąsiedzi bez wybierania promienia. Dualny diagram Woronoja dzieli płaszczyznę na
obszary najbliższe punktom; liczba boków obszaru to koordynacja (6 w siatce trójkątnej; 5 albo 7 to
defekt, para 5–7 to dyslokacja).
*Graf Gabriela:* krawędź pq istnieje, gdy w kole o średnicy pq nie leży inny punkt.
*RNG (relative neighborhood graph, Toussaint 1980):* krawędź pq istnieje, gdy nie ma punktu r
bliższego obu końcom niż one sobie nawzajem; pusty musi być soczewkowaty obszar wspólny kół
o promieniu |pq| wokół p i q. Pomyślany jako graf zgodny z tym, jak człowiek widzi kształt chmury
punktów.
*Zawierania:* minimalne drzewo rozpinające ⊆ RNG ⊆ Gabriel ⊆ Delaunay, więc wszystkie są płaskie.
Graf Urquharta (Delaunay bez najdłuższej krawędzi każdego trójkąta) przybliża RNG; dokładny RNG da
się wyznaczyć z triangulacji Delaunaya w czasie liniowym.
*U nas:*
- RNG to szkielet obiektów. W łańcuchu zostają same ogniwa (krawędź między drugimi sąsiadami
  odpada, bo środkowe jądro leży w soczewce), pierścień zostaje cyklem. Ten graf rysujemy też
  w podglądzie.
- Do siatek Gabriel albo Delaunay: na zaszumionej siatce trójkątnej RNG gubi zwykle najdłuższy bok
  każdego trójkąta i psuje ψ₆.
- Wszystkie trzy obcinamy zasięgiem ok. 45, bo inaczej łączą też odległe, odosobnione punkty.
- Na torusie: triangulacja z kopiami jąder leżących przy krawędziach świata; krawędź pamięta, przez
  który obraz torusa przechodzi.

### 3.5 Liczba nawinięć (torus)

Grupa może się zamknąć wokół świata; w rozwiniętej płaszczyźnie byłaby nieskończona. W union-find
każdy element pamięta wektor przesunięcia do korzenia. Jeśli krawędź łączy dwa elementy tej samej
grupy, a ich przesunięcia różnią się o całą szerokość lub wysokość świata, grupa obiega torus (tak
bada się perkolację, Newman i Ziff). Struktura obiegająca torus nie ma brzegu w tym kierunku, więc
jest fazą albo artefaktem skończonego świata.

### 3.6 Rozkład sił na jądra (łańcuchy sił)

Siły znamy z reguł, więc można policzyć, ile do przyspieszenia jądra A wnosi każde sąsiednie jądro.
W fizyce materiałów sypkich tak wyznacza się łańcuchy sił, czyli kontakty niosące duże obciążenie.
*U nas:* rozszerzenie lustra z `force_scales.rs` w dwóch przejściach: najpierw mianowniki (`Σh + 2`,
liczba sąsiadów stada, licznik separacji), potem przypisanie wkładu każdego sąsiada do jego jądra.
Typy: macierz (przyciąganie albo odpychanie), spójność, wyrównanie, separacja. Wektor siły na A od
B i na B od A osobno, bo macierz nie jest symetryczna. Sprawdzian: suma wkładów równa sile z lustra,
lustro zgodne z GPU (`check_mirror`). Lustro liczy po wszystkich sąsiadach, więc w gęstych jądrach
(gdzie GPU próbkuje) daje wartość oczekiwaną, nie chwilową.

### 3.7 Sieć i baza, ψₙ, wzorce, ziarna

*Sieć i baza:* krystalografia opisuje kryształ jako sieć (przesunięcia n₁·a₁ + n₂·a₂; w 2D pięć
typów: skośna, prostokątna, prostokątna centrowana, kwadratowa, heksagonalna) plus bazę, czyli
motyw powtarzany w każdym węźle. Dwa kryształy tego samego związku mają tę samą sieć i bazę,
a różnią się położeniem i orientacją; gdzie się spotykają, orientacja skacze: to granica ziaren.
*Wniosek:* typ siatki to (symetria, stała sieci, baza z gatunkami); zasięg pola nie jest cechą
typu. Pole jest fazą, jądro w węźle powtarzaną jednostką. Makrostruktura ma brzeg, za którym
otoczenie jest inne.
*ψₙ:* dla jądra z kątami θ do k sąsiadów ψₙ = (1/k)·Σ e^{inθ}. Gdy sąsiedzi leżą co 360°/n,
|ψₙ| = 1; w cieczy ≈ 0. arg ψₙ / n to orientacja lokalnej siatki (modulo 360°/n). n = 6: siatka
trójkątna, 4: kwadratowa, 3: plaster miodu, 2: łańcuch. Dla kierunków prędkości (boidy) to samo
z n = 1.
*Dopasowanie do wzorca (jak CNA i PTM w OVITO):* położenia sąsiadów porównuje się z idealnymi
wzorcami po najlepszym obrocie (w 2D jeden `atan2` z sum iloczynów współrzędnych, algorytm
Kabscha) i wybiera najlepiej pasujący, jeśli odchylenie (RMSD) jest małe. Do etykiety otoczenia
dochodzą gatunki sąsiadów.
*Ziarna:* sąsiednie jądra łączymy, gdy oba pasują do tego samego wzorca z tą samą bazą
(|ψₙ| > ok. 0,7), a orientacje różnią się najwyżej o kilka stopni. Spójne składowe to ziarna,
odrzucone krawędzie to granice. Jądra na granicach (niski |ψₙ|) dostają osobną klasę, żeby nie
udawały obiektów.
*Kryształ klasterowy (hipoteza):* w fizyce miękkiej materii (Likos, Mladek i in.) cząstki, które
odpychają się z ograniczoną siłą, a na krótkim dystansie słabo albo wcale, zbijają się w gromady
w węzłach siatki o stałej zależnej od zasięgu oddziaływania, nie od gęstości. U nas pasuje do tego
pierścień 20–40 z pustym środkiem przy ujemnym współczynniku i nasycana separacja. Siły nie są
symetryczne, więc to analogia. Test w etapie 4.

### 3.8 g(r) i S(k)

*Funkcja rozkładu radialnego g(r):* ile punktów jest średnio w odległości r od danego, w stosunku do
losowego rozmieszczenia o tej samej gęstości. Gaz: ≈ 1; ciecz: gasnące garby; kryształ: ostre piki
(w siatce trójkątnej a, √3·a, 2a). Pierwszy pik daje stałą sieci, pierwsze minimum naturalny
promień sąsiedztwa; g_AB(r) dla par gatunków.
*Czynnik struktury S(k):* kwadrat modułu transformaty Fouriera gęstości. Wzór okresowy daje piki
Bragga; siatka trójkątna o stałej a daje sześć pików na sześciokącie o promieniu 4π/(√3·a).
Szerokość piku ≈ 2π / rozmiar ziarna; wiele orientacji rozmywa piki w pierścień. Liczony osobno dla
gatunków pokazuje nadstrukturę (dodatkowe piki przy mniejszym |k|). Na torusie to FFT rastra
gęstości.

### 3.9 Boidy: porządek i stany zbiorowe

*Porządek polarny* φ = |Σvᵢ| / Σ|vᵢ| (Vicsek i in. 1995): 1, gdy wszyscy lecą w jedną stronę;
ok. 0 przy ruchu chaotycznym.
*Porządek nematyczny* |⟨e^{2iθ}⟩| (θ to kierunek prędkości): wysoki też dla ruchu w obie strony
wzdłuż jednej osi, np. pasów gatunków mijających się w przeciwnych kierunkach; wtedy polarny jest
niski.
*Porządek wirowy* m = |Σ(rᵢ − r_c) × vᵢ| / Σ|rᵢ − r_c|·|vᵢ|: 1, gdy wszyscy krążą w jedną stronę
wokół wspólnego środka.
*Stany Couzina (2002):* model ze strefami odpychania, wyrównania i przyciągania (u nas separacja,
wyrównanie, spójność) daje rój (zwarty, nieuporządkowany), wir („torus”: krążenie wokół pustego
środka), ruchomą grupę równoległą i silnie uporządkowaną grupę równoległą; o stanie decydują
zasięgi stref.
*Pasma:* w modelach typu Vicseka przy przejściu do ruchu uporządkowanego pojawiają się gęste pasma
biegnące przez cały układ (Grégoire i Chaté 2004). Pasmo obiegające torus i globalne stado to fazy;
lokalne stada i wiry to obiekty. Domeny wspólnego kierunku ruchu wydziela się jak ziarna, tylko
z ψ₁ prędkości zamiast ψ₆ wiązań.
*U nas:* wyrównanie i spójność działają tylko w obrębie gatunku, więc stada są jednogatunkowe.
Wielogatunkowe struktury ruchome wymagają macierzy (np. B przyciągany przez stado A jedzie z nim;
pościgi z niesymetrycznych współczynników). Spójność nie ma martwej strefy (działa w całym zakresie
0–40), więc poniżej 20 jest jedynym przyciąganiem, ale w obecnych specyfikacjach jest słaba
(≤ 40 j/s²) wobec macierzy (do ok. 5000 j/s²).

### 3.10 Śledzenie w czasie: indeks Jaccarda

J(A, B) = |A ∩ B| / |A ∪ B|. Jądra i struktury z kolejnych migawek dopasowujemy po zbiorach id
cząstek (J ≥ 0,5 to ta sama). Stałe id oszczędzają zgadywania po położeniu. Przy szybkiej wymianie
członków (stada) dopasowanie także po położeniu przewidzianym z prędkości.

### 3.11 Deskryptory i różnorodność

*Tensor bezwładności:* macierz 2×2 średnich iloczynów położeń względem środka; wartości własne
λ₁ ≥ λ₂ to kwadraty „długości” i „szerokości”. Rg = √(λ₁ + λ₂), anizotropia
(λ₁ − λ₂)/(λ₁ + λ₂): 0 dla koła, 1 dla odcinka.
*Dziury:* flood fill pustych komórek na małym rastrze struktury; obszary puste nieosiągalne
z zewnątrz to dziury (komórki, pierścienie).
*Asortatywność (Newman):* korelacja cech na końcach krawędzi grafu: +1 łączą się podobne, −1
różne, 0 przypadkowo. Porównanie z losowym przetasowaniem gatunków w obrębie struktury (z-score).
*Hash Weisfeilera–Lehmana:* węzły zaczynają z etykietą (gatunek dominujący jądra), potem kilka razy
zastępują ją skrótem własnej etykiety i posortowanych etykiet sąsiadów. Grafy o tej samej budowie
dostają ten sam skrót niezależnie od numeracji, co daje „morfotypy”.
*Liczby Hilla:* bogactwo (liczba typów) i e do potęgi entropii Shannona (efektywna liczba typów:
jeden typ w 90% i dziesięć po 1% daje ok. 1,7, nie 11).
*Vendi score:* to samo dla danych ciągłych. Wartości własne macierzy podobieństw par struktur
(podzielonej przez ich liczbę), z nich entropia; e do potęgi entropii to efektywna liczba wyraźnie
różnych struktur.

## 4. Architektura

- **Biblioteka analizy bez Bevy.** Zalecam osobny crate w workspace (np. `crates/analysis`). Wejście
  to zwykłe tablice (pozycje, prędkości, gatunki, id, rozmiar świata), wyjście to jądra, grafy,
  klasy, deskryptory. Konsumenci: testy `#[ignore]` (pomiar) i później aplikacja (podgląd). Zalety:
  testy na danych syntetycznych kompilują się w sekundy, a równoległe worktree nie kompilują Bevy od
  zera. Wada: przejście na workspace. Wariant prostszy: moduł `src/analysis/` w obecnym crate.
- **Rozkład sił zostaje przy lustrze** (w testach, `src/gpu/tests/`), bo potrzebuje reguł
  i specyfikacji; biblioteka dostaje gotowe wektory jako atrybuty krawędzi. Gdy podgląd na żywo
  będzie potrzebował sił, lustro trzeba przenieść do kodu aplikacji.
- **Zależności minimalne** (projekt ma przycięte zależności). Delaunay: crate `spade` albo
  `delaunator`, albo własny Bowyer–Watson. S(k): `rustfft` albo zwykła DFT w wybranych k. Wartości
  własne do Vendi: własna metoda Jacobiego (małe macierze symetryczne). `parquet` jest już
  w dev-dependencies, gdyby trzeba było zapisywać migawki.
- **Szkic typów** (do ustalenia w etapie 0):

```rust
pub struct Snapshot {
  pub step: u64,
  pub world: [f32; 2],
  pub pos: Vec<[f32; 2]>,
  pub vel: Vec<[f32; 2]>,
  pub species: Vec<u32>,
  pub id: Vec<u32>,
}
pub struct Core {
  pub members: Vec<u32>, // id cząstek
  pub center: [f64; 2],
  pub vel: [f64; 2],
  pub composition: Vec<u32>,
  pub radius: f64,
}
pub struct Edge {
  pub a: usize,
  pub b: usize,
  pub image: [i32; 2], // obraz torusa, przez który idzie krawędź
  pub dist: f64,
  pub stability: f64, // σ_d / ⟨d⟩ w oknie
  pub comoving: f64,  // udział okna z podobną prędkością
  pub forces: Option<EdgeForces>, // z rozkładu sił, po stronie testów
}
pub enum CoreClass { Isolated, Object(u32), Grain(u32), Boundary }
```

## 5. Etapy

### Etap 0: szkielet i dane syntetyczne (główna sesja)

- Crate albo moduł, typy, konwersja `GpuParticle` → `Snapshot`.
- Generator danych syntetycznych (jądro to kropla k cząstek o promieniu ok. 2, plus gaz):
  a) sam gaz; b) prosty łańcuch A-B-A-B z odstępem 20–25; c) zgięty łańcuch; d) łańcuch obiegający
  torus; e) zaszumiona siatka trójkątna jąder o mieszanych węzłach; f) dwa ziarna obrócone o 15°;
  g) pierścień jąder (dziura); h) stado: grupa o wspólnej prędkości w gazie; i) wir: pierścień
  cząstek krążących wokół środka.
- Szkielet testów akceptacyjnych z oczekiwanymi wynikami dla a–i.

### Etap 1: jądra, grafy, wiązania

1. DBSCAN na torusie (union-find, siatka `eps/√2`); jądra z opisem (3.2).
2. Śledzenie jąder między migawkami (Jaccard); zdarzenia: narodziny, zanik, połączenie, podział.
3. Grafy jąder: Delaunay z obrazami okresowymi, z niego Gabriel i RNG, obcięcie zasięgiem.
4. Wiązania: stabilność odległości w oknie τ (migawki co 25–50 kroków), wiązania ruchowe.
5. Nawinięcia w union-find na grafie wiązań.
6. Rozkład sił po stronie testów (3.6).

Kryteria: b, c, d, g dają oczekiwane grafy (łańcuch to ścieżka w RNG, pierścień to cykl, d ma
nawinięcie (1, 0)); wynik nie zmienia się po przesunięciu całego stanu po torusie; 100 tys. cząstek
poniżej ok. 1 s (release, wielowątkowo).

### Etap 2: fazy i obiekty

1. ψₙ (n = 2, 3, 4, 6) na sąsiadach z Gabriela albo Delaunaya; najlepsze n dla jądra.
2. Dopasowanie do wzorców plus gatunki sąsiadów daje etykietę otoczenia.
3. Ziarna i granice (3.7).
4. Boidy: domeny kierunku (ψ₁ prędkości), porządek polarny, nematyczny i wirowy dla składowych;
   stada (składowe wiązań ruchowych), wiry.
5. Obiekty: składowe grafu wiązań po usunięciu wnętrz ziaren i wnętrz globalnych stad; klasa
   każdego jądra.
6. Sprawdziany globalne: g(r) jąder i cząstek (też g_AB), S(k) z rastra gęstości dla gatunków.

Kryteria: e daje jedno ziarno, f dwa ziarna z orientacjami różnymi o 15°, h stado, i wir; na
prawdziwych specyfikacjach siatka nie jest jednym obiektem.

### Etap 3: deskryptory, skłonność, różnorodność, pomiar

1. Deskryptory obiektów: liczba cząstek i jąder, skład, Rg/√N, anizotropia, dziury, radialny profil
   składu, łańcuchowość (średnica grafu / liczba jąder), liście, hash WL, ruch (|V|/⟨|v|⟩, ω),
   nawinięcie, czas życia, wymiana członków.
2. Deskryptory faz: typ (n, wzorzec), stała sieci a (też a / `kernel_middle`), baza, udział
   powierzchni, rozkład rozmiarów ziaren, gęstość defektów; dla faz boidów porządek polarny
   i orientacja pasm.
3. Skłonność: φ_bound (udział cząstek w trwałych strukturach o liczności ≥ s_min),
   ⟨s⟩_w = Σs²/Σs, s_max/N, udział obiektów wielogatunkowych, asortatywność (z-score), czasy życia.
4. Różnorodność: liczby Hilla po morfotypach WL (tylko struktury trwałe), Vendi po standaryzowanych
   deskryptorach; osobno dla obiektów i faz.
5. Test `measure_structures` (`#[ignore]`) ze zmiennymi środowiskowymi na wzór `MACRO_*` (np.
   `STRUCT_SPECS`, `STRUCT_STEPS`, `STRUCT_OUT`); wyniki w `.artifacts/structures/<przebieg>/`:
   `objects.csv`, `phases.csv`, `summary.csv`.
6. Wrażliwość: na jednej specyfikacji zmiana progów (eps, τ, progi stabilności i |ψ|) o ±50%
   i porównanie wyników.

### Etap 4: testy hipotez (GPU, po kolei, przez `gate-run`)

1. **Łańcuchy:** histogram odstępów jąder (ok. 20 oznaczałoby martwą strefę); rozkład sił wzdłuż
   łańcuchów (duże przeciwne przyciągania to napięcie, prawie zero to martwa strefa); nawinięcia;
   test cięcia (usunąć cząstki jednego jądra, puścić dalej przez `Harness::new` z tym stanem,
   zmierzyć cofanie się końców); świat +10% w osi x. Napięty łańcuch utrzyma się tylko bez końców,
   czyli obiegając torus; skończony skurczy się do martwej strefy.
2. **Siatka jako kryształ klasterowy:** liczba cząstek ×0,5, ×1, ×2 przy tym samym świecie. Stała
   sieci powinna zostać, liczba cząstek w węźle rosnąć proporcjonalnie.
3. **Kto robi jądra:** `cohesion_stiffness = 0` i `alignment_rate = 0`. Czy jądra i siatki
   zostają?
4. **Artefakty próbkowania:** kluczowe pomiary powtórzone bez próbkowania
   (`set_max_candidates(u32::MAX / 2)`).
5. **Faza czy obiekt:** świat o dwukrotnej powierzchni i dwukrotnej liczbie cząstek. Obiektów
   powinno być dwa razy więcej i tej samej wielkości; fazy powinny rosnąć.

Wynik: krótka notatka `docs/observability/wyniki.md`.

### Etap 5: podgląd w aplikacji (decyzje użytkownika z 2026-11-27)

Cel: wyniki analizy widoczne i klikalne w aplikacji. Wszystkie dodatki GPU w stylu węzłowym
(sekcja 1). Etap dzieli się na kroki z punktami kontrolnymi (sekcja 5a).

#### 5.1 Inspekcja na pauzie (checkpoint 1)

1. **Analiza na pauzie.** Po wciśnięciu spacji (pauza): readback stanu (`ReadbackOnce::buffer`,
   jak `request_readback` w `src/gpu/headless.rs:530`), analiza w tle (`AsyncComputeTaskPool`),
   wynik jako zasób. Analiza bierze zatrzymany stan, więc jest dokładna.
2. **Historia do stabilności wiązań.** Stabilność odległości (3.3) wymaga okna czasu, a pauza daje
   jedną migawkę. W trakcie biegu aplikacja trzyma bufor ostatnich ok. 10 migawek (readback
   asynchroniczny co 25 kroków; tylko pozycje i prędkości po id). Bez historii (np. tuż po starcie)
   wiązania są oznaczane „bez historii” i liczone tylko z geometrii i chwilowej prędkości.
3. **Podświetlenie.** Bufor `labels[id]` (u32 na cząstkę, indeks po stałym id) jako nowe wiązanie
   `ParticleMaterial` oraz `selected` w `RenderParams`; `render.wesl` przyciemnia cząstki spoza
   wybranej struktury.
4. **Tryby kolorów** (klawisz V): gatunek (jak teraz) / struktura (losowy odcień, gaz szary) /
   klasa (gaz, obiekt, ziarno, granica) / orientacja ziarna (odcień = arg ψ₆ / 6) / kierunek ruchu.
5. **Graf** (klawisz G): `Mesh2d` z kolorami wierzchołków (`ColorMaterial`). Krawędzie to cienkie
   prostokąty: kolor to typ wiązania (stała odległość / ruchowe / oba), przezroczystość to
   stabilność. Węzły to kółka o polu proporcjonalnym do liczby cząstek, w kolorach składu. Krawędzie
   przez szew torusa w dwóch kawałkach. Bez `bevy_gizmos`.
6. **Wybór prawym przyciskiem** na strukturze: ekran → świat → najbliższe jądro (w promieniu kilku
   pikseli ekranu) → struktura albo ziarno; podświetlenie i panel. Prawy klik gdzie indziej
   odznacza.
7. **Panel** (bevy_ui, jak HUD, prawa krawędź): deskryptory wybranej struktury (liczba cząstek
   i jąder, skład, klasa, Rg, anizotropia, nawinięcie, łańcuchowość) i legenda kolorów.
8. Na checkpoint 1 kamera się nie rusza; lewy przycisk dalej przesuwa widok.

#### 5.2 Widok struktury (checkpoint 2)

1. **Kamera śledząca.** Wybór struktury przybliża i centruje widok tak, żeby prostokąt otaczający
   strukturę (po rozwinięciu z torusa) zajmował połowę ekranu w ciaśniejszym wymiarze. Widok
   sprzed wyboru jest zapamiętany.
2. **Zoom kółkiem działa dalej,** względem środka struktury. W widoku struktury skala jest ciągła
   (mnożnik `zoom_base` na ząbek), a po wyjściu wraca dyskretny `zoom_exponent` sprzed wyboru.
3. **Prawy klik poza strukturą** odznacza ją i płynnie przywraca zapamiętany widok. Prawy klik na
   innej strukturze przełącza wybór (płynny przelot).
4. **Przejścia płynne z ease-out:** 1 − (1 − t)³ przez ok. 0,4 s; położenie interpolowane liniowo,
   skala logarytmicznie (wtedy zoom „wygląda” równomiernie). Nowe przejście startuje z bieżącego
   stanu animacji, więc szybkie kliknięcia się nie szarpią.
5. **Lewy przycisk w widoku struktury nie przesuwa kamery.** Klik na elemencie wyświetla
   informacje (5.3); klik w puste miejsce zamyka kartę.
6. **Szew torusa:** struktura przecięta szwem świata rysuje się w dwóch kawałkach. Na checkpoint 2
   kamera centruje się na rozwiniętym środku; jeśli to przeszkadza, następny krok to rysowanie
   obrazów okresowych przy krawędziach (osobny przebieg renderu).

#### 5.3 Informacje o elemencie (lewy klik w widoku struktury)

Trafienie z priorytetem: węzeł (jądro) > krawędź > cząstka; promień trafienia kilka pikseli
ekranu. Trafiony element dostaje obrys (jądro: pierścień, krawędź: pogrubienie, cząstka: kółko),
a obok niego pojawia się **karta** (bevy_ui, zakotwiczona przy elemencie, odsunięta tak, żeby go
nie zasłaniać, z cienką linią prowadzącą). Karta zostaje do następnego kliku; panel boczny dalej
pokazuje całą strukturę.

- **Jądro:** numer, liczba cząstek, skład (pasek w kolorach gatunków), promień, prędkość, liczba
  sąsiadów w grafie, najlepsze n i |ψₙ|, klasa (obiekt, ziarno, granica), stabilność wiązań
  z sąsiadami.
- **Krawędź:** końce, odległość (także na tle pierścienia 20–40), typ, σ_d/⟨d⟩, udział wspólnego
  ruchu; po dodaniu sił (wstrzymane, 5.5) także wkłady sił A→B i B→A według typu.
- **Cząstka:** id, gatunek, prędkość, jądro, liczba sąsiadów w 40.

#### 5.4 Klawisze i mysz

Zajęte dziś: A, B, C, D, F, H, O, R, S, T, Y, spacja, Esc. Nowe:

| Wejście | Działanie |
|---|---|
| spacja | pauza (jak dziś); na pauzie startuje analiza |
| V | następny tryb kolorów (gatunek → struktura → klasa → orientacja ziarna → kierunek ruchu) |
| G | graf: szkielet RNG → wszystkie wiązania → wyłączony |
| Tab / Shift+Tab | następna / poprzednia struktura według liczności (wybór jak prawym klikiem) |
| L | panel boczny i legenda: pokaż / ukryj |
| prawy przycisk na strukturze | wybór (5.1), w 5.2 z przelotem kamery |
| prawy przycisk poza strukturą | odznaczenie i powrót widoku |
| lewy przycisk | poza widokiem struktury: przesuwanie jak dziś; w widoku struktury: karta elementu |
| kółko | zoom (w widoku struktury względem jej środka) |

HUD (klawisz H) wypisuje bieżący tryb kolorów i grafu, jak inne opcje.

#### 5.5 Siły w aplikacji: wstrzymane

Decyzja użytkownika (2026-11-27): siły w aplikacji czekają. W planach jest generowanie shaderów
z DSL albo interfejsu w stylu geometry nodes z Blendera; z takiego opisu da się wygenerować
zarówno reguły, jak i diagnostyczny przebieg rozkładu sił, bez ręcznego lustra w aplikacji.
Do tego czasu typy wiązań w podglądzie wynikają z geometrii i ruchu. Rozkład sił po stronie testów
(3.6, na istniejącym lustrze) zostaje, bo jest potrzebny do testów hipotez w etapie 4.

Rozważone za i przeciw (dla przyszłej decyzji):
- za: kolory wiązań według przyczyny, pół-krawędzie z niesymetrią macierzy, hipotezy widoczne na
  klikniętym łańcuchu, te same liczby co pomiar;
- przeciw: druga ręczna implementacja fizyki w kodzie aplikacji (dryf względem WGSL), lustro zna
  tylko domyślne opcje i wbudowane reguły, koszt w hipergęstych jądrach, wartość oczekiwana
  zamiast próbkowanej.

#### 5.6 Później

Analiza na żywo (co ok. 0,5 s) i węzły przypięte do cząstek-reprezentantów przez `slot_of[id]`;
kamera śledząca ruchomą strukturę; test cięcia klawiszem; zapis wybranej struktury jako przypadku
do testów headless; przewijanie historii analiz.

## 5a. Kolejność i checkpointy

Sesja **zatrzymuje się dopiero po checkpoincie 3**. Checkpointy 1 i 2 to punkty kontrolne bez
przerwy: sesja sprawdza je sama (testy, uruchomienie aplikacji, opis w notatce roboczej) i idzie
dalej. Po checkpoincie 3 opisuje całość: co działa, czego nie, jak testować, jakie klawisze, wyniki
pomiaru; potem czeka, aż użytkownik potestuje.

1. Etapy 0, 1 i 2, potem 5.1. Checkpoint 1: inspekcja na pauzie.
2. 5.2, 5.3, 5.4. Checkpoint 2: widok struktury, karty, kamera.
3. Etapy 3 i 4 (pomiar headless, testy hipotez, rozkład sił po stronie testów).
   **Checkpoint 3: stop.** Wyniki w `docs/observability/wyniki.md`.
4. 5.6 (i ewentualnie 5.5) według decyzji użytkownika po checkpoincie 3.

## 6. Podział na subagentów

### Zasady

- **Główna sesja (Opus 5.5) koordynuje:** trzyma plan, ustala interfejsy (etap 0), integruje,
  pilnuje fizyki (lustro, rozkład sił) i węzłowej struktury shaderów, interpretuje pomiary, pisze
  notatki z wynikami. Tego nie wydzielać: decyzje i interpretacja wymagają pełnego kontekstu.
- **Wydzielać zadania o jasnym wejściu i wyjściu,** testowalne na danych syntetycznych bez GPU:
  moduły biblioteki.
- **Nie zrównoleglać niczego, co używa GPU** albo `cargo test` w tym samym katalogu `target`: jeden
  GPU, blokada cargo; wszystko z GPU przez `gate-run`, po kolei.
- **Równoległe implementacje w izolowanych worktree** (`isolation: "worktree"`) mają sens tylko przy
  osobnym crate bez Bevy; inaczej każde drzewo kompiluje Bevy od zera. Bez osobnego crate
  subagenci pracują po kolei w głównym drzewie.
- **Każdy subagent dostaje:** odpowiedni fragment tego planu (sekcje 3 i 5), plik z typami
  i sygnaturami, generator danych syntetycznych, kryteria akceptacji i konwencje (komentarze po
  polsku, `rustfmt.toml`, clippy bez ostrzeżeń). Zwraca diff albo commit i krótki raport: co
  zrobił, czego nie, wyniki testów.
- **Przegląd po każdym module** świeżym kontekstem: subagent-recenzent na Opus 5.5 albo
  `/code-review`.
- Model ustawia się w definicji agenta (`.claude/agents/<nazwa>.md`, pole `model:` we
  frontmatterze obok `name`, `description`, `tools`; katalog `.claude` jest w `.gitignore`, więc
  definicje są lokalne) albo przy wywołaniu.

### Zadania i modele

| Zadanie | Model | Dlaczego |
|---|---|---|
| Koordynacja, interfejsy (etap 0), integracja, interpretacja | Opus 5.5 (główna sesja) | pełny kontekst, decyzje |
| DBSCAN, union-find, nawinięcia, śledzenie (Jaccard) | Sonnet 5.5 | dobrze opisane algorytmy, testy syntetyczne |
| Delaunay z obrazami okresowymi, Gabriel, RNG | Sonnet 5.5 | jw. |
| ψₙ, wzorce, ziarna, g(r), S(k) | Sonnet 5.5, przegląd Opus | konwencje kątów i normalizacji łatwo pomylić |
| Metryki boidów (porządek polarny, nematyczny, wirowy; stada) | Sonnet 5.5 | dobrze zdefiniowane |
| Deskryptory, hash WL, liczby Hilla, Vendi | Sonnet 5.5 | mechaniczne, dobrze zdefiniowane |
| Rozkład sił (rozszerzenie lustra) | Opus 5.5 | musi dokładnie odpowiadać WGSL; subtelne mianowniki i wagi |
| Dodatki GPU do podglądu (etap 5) | Opus 5.5 | węzłowa struktura shaderów, kontrakt silnika |
| Uruchamianie długich pomiarów GPU i streszczanie CSV | Haiku 5.5 | mechaniczne; interpretuje Opus |
| Szukanie w kodzie („gdzie jest X”) | agent Explore | tanio, nie zaśmieca kontekstu |
| Trudne debugowanie (lustro niezgodne z GPU, zagadkowe wyniki) | Opus 5.5; Fable 5.2, gdy Opus utknie | Fable najmocniejszy, ale ok. 2× droższy |
| Przegląd modułów | Opus 5.5, świeży kontekst | niezależne spojrzenie |

Ceny API (listopad 2026, za 1M tokenów wejście/wyjście): Fable 5.2 $8/$40, Opus 5.5 $4/$20,
Sonnet 5.5 $2/$10, Haiku 5.5 $0,10/$0,50.

### Kolejność i równoległość

- Etap 0: główna sesja sama.
- Etap 1: równolegle subagent A (jądra, union-find, śledzenie) i B (grafy); potem integracja
  i wiązania (główna sesja).
- Etap 2: równolegle C (ψₙ, wzorce, ziarna), D (g(r), S(k)), E (boidy); potem główna sesja:
  klasyfikacja faz i obiektów.
- 5.1 (do checkpointu 1): główna sesja (Opus): bufor etykiet i zmiany renderu w stylu węzłowym;
  subagent Sonnet może zrobić panel i siatkę grafu (`Mesh2d`) według specyfikacji.
- 5.2–5.4 (do checkpointu 2): Sonnet (kamera, animacje, trafienia, karty), przegląd Opus.
- Etap 3: F (deskryptory, różnorodność); główna sesja: test `measure_structures` i rozkład sił.
- Etap 4: po kolei (GPU); uruchomienia Haiku, interpretacja Opus.

## 7. Parametry startowe (do kalibracji)

| Parametr | Start | Uwagi |
|---|---|---|
| `eps` DBSCAN | 5 | powyżej separacji 3,5, wyraźnie poniżej odstępu jąder (~20) |
| `minPts` | 5–10 | |
| zasięg krawędzi jąder | 45 | 40 plus promienie jąder |
| okno stabilności τ | 200–500 kroków | migawki co 25–50 kroków |
| próg σ_d/⟨d⟩ | 0,1 | |
| wiązanie ruchowe | \|Δv\| < 0,3 · średnia \|v\| przez ≥ 80% okna | |
| \|ψₙ\| siatki | > 0,7 | |
| tolerancja orientacji ziarna | 5–10° | |
| Jaccard | ≥ 0,5 | |
| iteracje WL | 2–3 | |

## 8. Ryzyka i pułapki

- Hipergęste jądra (tysiące cząstek w promieniu `eps`): DBSCAN bez triku z komórkami `eps/√2` ma
  koszt kwadratowy w jądrze.
- Torus: środki jąder liczyć po rozwinięciu względem jednego członka; krawędzie z obrazem torusa.
- Lustro odwzorowuje tylko domyślne opcje (`overlap_damp=force` brak); pilnuje go `check_mirror`.
- Próbkowanie: lustro liczy po wszystkich sąsiadach, GPU w gęstych miejscach próbkuje.
- Progi są arbitralne, stąd raport wrażliwości (etap 3, punkt 6).
- GPU i cargo: pomiary po kolei; budowanie blokuje katalog `target`.

## 9. Odłożone: oddziaływania dalekiego zasięgu w strukturach

Do observability niepotrzebne; wrócić po pomiarze (da liczby do strojenia). Notatki z rozmowy:

- **A. Sygnał przez kontakty:** pole `shared` w `Model` (`//@field`); w czwartym słowie
  `sorted_hot` gatunek zajmuje cały u32, więc 16–24 bity są wolne. Przekazywać z wagą przyciągania
  (pierścień 20–40), osobno wyrównywać wewnątrz jądra (d < 20). Reguły: dyfuzja z zanikiem,
  odległość w skokach od „głowy” z sufitem, faza Kuramoto, aktywator–inhibitor. Jeden skok na krok.
- **B. Jawne wiązania:** id partnerów w prywatnych polach modelu; mapa `slot_of[id]` (ta sama co
  w etapie 5); uzgadnianie w dwóch krokach; walencja na gatunek; siła zgięcia. Uwaga o pływakach:
  przy siłach wzajemnych i tarciu liniowym środek masy stoi.
- **C. Agregaty struktur:** etykiety na GPU, redukcje do tabeli struktur, siły całego ciała.

Każdy z tych wariantów musi wejść w regułach jako węzły w stylu `nodes.wesl`.
