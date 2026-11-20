//! Siły między cząstkami, liczone wektorowo (SIMD, `LANES` sąsiadów naraz).
//!
//! Pozycje sąsiadów są czytane wprost z tablic siatki (x, y w kolejności komórek). Co krok
//! obok nich powstaje tablica współczynnika siły dla każdego typu cząstki pytającej, więc
//! współczynnik ładuje się wektorowo, bez wybierania po typie sąsiada. Komórki leżą wierszami,
//! więc sąsiednie komórki wiersza są w pamięci obok siebie i sąsiedztwo to kilka ciągłych
//! zakresów (`SpatialGrid::neighbour_runs`).

use std::ops::Range;
use std::sync::Mutex;

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
/// Od tej pracy na krok (z poprzedniego kroku) każda para jest liczona raz, z siłą dla obu
/// cząstek (`pair_forces_in_range`); poniżej każda cząstka sumuje swoje pary sama
/// (`forces_in_range`), bo zbieranie buforów kosztuje więcej, niż daje połowa par.
const SYMMETRIC_WORK: u64 = 600_000;

/// Siły dla cząstek „do przodu” z jednego zadania; bufor na wątek (zadania jednego wątku idą
/// po kolei), zerowany przy pierwszym użyciu w kroku.
#[derive(Default)]
struct PairBuffer {
  step: u64,
  xs: Vec<f32>,
  ys: Vec<f32>,
}

/// Współczynniki sił w kolejności siatki na jeden krok, podział pracy i wyniki; bufory
/// używane ponownie.
pub struct ForceData {
  /// Zakresy cząstek (w kolejności siatki) dla zadań, o zbliżonej liczbie par.
  chunks: Vec<Range<usize>>,
  /// Siła na cząstkę w kolejności siatki.
  forces: Vec<Vec2>,
  /// Praca (pary plus stały koszt na cząstkę) zmierzona w poprzednim kroku; na start duża,
  /// więc pierwszy krok dzieli się drobno.
  last_work: u64,
  /// `coeffs[me * stride + j]`: siła, jaką cząstka `j` siatki działa na cząstkę typu `me`.
  coeffs: Vec<f32>,
  stride: usize,
  /// `table[other * types + me]`: macierz sił spłaszczona.
  table: Vec<f32>,
  /// `out_coeffs[me * stride + j]`: siła, jaką cząstka typu `me` działa na cząstkę `j` siatki.
  out_coeffs: Vec<f32>,
  /// Bufory sił „do przodu” (tryb symetryczny), co najmniej tyle, ile zadań naraz.
  buffers: Vec<Mutex<PairBuffer>>,
  step: u64,
}

impl Default for ForceData {
  fn default() -> Self {
    ForceData {
      chunks: vec![],
      forces: vec![],
      last_work: u64::MAX,
      coeffs: vec![],
      stride: 0,
      table: vec![],
      out_coeffs: vec![],
      buffers: vec![],
      step: 0,
    }
  }
}

impl ForceData {
  fn fill(&mut self, grid: &SpatialGrid, spec: &ParticleSpec, symmetric: bool) {
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
    if symmetric {
      self.out_coeffs.resize(types * self.stride, 0.0);
      for (me, row) in self.out_coeffs.chunks_mut(self.stride).enumerate() {
        for (slot, other) in row.iter_mut().zip(grid.types()) {
          *slot = self.table[me * types + other.0];
        }
        row[n..].fill(0.0);
      }
    }
  }

  /// Podział cząstek na ciągłe zakresy po równo. Liczba zakresów z pracy poprzedniego kroku
  /// (ok. `CHUNK_WORK` na zakres, w granicach `CHUNKS_PER_THREAD` na każdy z `threads`
  /// wątków); nierówną pracę zakresów (skupiska) wyrównują wątki, które biorą kolejne zakresy.
  /// Dokładniejszy podział według kandydatów każdej komórki kosztował kilkadziesiąt µs na
  /// krok, więcej niż dawał.
  fn plan(&mut self, n: usize, threads: usize) {
    let threads = threads.max(1);
    let parts = (self.last_work.div_ceil(CHUNK_WORK) as usize)
      .clamp(threads * CHUNKS_PER_THREAD.start(), threads * CHUNKS_PER_THREAD.end())
      .min(n.max(1));
    self.chunks.clear();
    self.chunks.extend((0..parts).map(|k| k * n / parts..(k + 1) * n / parts));
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
  let symmetric = data.last_work >= SYMMETRIC_WORK;
  data.fill(&grid, &particle_spec, symmetric);
  let pool = ComputeTaskPool::get();
  let n = grid.len();
  data.plan(n, pool.thread_num());
  data.step += 1;
  while data.buffers.len() < pool.thread_num() + 1 {
    data.buffers.push(Mutex::default());
  }
  let ForceData { coeffs, out_coeffs, stride, chunks, forces, last_work, buffers, step, .. } = &mut *data;
  forces.clear();
  forces.resize(n, Vec2::ZERO);
  let (coeffs, out_coeffs, stride, grid, buffers, step) = (&coeffs[..], &out_coeffs[..], *stride, &*grid, &buffers[..], *step);

  let work = pool.scope(|scope| {
    let mut rest = &mut forces[..];
    for particles in chunks.iter() {
      let (chunk, tail) = rest.split_at_mut(particles.len());
      rest = tail;
      let particles = particles.clone();
      if symmetric {
        scope.spawn(async move {
          let mut buffer = take_buffer(buffers);
          if buffer.step != step {
            buffer.step = step;
            buffer.xs.clear();
            buffer.xs.resize(n + SpatialGrid::PADDING, 0.0);
            buffer.ys.clear();
            buffer.ys.resize(n + SpatialGrid::PADDING, 0.0);
          }
          pair_forces_in_range(grid, coeffs, out_coeffs, stride, particles, chunk, &mut buffer)
        });
      } else {
        scope.spawn(async move { forces_in_range(grid, coeffs, stride, particles, chunk) });
      }
    }
  });
  *last_work = work.iter().sum();
  if symmetric {
    for buffer in buffers.iter() {
      let buffer = buffer.lock().unwrap();
      if buffer.step == step {
        for ((force, x), y) in forces.iter_mut().zip(&buffer.xs).zip(&buffer.ys) {
          *force += Vec2::new(*x, *y);
        }
      }
    }
  }

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
/// Zwraca pracę: liczbę kandydatów plus stały koszt na cząstkę (dla podziału w następnym
/// kroku).
fn forces_in_range(grid: &SpatialGrid, coeffs: &[f32], stride: usize, particles: Range<usize>, out: &mut [Vec2]) -> u64 {
  let mut work = 0u64;
  let (xs, ys, starts, types) = (grid.xs(), grid.ys(), grid.cell_starts(), grid.types());
  // Pierwsza komórka zakresu: ostatnia, która zaczyna się nie dalej niż `particles.start`.
  let mut c = starts.partition_point(|&s| s as usize <= particles.start) - 1;
  let mut i = particles.start;
  while i < particles.end {
    let cell_end = (starts[c + 1] as usize).min(particles.end);
    let runs = grid.neighbour_runs(c);
    let candidates: u32 = runs.iter().map(|r| starts[r.last as usize] - starts[r.first as usize]).sum();
    work += (cell_end - i) as u64 * (candidates as u64 + 8);
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
  work
}

/// Wolny bufor sił „do przodu”: zadań naraz jest najwyżej tyle, ile wątków (z wątkiem
/// głównym), a buforów o jeden więcej, więc któryś zawsze jest wolny.
fn take_buffer(buffers: &[Mutex<PairBuffer>]) -> std::sync::MutexGuard<'_, PairBuffer> {
  loop {
    for buffer in buffers {
      if let Ok(guard) = buffer.try_lock() {
        return guard;
      }
    }
    std::hint::spin_loop();
  }
}

/// Jak `forces_in_range`, ale każda para raz: pary w tej samej komórce (dalsze cząstki) i z
/// komórek „do przodu” (`SpatialGrid::half_runs`) dają siłę obu cząstkom, druga trafia do
/// `buffer` (indeksy siatki); pary przez szew jednostronnie, jak zawsze.
fn pair_forces_in_range(
  grid: &SpatialGrid,
  coeffs: &[f32],
  out_coeffs: &[f32],
  stride: usize,
  particles: Range<usize>,
  out: &mut [Vec2],
  buffer: &mut PairBuffer,
) -> u64 {
  let mut work = 0u64;
  let (xs, ys, starts, types) = (grid.xs(), grid.ys(), grid.cell_starts(), grid.types());
  let PairBuffer { xs: bx, ys: by, .. } = buffer;
  let mut c = starts.partition_point(|&s| s as usize <= particles.start) - 1;
  let mut i = particles.start;
  while i < particles.end {
    let full_end = starts[c + 1] as usize;
    let cell_end = full_end.min(particles.end);
    let candidates: u32 = grid.neighbour_runs(c).iter().map(|r| starts[r.last as usize] - starts[r.first as usize]).sum();
    work += (cell_end - i) as u64 * (candidates as u64 + 8);
    let (forward, seam) = grid.half_runs(c);
    for i in i..cell_end {
      let me = types[i].0;
      let cin = &coeffs[me * stride..(me + 1) * stride];
      let cout = &out_coeffs[me * stride..(me + 1) * stride];
      let mut sum = Kernel::new(Vec2::new(xs[i], ys[i]));
      sum.pair_run(xs, ys, cin, cout, i + 1..full_end, bx, by);
      for run in forward {
        let range = starts[run.first as usize] as usize..starts[run.last as usize] as usize;
        sum.pair_run(xs, ys, cin, cout, range, bx, by);
      }
      for run in seam {
        let range = starts[run.first as usize] as usize..starts[run.last as usize] as usize;
        sum.run::<true>(xs, ys, cin, range, run.shift);
      }
      out[i - particles.start] = sum.total();
    }
    i = cell_end;
    c += 1;
  }
  work
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

  /// Pary z zakresu tablic (bez przesunięcia) liczone raz: siła od sąsiadów do sumy tej
  /// cząstki, siła od tej cząstki na sąsiadów odjęta (przeciwny kierunek) w `bx`, `by`.
  #[inline(always)]
  fn pair_run(&mut self, xs: &[f32], ys: &[f32], cin: &[f32], cout: &[f32], range: Range<usize>, bx: &mut [f32], by: &mut [f32]) {
    const LANE_INDEX: Lanes = Lanes::new([0., 1., 2., 3., 4., 5., 6., 7.]);
    const RADIUS_SQ: Lanes = Lanes::splat(INTERACTION_RADIUS * INTERACTION_RADIUS);
    const REPULSION: Lanes = Lanes::splat(-REPULSION_STRENGTH);
    const REPULSION_R: Lanes = Lanes::splat(REPULSION_RADIUS);
    const MIDDLE: Lanes = Lanes::splat(KERNEL_MIDDLE);
    const WIDTH: Lanes = Lanes::splat(KERNEL_WIDTH);
    let mut j = range.start;
    while j < range.end {
      let dx = load(&xs[j..]) - self.x;
      let dy = load(&ys[j..]) - self.y;
      let distance_sq = dx * dx + dy * dy;
      let valid = LANE_INDEX.simd_lt(Lanes::splat((range.end - j) as f32))
        & distance_sq.simd_le(RADIUS_SQ)
        & distance_sq.simd_gt(Lanes::ZERO);
      let distance = distance_sq.sqrt();
      let repelled = distance.simd_lt(REPULSION_R);
      let repulsion = REPULSION * (REPULSION_R - distance);
      let shape = (Lanes::ONE - ((distance - MIDDLE) / WIDTH).abs()).max(Lanes::ZERO);
      let mag_in = repelled.select(repulsion, load(&cin[j..]) * shape);
      let mag_out = repelled.select(repulsion, load(&cout[j..]) * shape);
      let (ux, uy) = (dx / distance, dy / distance);
      self.sum_x += valid.select(mag_in * ux, Lanes::ZERO);
      self.sum_y += valid.select(mag_in * uy, Lanes::ZERO);
      let out_x = load(&bx[j..]) - valid.select(mag_out * ux, Lanes::ZERO);
      let out_y = load(&by[j..]) - valid.select(mag_out * uy, Lanes::ZERO);
      bx[j..j + LANES].copy_from_slice(&out_x.to_array());
      by[j..j + LANES].copy_from_slice(&out_y.to_array());
      j += LANES;
    }
  }

  fn total(&self) -> Vec2 {
    Vec2::new(self.sum_x.reduce_add(), self.sum_y.reduce_add())
  }
}

#[cfg(test)]
mod tests;
