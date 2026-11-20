//! Siły między cząstkami, liczone wektorowo (SIMD, `LANES` sąsiadów naraz).
//!
//! Pozycje sąsiadów są czytane wprost z tablic siatki (x, y w kolejności komórek). Co krok
//! obok nich powstaje tablica współczynnika siły dla każdego typu cząstki pytającej, więc
//! współczynnik ładuje się wektorowo, bez wybierania po typie sąsiada. Komórki leżą wierszami,
//! więc sąsiednie komórki wiersza są w pamięci obok siebie i sąsiedztwo to kilka ciągłych
//! zakresów (`SpatialGrid::neighbour_runs`).

use std::ops::Range;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use wide::f32x8;

use super::{Acceleration, FORCES_BATCHING, InteractionId, SpatialGrid};
use crate::config::*;
use crate::spec::ParticleSpec;

type Lanes = f32x8;
const LANES: usize = 8;

/// Współczynniki sił w kolejności siatki na jeden krok, bufory używane ponownie.
#[derive(Default)]
pub struct ForceData {
  /// `coeffs[me * stride + j]`: siła, jaką cząstka `j` siatki działa na cząstkę typu `me`.
  coeffs: Vec<f32>,
  stride: usize,
  /// `table[other * types + me]`: macierz sił spłaszczona.
  table: Vec<f32>,
}

impl ForceData {
  fn fill(&mut self, grid: &SpatialGrid, spec: &ParticleSpec) {
    let types = spec.interactions.len();
    self.table.clear();
    for other in &spec.interactions {
      self.table.extend_from_slice(&other.force_coeffs[..types]);
    }
    let n = grid.len();
    // Zapas jak w tablicach pozycji siatki; zero, bo tam i tak NaN poza zasięgiem.
    self.stride = n + SpatialGrid::PADDING;
    self.coeffs.resize(types * self.stride, 0.0);
    for (me, row) in self.coeffs.chunks_mut(self.stride).enumerate() {
      for (slot, other) in row.iter_mut().zip(grid.types()) {
        *slot = self.table[other.0 * types + me];
      }
      row[n..].fill(0.0);
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
  let (xs, ys, starts) = (grid.xs(), grid.ys(), grid.cell_starts());

  particles
    .par_iter_mut()
    .batching_strategy(FORCES_BATCHING)
    .for_each(|(transform, mut acceleration, interaction)| {
      let position = transform.translation.xy();
      let coeffs = &data.coeffs[interaction.0 * data.stride..(interaction.0 + 1) * data.stride];
      let mut sum = Kernel::new(position);
      for run in grid.neighbour_runs(grid.cell_at(position.x, position.y)) {
        let start = starts[run.first as usize] as usize;
        let end = starts[run.last as usize] as usize;
        // Przebiegi przez szew są tylko przy krawędziach świata; reszta bez dodawania.
        if run.shift == Vec2::ZERO {
          sum.run::<false>(xs, ys, coeffs, start..end, Vec2::ZERO);
        } else {
          sum.run::<true>(xs, ys, coeffs, start..end, run.shift);
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
  sum_x: Lanes,
  sum_y: Lanes,
}

impl Kernel {
  fn new(position: Vec2) -> Self {
    Kernel {
      x: Lanes::splat(position.x),
      y: Lanes::splat(position.y),
      sum_x: Lanes::ZERO,
      sum_y: Lanes::ZERO,
    }
  }

  /// Siły od sąsiadów z zakresu tablic, jednego przebiegu komórek z przesunięciem `shift`
  /// (`NeighbourRun::shift`; `SHIFTED == false` tylko dla zerowego).
  #[inline(always)]
  fn run<const SHIFTED: bool>(&mut self, xs: &[f32], ys: &[f32], coeffs: &[f32], range: Range<usize>, shift: Vec2) {
    let (shift_x, shift_y) = (Lanes::splat(shift.x), Lanes::splat(shift.y));
    let mut i = range.start;
    while i < range.end {
      let mut dx = load(&xs[i..]) - self.x;
      let mut dy = load(&ys[i..]) - self.y;
      if SHIFTED {
        dx += shift_x;
        dy += shift_y;
      }
      self.add(dx, dy, load(&coeffs[i..]), range.end - i);
      i += LANES;
    }
  }

  /// Dodaje siły od `remaining.min(LANES)` sąsiadów z wektorów (różnice już przez szew).
  ///
  /// Stałe wektory muszą być `const`: `Lanes::splat` ze stałą w pętli kompiluje się do
  /// `memcpy` z puli stałych przy każdym użyciu, co podwajało czas pętli.
  #[inline(always)]
  fn add(&mut self, dx: Lanes, dy: Lanes, coeffs: Lanes, remaining: usize) {
    const LANE_INDEX: Lanes = Lanes::new([0., 1., 2., 3., 4., 5., 6., 7.]);
    const RADIUS_SQ: Lanes = Lanes::splat(INTERACTION_RADIUS * INTERACTION_RADIUS);
    const REPULSION: Lanes = Lanes::splat(-REPULSION_STRENGTH);
    const REPULSION_R: Lanes = Lanes::splat(REPULSION_RADIUS);
    const MIDDLE: Lanes = Lanes::splat(KERNEL_MIDDLE);
    const WIDTH: Lanes = Lanes::splat(KERNEL_WIDTH);
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
