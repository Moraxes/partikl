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
pub const REPULSION_RADIUS: f32 = 1.0;

/// Skala przyciągania z macierzy: współczynnik macierzy razy to daje przyspieszenie (j/s²).
pub const ATTRACTION_SCALE: f32 = 10.0;
/// Przyciąganie jako suma (opcja `attraction=sum`): suma wkładów dzielona przez tę wagę jądra,
/// czyli siła jak od tylu sąsiadów w środku jądra.
pub const ATTRACTION_SUM_REF: f32 = 8.0;

/// Współczynnik tarcia kwadratowego (proporcjonalnego do |v|·v).
pub const FRICTION_COEFF: f32 = 0.01;

// Parametry boidów. Wszystkie składowe są przyspieszeniami (j/s²).
/// Zasięg separacji (sprężyny od wszystkich sąsiadów bliżej niż to).
pub const SEPARATION_RADIUS: f32 = 3.5;
/// Sztywność sprężyny separacji (1/s²): siła od jednego sąsiada to `k · (R − d)`.
pub const SEPARATION_STIFFNESS: f32 = 10.0;
/// Nasycenie separacji: suma sprężyn dzielona przez `1 + n / to`, więc w tłumie rośnie wolniej niż
/// liczba sąsiadów (najwyżej ok. `k · R · to`).
pub const SEPARATION_SATURATION: f32 = 4.0;
pub const VISION_RADIUS: f32 = 40.0;
/// Sztywność sprężyny do środka sąsiadów własnego gatunku (1/s²).
pub const COHESION_STIFFNESS: f32 = 1.0;
/// Szybkość wyrównania prędkości do średniej sąsiadów własnego gatunku (1/s).
pub const ALIGNMENT_RATE: f32 = 1.0;
pub const TARGET_SPEED: f32 = 40.0;
pub const DRIVE_RATE: f32 = 0.0;
pub const FOV_COS: f32 = 0.86;
