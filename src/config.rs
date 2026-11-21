//! Stałe symulacji. Wszystkie odległości w jednostkach świata.

/// Krok czasowy symulacji (sekundy).
pub const DELTA_TIME: f64 = 0.01;
/// Kwadrat prędkości poniżej którego cząstka uznawana jest za nieruchomą.
pub const VELOCITY_THRESHOLD: f32 = 0.0001;

/// Środek trójkątnego jądra siły oddziaływania.
pub const KERNEL_MIDDLE: f32 = 30.0;
/// Szerokość (połowa podstawy) trójkątnego jądra siły oddziaływania.
pub const KERNEL_WIDTH: f32 = 10.0;
/// Zasięg oddziaływania: poza nim siła jest zerowa.
pub const INTERACTION_RADIUS: f32 = KERNEL_MIDDLE + KERNEL_WIDTH;
/// Na ile komórek siatki przestrzennej dzieli się zasięg oddziaływania: komórka ma bok
/// `INTERACTION_RADIUS / GRID_REACH`, a sąsiedztwo to `(2·GRID_REACH + 1)²` komórek. Mniejsze
/// komórki to mniej kandydatów poza zasięgiem, ale więcej i krótszych przebiegów.
pub const GRID_REACH: i32 = 2;

/// Poniżej tej odległości działa twarde odpychanie niezależne od typu cząstek.
pub const REPULSION_RADIUS: f32 = 10.0;
/// Siła twardego odpychania na jednostkę wnikania w `REPULSION_RADIUS`.
pub const REPULSION_STRENGTH: f32 = 100.0;

/// Przy uśrednianiu sił (`SimOptions::average_forces`): siła cząstki z więcej sąsiadami w
/// zasięgu niż tyle jest mnożona przez `AVERAGE_NEIGHBOURS / sąsiedzi`, więc nie rośnie z
/// gęstością. Sztywność odpychania jest wtedy co najwyżej `AVERAGE_NEIGHBOURS ·
/// REPULSION_STRENGTH`, stabilna przy `DELTA_TIME` dla wartości poniżej ok. 400.
pub const AVERAGE_NEIGHBOURS: f32 = 64.0;

/// Tryb `Overlap::AdaptiveRange`: zasięg odpychania `REPULSION_RADIUS · √(N̄ / ADAPTIVE_NEIGHBOURS)`
/// (N̄: wygładzona liczba sąsiadów, pole `crowding` cząstki), od `REPULSION_RADIUS` do `ADAPTIVE_RANGE_MAX`.
/// Przy upakowaniu heksagonalnym z odstępem `REPULSION_RADIUS` w zasięgu oddziaływania jest
/// ok. 64 sąsiadów, więc powyżej tego skupisko się rozpycha.
pub const ADAPTIVE_NEIGHBOURS: f32 = 64.0;
/// Największy zasięg odpychania w trybie zmiennym: nie dalej niż początek przyciągania
/// (`KERNEL_MIDDLE - KERNEL_WIDTH`), żeby odpychanie go nie zasłaniało.
pub const ADAPTIVE_RANGE_MAX: f32 = KERNEL_MIDDLE - KERNEL_WIDTH;
/// Waga nowej liczby sąsiadów w `crowding` na krok (stała czasowa ok. `1 / α` kroków):
/// zasięg zmienia się wolniej niż ruch cząstek, więc opóźnienie o krok nie wzbudza drgań.
pub const CROWDING_SMOOTHING: f32 = 0.02;

/// Współczynnik tarcia kwadratowego (proporcjonalnego do |v|·v).
pub const FRICTION_COEFF: f32 = 0.01;
