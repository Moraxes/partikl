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
pub const GRID_REACH: i32 = 1;

/// Poniżej tej odległości działa twarde odpychanie niezależne od typu cząstek.
pub const REPULSION_RADIUS: f32 = 10.0;
/// Siła twardego odpychania na jednostkę wnikania w `REPULSION_RADIUS`.
pub const REPULSION_STRENGTH: f32 = 100.0;

/// Współczynnik tarcia kwadratowego (proporcjonalnego do |v|·v).
pub const FRICTION_COEFF: f32 = 0.01;
