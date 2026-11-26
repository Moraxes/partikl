# Observability: struktury okresowe w czasie (odłożone)

Status: odłożone 2026-11-27. Zakres pomiaru struktur przestrzennych ([plan.md](plan.md), etapy 0–4)
jest już duży. Wrócić po etapie 3 planu, bo stamtąd pochodzą potrzebne klocki: jądra, ich śledzenie
w czasie, graf jąder i deskryptory kształtu.

## Co chcemy wykrywać

- pulsowanie w miejscu: rozmiar lub kształt struktury zmienia się z okresem T;
- obrót (rotor; u boidów wir, stan „torus” z modelu Couzina);
- szybowiec: kształt wraca po okresie, ale w innym miejscu (pływak z cyklicznym ruchem, jak
  statki w Grze w życie);
- fala biegnąca wzdłuż struktury, np. łańcucha jąder;
- synchronizacja wielu struktur albo drgania całej siatki jąder.

## Warunki pomiaru

### Tożsamość w czasie: indeks Jaccarda

*Teoria:* J(A, B) = |A ∩ B| / |A ∪ B|, ułamek wspólnych elementów dwóch zbiorów. Struktury
z kolejnych migawek dopasowuje się po zbiorach id cząstek; J ≥ 0,5 oznacza tę samą strukturę.
*U nas:* id cząstek są stałe, więc nie trzeba zgadywać po położeniu. Gdy członkowie szybko się
wymieniają (wir, fala), tożsamość prowadzi się po jądrach. Śledzenie powstaje w etapie 1 planu.

### Gęstość migawek: twierdzenie Nyquista–Shannona

*Teoria:* okres T da się wykryć tylko przy migawkach częstszych niż co T/2; żeby zobaczyć kształt
cyklu, potrzeba migawki co ok. T/10. Okres krótszy niż dwa odstępy migawek udaje okres dłuższy
(aliasing).
*U nas:* `macro_stats` robi migawkę co 250 kroków (2,5 s). Tu potrzeba co 5–10 kroków, przynajmniej
w wybranych oknach. Okresy podawać w sekundach czasu symulacji, nie w krokach (podkroki skracają
krok).

### Ile danych

Pełna migawka to 100 tys. × 24 B ≈ 2,4 MB, więc 400 migawek to ok. 1 GB. Zapisywać szeregi na
poziomie jąder (środek, promień, skład, prędkość: kilka tysięcy jąder × kilkadziesiąt bajtów),
a pełne migawki tylko w krótkich oknach.

## Metody, od najprostszej

### 1. Autokorelacja i widmo mocy deskryptorów

*Teoria:* autokorelacja C(τ) mierzy, jak sygnał x(t) jest podobny do siebie przesuniętego o τ. Dla
sygnału okresowego ma maksima przy τ = T, 2T, 3T…; dla szumu szybko spada do zera. Widmo mocy to
kwadrat modułu transformaty Fouriera; z twierdzenia Wienera–Chinczyna jest transformatą
autokorelacji i pokazuje okres jako pik przy częstotliwości 1/T. Okno musi objąć kilka okresów;
metoda Welcha uśrednia widma nakładających się kawałków, żeby zmniejszyć szum.
*U nas:* sygnały na strukturę: promień bezwładności, anizotropia, długości wiązań, kąt osi głównej
(rozwinięty albo jako cos/sin).

### 2. Kształt bez względu na położenie i obrót: Kabsch i wykres rekurencji

*Teoria Kabscha:* dla dwóch układów punktów o tej samej numeracji (te same id) po nałożeniu środków
mas algorytm szuka obrotu, który najlepiej je pokrywa. W 2D to jeden `atan2` z sum iloczynów
współrzędnych: θ = atan2(Σ(xᵢy′ᵢ − yᵢx′ᵢ), Σ(xᵢx′ᵢ + yᵢy′ᵢ)). Resztkowe odchylenie (RMSD) mierzy
różnicę kształtu niezależnie od położenia i obrotu.
*Teoria wykresu rekurencji:* macierz R(t₁, t₂) = 1, gdy stan w t₁ jest bliski stanowi w t₂ (tu:
RMSD po Kabschu < ε). Ruch okresowy rysuje linie równoległe do przekątnej, odległe o okres; chaos
daje krótkie, porwane odcinki, a ruch bez powrotów samą przekątną. Ilościowo (RQA): udział punktów
na liniach ukośnych (determinizm) i średnia długość tych linii.
*U nas:* obrót i przesunięcie z Kabscha między t a t + T dają typ ruchu. Kształt wraca, a kąt
narasta: rotor (ω = kąt / T). Kształt wraca, położenie nie: szybowiec (prędkość = przesunięcie / T).

### 3. Faza i synchronizacja: transformata Hilberta i parametr Kuramoto

*Teoria:* z oscylującego sygnału (po odjęciu średniej i odfiltrowaniu innych częstotliwości)
transformata Hilberta robi sygnał analityczny, którego argument to faza φ(t), czyli miejsce
w cyklu. Parametr porządku Kuramoto R = |(1/N)·Σ e^{iφⱼ}| wynosi ok. 1, gdy oscylatory są w tej
samej fazie, i ok. 0, gdy fazy są rozrzucone. Faza rosnąca liniowo wzdłuż struktury oznacza falę
biegnącą.
*U nas:* sygnał na jądro, np. jego promień albo odległość do sąsiada w łańcuchu.

### 4. Kymograf

*Teoria:* obraz, w którym oś pozioma to pozycja wzdłuż struktury (numer jądra w łańcuchu), oś
pionowa to czas, a kolor to wybrana wielkość (np. długość wiązania). Fala biegnąca rysuje ukośne
pasy (nachylenie to prędkość), pulsowanie całości poziome, fala stojąca szachownicę, wzór
nieruchomy pionowe.
*U nas:* najtańszy sposób, żeby zobaczyć, czy łańcuch „pracuje”. Kolejność jąder bierzemy ze
szkieletu (RNG) z planu.

### 5. Cała siatka naraz: dynamiczny czynnik struktury S(k, ω)

*Teoria:* transformata Fouriera gęstości jednocześnie w przestrzeni i w czasie. Pik przy (k, ω)
z ω ≠ 0 to fala o długości 2π/|k| i okresie 2π/ω, biegnąca z prędkością ω/|k|. Piki przy ω = 0 to
wzór nieruchomy (siatka), przy k = 0 i ω ≠ 0 oscylacja całego pola.
*U nas:* nie wymaga dzielenia na struktury; rozszerzenie S(k) z etapu 2 planu o czas.

## Pułapki

- **Okres z silnika.** Próbkowanie sąsiadów powtarza się co `every` kroków
  (`src/gpu/shaders/engine/step_driver.wesl:111`); w gęstych jądrach to kilka do kilkudziesięciu
  kroków. Wykryty okres tej długości jest podejrzany. Sprawdzian: ten sam pomiar bez próbkowania
  (`set_max_candidates(u32::MAX / 2)` w `Harness`, jak w `src/gpu/tests/clusters.rs`).
- **Aliasing** przy zbyt rzadkich migawkach (wyżej).

## Pierwszy eksperyment po powrocie

1. Wybrać specyfikację z widocznym pulsowaniem lub obrotem; pomaga nagrywanie klipów bez okna
   (`src/gpu/tests/record.rs`).
2. Migawki co 5 kroków przez 2000 kroków (20 s), zapisywać szeregi jąder.
3. Autokorelacja promieni jąder i długości wiązań; kymograf najdłuższego łańcucha.
4. Powtórzyć bez próbkowania i porównać okresy.
