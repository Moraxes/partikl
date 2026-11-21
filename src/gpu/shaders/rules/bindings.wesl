// Wiązania danych reguł (grupa 0): macierz sił i parametry. Importują je reguły (modele, matrix.wesl);
// silnik ich nie czyta. Numery i nazwy muszą zgadzać się z `Binding` w `src/gpu/compute/bindings.rs`.

// Parametry reguł, osobny bufor; układ jak `RuleParams` w Ruście.
struct RuleParams {
  dt: f32,
  repulsion_radius: f32,
  repulsion_strength: f32,
  kernel_middle: f32,
  kernel_width: f32,
  average_neighbours: f32,
  adaptive_neighbours: f32,
  adaptive_range_max: f32,
  crowding_smoothing: f32,
  friction: f32,
  velocity_threshold: f32,
}

// Macierz sił `force_matrix[a * T + b]`: siła, z jaką typ `a` działa na typ `b`.
@group(0) @binding(7) var<storage, read_write> force_matrix: array<f32>;
// Parametry reguł (promienie, jądro, tarcie…); wspólne dla wszystkich kroków klatki.
@group(0) @binding(8) var<uniform> rule_params: RuleParams;
