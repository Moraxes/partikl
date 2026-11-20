//! Siły między cząstkami, liczone wektorowo (SIMD, `LANES` sąsiadów naraz).
//!
//! Pozycje sąsiadów są czytane wprost z tablic siatki (x, y w kolejności komórek). Co krok
//! obok nich powstaje tablica współczynnika siły dla każdego typu cząstki pytającej, więc
//! współczynnik ładuje się wektorowo, bez wybierania po typie sąsiada. Komórki leżą wierszami,
//! więc sąsiednie komórki wiersza są w pamięci obok siebie i sąsiedztwo to kilka ciągłych
//! zakresów (`SpatialGrid::neighbour_runs`).

use std::ops::Range;

use bevy::prelude::*;
use wide::f32x8;

use bevy::tasks::ComputeTaskPool;

use super::{Acceleration, SpatialGrid};
use crate::config::*;
use crate::spec::ParticleSpec;

type Lanes = f32x8;
const LANES: usize = 8;

/// Praca jednego zakresu cząstek (pary plus stały koszt na cząstkę): przy małej gęstości mało
/// zakresów, bo każde zadanie puli kosztuje, przy dużej wiele, żeby wątki, które skończą
/// wcześniej (rdzenie wydajne), przejmowały resztę.
const CHUNK_WORK: u64 = 50_000;
/// Granice liczby zakresów na wątek.
const CHUNKS_PER_THREAD: std::ops::RangeInclusive<usize> = 1..=16;

/// Współczynniki sił w kolejności siatki na jeden krok, podział pracy i wyniki; bufory
/// używane ponownie.
#[derive(Default)]
pub struct ForceData {
  /// Zakresy cząstek (w kolejności siatki) dla zadań, o zbliżonej liczbie par.
  chunks: Vec<Range<usize>>,
  /// Siła na cząstkę w kolejności siatki.
  forces: Vec<Vec2>,
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

  /// Podział cząstek na ciągłe zakresy o zbliżonej pracy (ok. `CHUNK_WORK`, w granicach
  /// `CHUNKS_PER_THREAD` na każdy z `threads` wątków). Praca cząstki to
  /// liczba kandydatów w sąsiedztwie jej komórki plus stały koszt; zakres może zacząć się i
  /// skończyć w środku komórki, bo jedna komórka w gęstym skupisku potrafi mieć więcej pracy
  /// niż wątek powinien dostać.
  fn plan(&mut self, grid: &SpatialGrid, threads: usize) {
    let starts = grid.cell_starts();
    let per_particle = |c: usize| -> u64 {
      grid.neighbour_runs(c).iter().map(|r| (starts[r.last as usize] - starts[r.first as usize]) as u64).sum::<u64>() + 8
    };
    let total: u64 = (0..grid.cell_count()).map(|c| grid.cell_range(c).len() as u64 * per_particle(c)).sum();
    let threads = threads.max(1);
    let parts = (total.div_ceil(CHUNK_WORK) as usize)
      .clamp(threads * CHUNKS_PER_THREAD.start(), threads * CHUNKS_PER_THREAD.end());
    let target = total.div_ceil(parts as u64).max(1);
    self.chunks.clear();
    let (mut first, mut acc) = (0, 0u64);
    for c in 0..grid.cell_count() {
      let range = grid.cell_range(c);
      if range.is_empty() {
        continue;
      }
      let cost = per_particle(c);
      let mut i = range.start;
      while i < range.end {
        // Ile cząstek tej komórki zmieści się do celu bieżącego zakresu.
        let fit = ((target - acc).div_ceil(cost) as usize).min(range.end - i);
        i += fit;
        acc += fit as u64 * cost;
        if acc >= target {
          self.chunks.push(first..i);
          first = i;
          acc = 0;
        }
      }
    }
    if first < grid.len() {
      self.chunks.push(first..grid.len());
    }
  }
}

/// Siły od sąsiadów z siatki, liczone w kolejności siatki: cząstki komórka po komórce
/// (sąsiedzi kolejnych cząstek są ci sami i już w pamięci podręcznej), zakresami komórek o
/// wyrównanej pracy, potem dodane do `Acceleration`. Pozycje i typy pochodzą z siatki, nie z
/// ECS, więc siatka musi być aktualna (`rebuild_grid`). Sama cząstka jest pomijana przez
/// zerową odległość (tak samo każda inna dokładnie w tym samym miejscu, dla której kierunek
/// siły i tak jest nieokreślony).
pub fn compute_forces(
  particle_spec: Res<ParticleSpec>,
  grid: Res<SpatialGrid>,
  mut data: Local<ForceData>,
  mut particles: Query<(Entity, &mut Acceleration)>,
) {
  data.fill(&grid, &particle_spec);
  let pool = ComputeTaskPool::get();
  data.plan(&grid, pool.thread_num());
  let n = grid.len();
  let ForceData { coeffs, stride, chunks, forces, .. } = &mut *data;
  forces.clear();
  forces.resize(n, Vec2::ZERO);
  let (coeffs, stride, grid) = (&coeffs[..], *stride, &*grid);

  pool.scope(|scope| {
    let mut rest = &mut forces[..];
    for particles in chunks.iter() {
      let (chunk, tail) = rest.split_at_mut(particles.len());
      rest = tail;
      let particles = particles.clone();
      scope.spawn(async move { forces_in_range(grid, coeffs, stride, particles, chunk) });
    }
  });

  // Z powrotem do ECS: iteracja w tej samej kolejności co w `rebuild_grid` (ten sam zestaw
  // encji, bez zmian struktury między systemami), sprawdzana encja po encji.
  let (placed, entities) = (grid.placed(), grid.entities());
  let mut lost = false;
  for (k, (entity, mut acceleration)) in particles.iter_mut().enumerate() {
    match placed.get(k).map(|&i| i as usize) {
      Some(i) if entities[i] == entity => acceleration.0 += forces[i],
      _ => lost = true,
    }
  }
  if lost {
    // Inna kolejność niż przy budowaniu siatki: wolniej, przez szukanie encji.
    let index: std::collections::HashMap<Entity, usize> =
      entities.iter().enumerate().map(|(i, e)| (*e, i)).collect();
    for (k, (entity, mut acceleration)) in particles.iter_mut().enumerate() {
      let ok = placed.get(k).is_some_and(|&i| entities[i as usize] == entity);
      if !ok && let Some(&i) = index.get(&entity) {
        acceleration.0 += forces[i];
      }
    }
  }
}

/// Siły dla cząstek `particles` (indeksy siatki); `out[i - particles.start]` dla cząstki `i`.
fn forces_in_range(grid: &SpatialGrid, coeffs: &[f32], stride: usize, particles: Range<usize>, out: &mut [Vec2]) {
  let (xs, ys, starts, types) = (grid.xs(), grid.ys(), grid.cell_starts(), grid.types());
  // Pierwsza komórka zakresu: ostatnia, która zaczyna się nie dalej niż `particles.start`.
  let mut c = starts.partition_point(|&s| s as usize <= particles.start) - 1;
  let mut i = particles.start;
  while i < particles.end {
    let cell_end = (starts[c + 1] as usize).min(particles.end);
    let runs = grid.neighbour_runs(c);
    for i in i..cell_end {
      let me = types[i].0;
      let coeffs = &coeffs[me * stride..(me + 1) * stride];
      let mut sum = Kernel::new(Vec2::new(xs[i], ys[i]));
      for run in runs {
        let start = starts[run.first as usize] as usize;
        let end = starts[run.last as usize] as usize;
        // Przebiegi przez szew są tylko przy krawędziach świata; reszta bez dodawania.
        if run.shift == Vec2::ZERO {
          sum.run::<false>(xs, ys, coeffs, start..end, Vec2::ZERO);
        } else {
          sum.run::<true>(xs, ys, coeffs, start..end, run.shift);
        }
      }
      out[i - particles.start] = sum.total();
    }
    i = cell_end;
    c += 1;
  }
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

#[cfg(test)]
mod tests;
