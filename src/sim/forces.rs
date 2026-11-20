//! Siły między cząstkami, liczone wektorowo (SIMD, `LANES` sąsiadów naraz).
//!
//! Co krok dane z siatki są kopiowane do osobnych tablic (struktura tablic) w kolejności
//! komórek: x, y i współczynnik siły dla każdego typu cząstki pytającej. Komórki leżą
//! wierszami, więc sąsiednie komórki wiersza są w pamięci obok siebie i sąsiedztwo to kilka
//! ciągłych zakresów (`SpatialGrid::neighbour_runs`), a współczynnik ładuje się wektorowo,
//! bez wybierania po typie sąsiada.

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use wide::f32x8;

use super::{Acceleration, FORCES_BATCHING, InteractionId, SpatialGrid};
use crate::config::*;
use crate::spec::ParticleSpec;

type Lanes = f32x8;
const LANES: usize = 8;

/// Kopia danych siatki na jeden krok, bufory używane ponownie między krokami.
#[derive(Default)]
pub struct ForceData {
  xs: Vec<f32>,
  ys: Vec<f32>,
  /// `coeffs[me * stride + j]`: siła, jaką cząstka `j` działa na cząstkę typu `me`.
  coeffs: Vec<f32>,
  kinds: Vec<usize>,
  /// Początek komórki `c` w tablicach to `starts[c]`, koniec `starts[c + 1]`.
  starts: Vec<u32>,
  stride: usize,
}

impl ForceData {
  fn fill(&mut self, grid: &SpatialGrid, spec: &ParticleSpec) {
    self.xs.clear();
    self.ys.clear();
    self.kinds.clear();
    self.starts.clear();
    for cell in grid.cells() {
      self.starts.push(self.xs.len() as u32);
      for n in cell {
        self.xs.push(n.position.x);
        self.ys.push(n.position.y);
        self.kinds.push(n.interaction.0);
      }
    }
    self.starts.push(self.xs.len() as u32);
    // Zapas na ostatni niepełny wektor: NaN nigdy nie jest w zasięgu.
    self.xs.extend([f32::NAN; LANES]);
    self.ys.extend([f32::NAN; LANES]);

    let types = spec.interactions.len();
    self.stride = self.xs.len();
    self.coeffs.clear();
    for me in 0..types {
      self
        .coeffs
        .extend(self.kinds.iter().map(|&other| spec.interactions[other].force_coeffs[me]));
      self.coeffs.extend([0.0; LANES]);
    }
  }
}

/// Siły od sąsiadów z siatki. Pozycje i typy sąsiadów pochodzą z siatki, nie z ECS, więc
/// siatka musi być zsynchronizowana z `Transform` (robi to `wrap_around`). Sama cząstka jest
/// pomijana przez zerową odległość (tak samo każda inna dokładnie w tym samym miejscu, dla
/// której kierunek siły i tak jest nieokreślony).
pub fn compute_forces(
  particle_spec: Res<ParticleSpec>,
  grid: Res<SpatialGrid>,
  mut data: Local<ForceData>,
  mut particles: Query<(&Transform, &mut Acceleration, &InteractionId)>,
) {
  data.fill(&grid, &particle_spec);
  let data = &*data;
  let half = grid.half_size();

  particles
    .par_iter_mut()
    .batching_strategy(FORCES_BATCHING)
    .for_each(|(transform, mut acceleration, interaction)| {
      let position = transform.translation.xy();
      let coeffs = &data.coeffs[interaction.0 * data.stride..(interaction.0 + 1) * data.stride];
      let mut sum = Kernel::new(position, half);
      for &(first, last) in grid.neighbour_runs(grid.cell_at(position.x, position.y)) {
        let start = data.starts[first as usize] as usize;
        let end = data.starts[last as usize] as usize;
        let mut i = start;
        while i < end {
          sum.add(
            load(&data.xs[i..]),
            load(&data.ys[i..]),
            load(&coeffs[i..]),
            end - i,
          );
          i += LANES;
        }
      }
      acceleration.0 += sum.total();
    });
}

fn load(values: &[f32]) -> Lanes {
  let lanes: [f32; LANES] = values[..LANES].try_into().unwrap();
  Lanes::from(lanes)
}

/// Suma sił działających na jedną cząstkę, `LANES` sąsiadów naraz.
struct Kernel {
  x: Lanes,
  y: Lanes,
  half_x: Lanes,
  half_y: Lanes,
  sum_x: Lanes,
  sum_y: Lanes,
}

impl Kernel {
  fn new(position: Vec2, half: Vec2) -> Self {
    Kernel {
      x: Lanes::splat(position.x),
      y: Lanes::splat(position.y),
      half_x: Lanes::splat(half.x),
      half_y: Lanes::splat(half.y),
      sum_x: Lanes::ZERO,
      sum_y: Lanes::ZERO,
    }
  }

  /// Różnica współrzędnych krótszą drogą na torusie (`get_corrected_position_delta`).
  #[inline(always)]
  fn wrap(delta: Lanes, half: Lanes) -> Lanes {
    let size = half + half;
    let delta = delta.simd_gt(half).select(delta - size, delta);
    delta.simd_lt(-half).select(delta + size, delta)
  }

  /// Dodaje siły od `remaining.min(LANES)` sąsiadów z wektorów.
  ///
  /// Stałe wektory muszą być `const`: `Lanes::splat` ze stałą w pętli kompiluje się do
  /// `memcpy` z puli stałych przy każdym użyciu, co podwajało czas pętli.
  #[inline(always)]
  fn add(&mut self, xs: Lanes, ys: Lanes, coeffs: Lanes, remaining: usize) {
    const LANE_INDEX: Lanes = Lanes::new([0., 1., 2., 3., 4., 5., 6., 7.]);
    const RADIUS_SQ: Lanes = Lanes::splat(INTERACTION_RADIUS * INTERACTION_RADIUS);
    const REPULSION: Lanes = Lanes::splat(-REPULSION_STRENGTH);
    const REPULSION_R: Lanes = Lanes::splat(REPULSION_RADIUS);
    const MIDDLE: Lanes = Lanes::splat(KERNEL_MIDDLE);
    const WIDTH: Lanes = Lanes::splat(KERNEL_WIDTH);
    let dx = Self::wrap(xs - self.x, self.half_x);
    let dy = Self::wrap(ys - self.y, self.half_y);
    let distance_sq = dx * dx + dy * dy;
    let valid = LANE_INDEX.simd_lt(Lanes::splat(remaining as f32))
      & distance_sq.simd_le(RADIUS_SQ)
      & distance_sq.simd_gt(Lanes::ZERO);
    let distance = distance_sq.sqrt();
    let repulsion = REPULSION * (REPULSION_R - distance);
    let attraction = coeffs * (Lanes::ONE - ((distance - MIDDLE) / WIDTH).abs()).max(Lanes::ZERO);
    let magnitude = distance.simd_lt(REPULSION_R).select(repulsion, attraction);
    // Poza zasięgiem `dx / distance` może być NaN, więc maska na samym wkładzie.
    self.sum_x += valid.select(magnitude * (dx / distance), Lanes::ZERO);
    self.sum_y += valid.select(magnitude * (dy / distance), Lanes::ZERO);
  }

  fn total(&self) -> Vec2 {
    Vec2::new(self.sum_x.reduce_add(), self.sum_y.reduce_add())
  }
}
