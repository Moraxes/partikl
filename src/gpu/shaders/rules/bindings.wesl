// Wiązania danych reguł (grupa 0): macierz sił i parametry. Importują je reguły (modele, matrix.wesl);
// silnik ich nie czyta. Numery i nazwy muszą zgadzać się z `Binding` w `src/gpu/compute/bindings.rs`.

// Parametry reguł, osobny bufor; układ jak `RuleParams` w Ruście.
struct RuleParams {
  dt: f32,
  repulsion_radius: f32,
  kernel_middle: f32,
  kernel_width: f32,
  attraction_scale: f32,
  attraction_sum_ref: f32,
  friction: f32,
  velocity_threshold: f32,
  separation_radius: f32,
  separation_stiffness: f32,
  separation_saturation: f32,
  vision_radius: f32,
  cohesion_stiffness: f32,
  alignment_rate: f32,
  target_speed: f32,
  drive_rate: f32,
  fov_cos: f32,
}

// Macierz sił `force_matrix[a * T + b]`: siła, z jaką typ `a` działa na typ `b`.
@group(0) @binding(7) var<storage, read_write> force_matrix: array<f32>;
// Parametry reguł (promienie, jądro, tarcie…); wspólne dla wszystkich kroków klatki.
@group(0) @binding(8) var<uniform> rule_params: RuleParams;
