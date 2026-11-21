// Macierz sił typów cząstek, czytana przez reguły (nie przez silnik): `coeff_for(c, other)` to siła, z
// jaką sąsiad typu `other` działa na cząstkę typu własnego `species` z `load_coeffs(species)`.
// Odczyt macierzy w pętli po sąsiadach (zależny od typu sąsiada) kosztował ok. 20% przebiegu sił,
// więc dla małej liczby typów kolumna macierzy leży w rejestrach (warianty shadera wg flag
// `TYPES_AT_MOST_4` / `TYPES_AT_MOST_8`).

import super::super::engine::uniforms::uniforms;
import super::bindings::force_matrix;

@if(TYPES_AT_MOST_4) {
  alias Coeffs = vec4<f32>;

  fn load_coeffs(species: u32) -> Coeffs {
    var c = vec4<f32>(0.0);
    for (var t = 0u; t < uniforms.num_types; t++) {
      c[t] = force_matrix[t * uniforms.num_types + species];
    }
    return c;
  }

  fn coeff_for(c: Coeffs, other: u32) -> f32 {
    return c[other];
  }
}
@elif(TYPES_AT_MOST_8) {
  struct Coeffs {
    lo: vec4<f32>,
    hi: vec4<f32>,
  }

  fn load_coeffs(species: u32) -> Coeffs {
    var c = Coeffs(vec4<f32>(0.0), vec4<f32>(0.0));
    for (var t = 0u; t < uniforms.num_types; t++) {
      let v = force_matrix[t * uniforms.num_types + species];
      if t < 4u { c.lo[t] = v; } else { c.hi[t - 4u] = v; }
    }
    return c;
  }

  fn coeff_for(c: Coeffs, other: u32) -> f32 {
    return select(c.lo[other & 3u], c.hi[other & 3u], other >= 4u);
  }
}
@else {
  struct Coeffs {
    species: u32,
  }

  fn load_coeffs(species: u32) -> Coeffs {
    return Coeffs(species);
  }

  fn coeff_for(c: Coeffs, other: u32) -> f32 {
    return force_matrix[other * uniforms.num_types + c.species];
  }
}
