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

use super::{Acceleration, RESOLVE, SpatialGrid};
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

/// Od tylu kandydatów cząstka liczy tylko część par (co `n`-ty wektor sąsiadów z wagą `n`,
/// tak żeby zostało ok. `NEIGHBOUR_CAP` kandydatów), z przesunięciem wyboru co krok: w gęstych
/// skupiskach koszt na cząstkę jest ograniczony, a błąd sił to szum, nie stałe przesunięcie
/// (średnia po `n` kolejnych krokach to pełna suma).
const NEIGHBOUR_CAP: u32 = 512;

/// Które wektory sąsiadów liczyć: co `every`-ty od `offset`, z wagą `weight`.
#[derive(Clone, Copy)]
struct Sample {
  every: usize,
  offset: usize,
  weight: Lanes,
}

impl Sample {
  const ALL: Sample = Sample { every: 1, offset: 0, weight: Lanes::ONE };

  /// Co który wektor dla cząstek komórki z `candidates` kandydatami.
  fn every(candidates: u32) -> usize {
    candidates.div_ceil(NEIGHBOUR_CAP).max(1) as usize
  }

  /// Dla cząstki `i` w kroku `step`; `ALL`, jeśli `every == 1`.
  fn new(every: usize, i: usize, step: u64) -> Self {
    if every == 1 {
      return Sample::ALL;
    }
    Sample { every, offset: (i + step as usize) % every, weight: Lanes::splat(every as f32) }
  }
}

/// Siły dla cząstek „do przodu” z jednego zakresu: okno indeksów siatki od początku zakresu
/// do końca najdalszej komórki sąsiedztwa „do przodu” (`first + k` pod indeksem `k`). Bufory
/// sumowane w kolejności zakresów, więc wynik nie zależy od kolejności pracy wątków
/// (symulacja zostaje deterministyczna).
#[derive(Default)]
struct PairBuffer {
  first: usize,
  xs: Vec<f32>,
  ys: Vec<f32>,
  /// Liczba sąsiadów „do przodu” (tylko przy uśrednianiu sił).
  counts: Vec<f32>,
  /// Poprawki pozycji „do przodu” i liczba nakładających się par (tylko przy rozsuwaniu).
  cx: Vec<f32>,
  cy: Vec<f32>,
  cn: Vec<f32>,
}

/// Współczynniki sił w kolejności siatki na jeden krok, podział pracy i wyniki; bufory
/// używane ponownie. Zasób, nie `Local`, bo dzielą go wszystkie warianty `compute_forces`
/// (praca z poprzedniego kroku przetrwa przełączenie opcji).
#[derive(Resource)]
pub struct ForceData {
  /// Zakresy cząstek (w kolejności siatki) dla zadań, o zbliżonej liczbie par.
  chunks: Vec<Range<usize>>,
  /// Siła na cząstkę w kolejności siatki.
  forces: Vec<Vec2>,
  /// Liczba sąsiadów w zasięgu na cząstkę w kolejności siatki (z wagą próbkowania); tylko przy
  /// uśrednianiu sił, inaczej pusta.
  counts: Vec<f32>,
  /// Przy rozsuwaniu (`Overlap::Resolve`): suma poprawek pozycji (x, y) i liczba nakładających
  /// się par (z) na cząstkę w kolejności siatki; inaczej pusta.
  corrections: Vec<Vec3>,
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
  /// Bufory sił „do przodu” (tryb symetryczny), po jednym na zakres.
  buffers: Vec<PairBuffer>,
  /// Numer kroku (przesunięcie wyboru par przy ograniczeniu sąsiadów).
  step: u64,
}

impl Default for ForceData {
  fn default() -> Self {
    ForceData {
      chunks: vec![],
      forces: vec![],
      counts: vec![],
      corrections: vec![],
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
///
/// Warianty według `SimOptions`: `AVERAGE` (`SimOptions::average_forces`) i `OVERLAP`
/// (`Overlap` jako `u8`, stałe `SPRING`, `RESOLVE`, `ADAPTIVE_RANGE`).
pub fn compute_forces<const AVERAGE: bool, const OVERLAP: u8>(
  particle_spec: Res<ParticleSpec>,
  grid: Res<SpatialGrid>,
  mut data: ResMut<ForceData>,
  mut particles: Query<(Entity, &mut Acceleration, &mut Transform)>,
) {
  let resolve = OVERLAP == RESOLVE;
  let symmetric = data.last_work >= SYMMETRIC_WORK;
  data.fill(&grid, &particle_spec, symmetric);
  let pool = ComputeTaskPool::get();
  let n = grid.len();
  data.plan(n, pool.thread_num());
  data.step += 1;
  let parts = data.chunks.len();
  if symmetric && data.buffers.len() < parts {
    data.buffers.resize_with(parts, PairBuffer::default);
  }
  let ForceData { coeffs, out_coeffs, stride, chunks, forces, counts, corrections, last_work, buffers, step, .. } = &mut *data;
  let step = *step;
  forces.clear();
  forces.resize(n, Vec2::ZERO);
  counts.clear();
  counts.resize(if AVERAGE { n } else { 0 }, 0.0);
  corrections.clear();
  corrections.resize(if resolve { n } else { 0 }, Vec3::ZERO);
  let (coeffs, out_coeffs, stride, grid) = (&coeffs[..], &out_coeffs[..], *stride, &*grid);

  let work = pool.scope(|scope| {
    let mut rest = &mut forces[..];
    let mut rest_counts = &mut counts[..];
    let mut rest_corrections = &mut corrections[..];
    let mut buffers = buffers.iter_mut();
    for particles in chunks.iter() {
      let (chunk, tail) = rest.split_at_mut(particles.len());
      rest = tail;
      let (chunk_counts, tail) = rest_counts.split_at_mut(if AVERAGE { particles.len() } else { 0 });
      rest_counts = tail;
      let (chunk_corrections, tail) = rest_corrections.split_at_mut(if resolve { particles.len() } else { 0 });
      rest_corrections = tail;
      let out = Out { forces: chunk, counts: chunk_counts, corrections: chunk_corrections };
      let particles = particles.clone();
      if symmetric {
        let buffer = buffers.next().unwrap();
        scope.spawn(async move {
          let window = forward_window(grid, &particles);
          let len = window.len() + SpatialGrid::PADDING;
          buffer.first = particles.start;
          buffer.xs.clear();
          buffer.xs.resize(len, 0.0);
          buffer.ys.clear();
          buffer.ys.resize(len, 0.0);
          buffer.counts.clear();
          buffer.counts.resize(if AVERAGE { len } else { 0 }, 0.0);
          for values in [&mut buffer.cx, &mut buffer.cy, &mut buffer.cn] {
            values.clear();
            values.resize(if resolve { len } else { 0 }, 0.0);
          }
          pair_forces_in_range::<AVERAGE, OVERLAP>(grid, coeffs, out_coeffs, stride, particles, out, buffer, step)
        });
      } else {
        scope.spawn(async move { forces_in_range::<AVERAGE, OVERLAP>(grid, coeffs, stride, particles, out, step) });
      }
    }
  });
  *last_work = work.iter().sum();
  if symmetric {
    for buffer in &buffers[..chunks.len()] {
      let len = buffer.xs.len() - SpatialGrid::PADDING;
      let window = &mut forces[buffer.first..buffer.first + len];
      for ((force, x), y) in window.iter_mut().zip(&buffer.xs).zip(&buffer.ys) {
        *force += Vec2::new(*x, *y);
      }
      if AVERAGE {
        for (count, c) in counts[buffer.first..buffer.first + len].iter_mut().zip(&buffer.counts) {
          *count += c;
        }
      }
      if resolve {
        let window = &mut corrections[buffer.first..buffer.first + len];
        for (k, correction) in window.iter_mut().enumerate() {
          *correction += Vec3::new(buffer.cx[k], buffer.cy[k], buffer.cn[k]);
        }
      }
    }
  }
  if AVERAGE {
    // Bez sąsiadów `AVERAGE_NEIGHBOURS / 0 = ∞`, więc skala 1.
    for (force, count) in forces.iter_mut().zip(counts.iter()) {
      *force *= (AVERAGE_NEIGHBOURS / count).min(1.0);
    }
  }

  // Z powrotem do ECS: iteracja w tej samej kolejności co w `rebuild_grid` (ten sam zestaw
  // encji, bez zmian struktury między systemami), sprawdzana encja po encji.
  // Rozsuwanie: poprawka pozycji przed całkowaniem, uśredniona po nakładających się parach
  // cząstki (Jacobi z uśrednianiem: wiele nakładań naraz się nie sumuje w przestrzał).
  // Tylko pozycja, bez `LastPosition`, więc poprawka staje się prędkością `poprawka / dt`, jak
  // w dynamice opartej na pozycjach: prędkość zbliżania się pary znika (przesuwanie też
  // `LastPosition` dawało gęstsze skupiska, sonda w zadaniu 025).
  let apply = |i: usize, acceleration: &mut Acceleration, transform: &mut Transform| {
    acceleration.0 += forces[i];
    if resolve {
      let c = corrections[i];
      if c.z > 0.0 {
        transform.translation += (c.xy() / c.z).extend(0.0);
      }
    }
  };
  let (placed, entities) = (grid.placed(), grid.entities());
  let mut lost = false;
  for (k, (entity, mut acceleration, mut transform)) in particles.iter_mut().enumerate() {
    match placed.get(k).map(|&i| i as usize) {
      Some(i) if entities[i] == entity => apply(i, &mut acceleration, &mut transform),
      _ => lost = true,
    }
  }
  if lost {
    // Inna kolejność niż przy budowaniu siatki: wolniej, przez szukanie encji.
    let index: std::collections::HashMap<Entity, usize> =
      entities.iter().enumerate().map(|(i, e)| (*e, i)).collect();
    for (k, (entity, mut acceleration, mut transform)) in particles.iter_mut().enumerate() {
      let ok = placed.get(k).is_some_and(|&i| entities[i as usize] == entity);
      if !ok && let Some(&i) = index.get(&entity) {
        apply(i, &mut acceleration, &mut transform);
      }
    }
  }
}

/// Wyniki dla zakresu cząstek (indeks względem początku zakresu): siły, przy uśrednianiu
/// liczby sąsiadów, przy rozsuwaniu poprawki pozycji (nieużywane puste).
struct Out<'a> {
  forces: &'a mut [Vec2],
  counts: &'a mut [f32],
  corrections: &'a mut [Vec3],
}

impl Out<'_> {
  fn cell(&mut self, range: Range<usize>) -> Out<'_> {
    let part = |len: usize| if len == 0 { 0..0 } else { range.clone() };
    let (counts, corrections) = (part(self.counts.len()), part(self.corrections.len()));
    Out {
      counts: &mut self.counts[counts],
      corrections: &mut self.corrections[corrections],
      forces: &mut self.forces[range],
    }
  }

  fn set<const AVERAGE: bool, const OVERLAP: u8>(&mut self, k: usize, sum: &Kernel) {
    self.forces[k] = sum.total();
    if AVERAGE {
      self.counts[k] = sum.count();
    }
    if OVERLAP == RESOLVE {
      self.corrections[k] = sum.correction();
    }
  }
}

/// Siły dla cząstek `particles` (indeksy siatki); `out` od cząstki `particles.start`.
/// Zwraca pracę: liczbę kandydatów plus stały koszt na cząstkę (dla podziału w następnym
/// kroku).
fn forces_in_range<const AVERAGE: bool, const OVERLAP: u8>(grid: &SpatialGrid, coeffs: &[f32], stride: usize, particles: Range<usize>, mut out: Out, step: u64) -> u64 {
  let mut work = 0u64;
  let starts = grid.cell_starts();
  // Pierwsza komórka zakresu: ostatnia, która zaczyna się nie dalej niż `particles.start`.
  let mut c = starts.partition_point(|&s| s as usize <= particles.start) - 1;
  let mut i = particles.start;
  while i < particles.end {
    let cell_end = (starts[c + 1] as usize).min(particles.end);
    let runs = grid.neighbour_runs(c);
    let candidates: u32 = runs.iter().map(|r| starts[r.last as usize] - starts[r.first as usize]).sum();
    work += (cell_end - i) as u64 * (candidates as u64 + 8);
    let every = Sample::every(candidates);
    let cell = OneSidedCell { grid, coeffs, stride, runs, every, step };
    let out = out.cell(i - particles.start..cell_end - particles.start);
    // Wariant bez próbkowania osobno: ten sam kod co bez ograniczenia sąsiadów.
    if every == 1 {
      cell.forces::<false, AVERAGE, OVERLAP>(i..cell_end, out);
    } else {
      cell.forces::<true, AVERAGE, OVERLAP>(i..cell_end, out);
    }
    i = cell_end;
    c += 1;
  }
  work
}

/// Cząstki jednej komórki w trybie par liczonych raz.
struct PairCell<'a> {
  grid: &'a SpatialGrid,
  coeffs: &'a [f32],
  out_coeffs: &'a [f32],
  stride: usize,
  forward: &'a [super::grid::NeighbourRun],
  seam: &'a [super::grid::NeighbourRun],
  /// Koniec całej komórki (pary w komórce z dalszymi cząstkami, także spoza zakresu).
  full_end: usize,
  every: usize,
  step: u64,
}

impl PairCell<'_> {
  #[inline(always)]
  fn forces<const SAMPLED: bool, const AVERAGE: bool, const OVERLAP: u8>(&self, particles: Range<usize>, mut out: Out, buffer: &mut PairBuffer) {
    let (xs, ys, starts, types) = (self.grid.xs(), self.grid.ys(), self.grid.cell_starts(), self.grid.types());
    let stride = self.stride;
    for (k, i) in particles.enumerate() {
      let me = types[i].0;
      let cin = &self.coeffs[me * stride..(me + 1) * stride];
      let cout = &self.out_coeffs[me * stride..(me + 1) * stride];
      let sample = if SAMPLED { Sample::new(self.every, i, self.step) } else { Sample::ALL };
      let mut sum = Kernel::new(Vec2::new(xs[i], ys[i]));
      sum.pair_run::<SAMPLED, AVERAGE, OVERLAP>(xs, ys, cin, cout, i + 1..self.full_end, buffer, sample);
      for run in self.forward {
        let range = starts[run.first as usize] as usize..starts[run.last as usize] as usize;
        sum.pair_run::<SAMPLED, AVERAGE, OVERLAP>(xs, ys, cin, cout, range, buffer, sample);
      }
      for run in self.seam {
        let range = starts[run.first as usize] as usize..starts[run.last as usize] as usize;
        sum.run::<true, SAMPLED, AVERAGE, OVERLAP>(xs, ys, cin, range, run.shift, sample);
      }
      out.set::<AVERAGE, OVERLAP>(k, &sum);
    }
  }
}

/// Cząstki jednej komórki w trybie jednostronnym.
struct OneSidedCell<'a> {
  grid: &'a SpatialGrid,
  coeffs: &'a [f32],
  stride: usize,
  runs: &'a [super::grid::NeighbourRun],
  every: usize,
  step: u64,
}

impl OneSidedCell<'_> {
  #[inline(always)]
  fn forces<const SAMPLED: bool, const AVERAGE: bool, const OVERLAP: u8>(&self, particles: Range<usize>, mut out: Out) {
    let (xs, ys, starts, types) = (self.grid.xs(), self.grid.ys(), self.grid.cell_starts(), self.grid.types());
    for (k, i) in particles.enumerate() {
      let me = types[i].0;
      let coeffs = &self.coeffs[me * self.stride..(me + 1) * self.stride];
      let sample = if SAMPLED { Sample::new(self.every, i, self.step) } else { Sample::ALL };
      let mut sum = Kernel::new(Vec2::new(xs[i], ys[i]));
      for run in self.runs {
        let start = starts[run.first as usize] as usize;
        let end = starts[run.last as usize] as usize;
        // Przebiegi przez szew są tylko przy krawędziach świata; reszta bez dodawania.
        if run.shift == Vec2::ZERO {
          sum.run::<false, SAMPLED, AVERAGE, OVERLAP>(xs, ys, coeffs, start..end, Vec2::ZERO, sample);
        } else {
          sum.run::<true, SAMPLED, AVERAGE, OVERLAP>(xs, ys, coeffs, start..end, run.shift, sample);
        }
      }
      out.set::<AVERAGE, OVERLAP>(k, &sum);
    }
  }
}

/// Indeksy siatki, do których zakres `particles` dopisuje siły „do przodu”: od początku
/// zakresu do końca najdalszej komórki „do przodu” ostatniej komórki zakresu (komórki
/// sąsiedztwa bez szwu leżą najwyżej `reach` wierszy i `reach` kolumn dalej).
fn forward_window(grid: &SpatialGrid, particles: &Range<usize>) -> Range<usize> {
  if particles.is_empty() {
    return particles.clone();
  }
  let starts = grid.cell_starts();
  let last = starts.partition_point(|&s| s as usize <= particles.end - 1) - 1;
  let reach = grid.reach();
  let end_cell = (last + reach * grid.columns() + reach + 1).min(grid.cell_count());
  particles.start..starts[end_cell] as usize
}

/// Jak `forces_in_range`, ale każda para raz: pary w tej samej komórce (dalsze cząstki) i z
/// komórek „do przodu” (`SpatialGrid::half_runs`) dają siłę obu cząstkom, druga trafia do
/// `buffer` (indeksy siatki); pary przez szew jednostronnie, jak zawsze.
#[allow(clippy::too_many_arguments)]
fn pair_forces_in_range<const AVERAGE: bool, const OVERLAP: u8>(
  grid: &SpatialGrid,
  coeffs: &[f32],
  out_coeffs: &[f32],
  stride: usize,
  particles: Range<usize>,
  mut out: Out,
  buffer: &mut PairBuffer,
  step: u64,
) -> u64 {
  let mut work = 0u64;
  let starts = grid.cell_starts();
  let mut c = starts.partition_point(|&s| s as usize <= particles.start) - 1;
  let mut i = particles.start;
  while i < particles.end {
    let full_end = starts[c + 1] as usize;
    let cell_end = full_end.min(particles.end);
    let candidates: u32 = grid.neighbour_runs(c).iter().map(|r| starts[r.last as usize] - starts[r.first as usize]).sum();
    work += (cell_end - i) as u64 * (candidates as u64 + 8);
    let (forward, seam) = grid.half_runs(c);
    let every = Sample::every(candidates);
    let cell = PairCell { grid, coeffs, out_coeffs, stride, forward, seam, full_end, every, step };
    let out = out.cell(i - particles.start..cell_end - particles.start);
    // Wariant bez próbkowania osobno: ten sam kod co bez ograniczenia sąsiadów.
    if every == 1 {
      cell.forces::<false, AVERAGE, OVERLAP>(i..cell_end, out, buffer);
    } else {
      cell.forces::<true, AVERAGE, OVERLAP>(i..cell_end, out, buffer);
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
  /// Sąsiedzi w zasięgu (z wagą próbkowania); liczeni tylko przy uśrednianiu sił.
  count: Lanes,
  /// Przy rozsuwaniu: suma poprawek pozycji od nakładających się par i ich liczba (bez wagi
  /// próbkowania: pominięte pary dojdą w kolejnych krokach, a waga dawałaby przestrzał).
  cor_x: Lanes,
  cor_y: Lanes,
  cor_n: Lanes,
}

impl Kernel {
  fn new(position: Vec2) -> Self {
    Kernel {
      x: Lanes::splat(position.x),
      y: Lanes::splat(position.y),
      sum_x: Lanes::ZERO,
      sum_y: Lanes::ZERO,
      count: Lanes::ZERO,
      cor_x: Lanes::ZERO,
      cor_y: Lanes::ZERO,
      cor_n: Lanes::ZERO,
    }
  }

  /// Siły od sąsiadów z zakresu tablic, jednego przebiegu komórek z przesunięciem `shift`
  /// (`NeighbourRun::shift`; `SHIFTED == false` tylko dla zerowego).
  #[inline(always)]
  /// `SAMPLED`: tylko wektory z `sample` (z wagą); bez tego wszystkie, `sample` pomijane.
  /// `AVERAGE`: także liczba sąsiadów (`count`).
  /// `OVERLAP == RESOLVE`: zamiast sprężyny poprawka pozycji.
  fn run<const SHIFTED: bool, const SAMPLED: bool, const AVERAGE: bool, const OVERLAP: u8>(&mut self, xs: &[f32], ys: &[f32], coeffs: &[f32], range: Range<usize>, shift: Vec2, sample: Sample) {
    let (shift_x, shift_y) = (Lanes::splat(shift.x), Lanes::splat(shift.y));
    let (offset, every) = if SAMPLED { (sample.offset, sample.every) } else { (0, 1) };
    let mut i = range.start + offset * LANES;
    while i < range.end {
      let mut dx = load(&xs[i..]) - self.x;
      let mut dy = load(&ys[i..]) - self.y;
      if SHIFTED {
        dx += shift_x;
        dy += shift_y;
      }
      if SAMPLED {
        self.add::<true, AVERAGE, OVERLAP>(dx, dy, load(&coeffs[i..]) * sample.weight, sample.weight, range.end - i);
      } else {
        self.add::<false, AVERAGE, OVERLAP>(dx, dy, load(&coeffs[i..]), Lanes::ONE, range.end - i);
      }
      i += every * LANES;
    }
  }

  /// Dodaje siły od `remaining.min(LANES)` sąsiadów z wektorów (różnice już przez szew).
  ///
  /// Stałe wektory muszą być `const`: `Lanes::splat` ze stałą w pętli kompiluje się do
  /// `memcpy` z puli stałych przy każdym użyciu, co podwajało czas pętli.
  #[inline(always)]
  fn add<const SAMPLED: bool, const AVERAGE: bool, const OVERLAP: u8>(&mut self, dx: Lanes, dy: Lanes, coeffs: Lanes, weight: Lanes, remaining: usize) {
    const LANE_INDEX: Lanes = Lanes::new([0., 1., 2., 3., 4., 5., 6., 7.]);
    const RADIUS_SQ: Lanes = Lanes::splat(INTERACTION_RADIUS * INTERACTION_RADIUS);
    const REPULSION: Lanes = Lanes::splat(-REPULSION_STRENGTH);
    const REPULSION_R: Lanes = Lanes::splat(REPULSION_RADIUS);
    const MIDDLE: Lanes = Lanes::splat(KERNEL_MIDDLE);
    const WIDTH: Lanes = Lanes::splat(KERNEL_WIDTH);
    const HALF: Lanes = Lanes::splat(0.5);
    let distance_sq = dx * dx + dy * dy;
    let valid = LANE_INDEX.simd_lt(Lanes::splat(remaining as f32))
      & distance_sq.simd_le(RADIUS_SQ)
      & distance_sq.simd_gt(Lanes::ZERO);
    let distance = distance_sq.sqrt();
    let attraction = coeffs * (Lanes::ONE - ((distance - MIDDLE) / WIDTH).abs()).max(Lanes::ZERO);
    let (ux, uy) = (dx / distance, dy / distance);
    let magnitude = if OVERLAP == RESOLVE {
      // Przyciąganie zaczyna się dalej niż zasięg odpychania, więc bez sprężyny zostaje samo.
      let overlap = valid & distance.simd_lt(REPULSION_R);
      // Maska po mnożeniu: poza zasięgiem i na zapasie kierunek bywa NaN.
      let push = HALF * (distance - REPULSION_R);
      self.cor_x += overlap.select(push * ux, Lanes::ZERO);
      self.cor_y += overlap.select(push * uy, Lanes::ZERO);
      self.cor_n += overlap.select(Lanes::ONE, Lanes::ZERO);
      attraction
    } else {
      let repulsion = if SAMPLED { REPULSION * weight * (REPULSION_R - distance) } else { REPULSION * (REPULSION_R - distance) };
      distance.simd_lt(REPULSION_R).select(repulsion, attraction)
    };
    // Poza zasięgiem `dx / distance` może być NaN, więc maska na samym wkładzie.
    self.sum_x += valid.select(magnitude * ux, Lanes::ZERO);
    self.sum_y += valid.select(magnitude * uy, Lanes::ZERO);
    if AVERAGE {
      self.count += valid.select(weight, Lanes::ZERO);
    }
  }

  /// Pary z zakresu tablic (bez przesunięcia) liczone raz: siła od sąsiadów do sumy tej
  /// cząstki, siła od tej cząstki na sąsiadów odjęta (przeciwny kierunek) w `bx`, `by`
  /// (okno od indeksu `buffer.first`); przy `AVERAGE` para liczy się jako sąsiad obu cząstek.
  #[inline(always)]
  #[allow(clippy::too_many_arguments)]
  fn pair_run<const SAMPLED: bool, const AVERAGE: bool, const OVERLAP: u8>(
    &mut self,
    xs: &[f32],
    ys: &[f32],
    cin: &[f32],
    cout: &[f32],
    range: Range<usize>,
    buffer: &mut PairBuffer,
    sample: Sample,
  ) {
    let PairBuffer { first, xs: bx, ys: by, counts: bc, cx, cy, cn } = buffer;
    let first = *first;
    const LANE_INDEX: Lanes = Lanes::new([0., 1., 2., 3., 4., 5., 6., 7.]);
    const RADIUS_SQ: Lanes = Lanes::splat(INTERACTION_RADIUS * INTERACTION_RADIUS);
    const REPULSION: Lanes = Lanes::splat(-REPULSION_STRENGTH);
    const REPULSION_R: Lanes = Lanes::splat(REPULSION_RADIUS);
    const MIDDLE: Lanes = Lanes::splat(KERNEL_MIDDLE);
    const WIDTH: Lanes = Lanes::splat(KERNEL_WIDTH);
    const HALF: Lanes = Lanes::splat(0.5);
    let (offset, every) = if SAMPLED { (sample.offset, sample.every) } else { (0, 1) };
    let mut j = range.start + offset * LANES;
    while j < range.end {
      let dx = load(&xs[j..]) - self.x;
      let dy = load(&ys[j..]) - self.y;
      let distance_sq = dx * dx + dy * dy;
      let valid = LANE_INDEX.simd_lt(Lanes::splat((range.end - j) as f32))
        & distance_sq.simd_le(RADIUS_SQ)
        & distance_sq.simd_gt(Lanes::ZERO);
      let distance = distance_sq.sqrt();
      let repelled = distance.simd_lt(REPULSION_R);
      let mut repulsion = if OVERLAP == RESOLVE { Lanes::ZERO } else { REPULSION * (REPULSION_R - distance) };
      let mut shape = (Lanes::ONE - ((distance - MIDDLE) / WIDTH).abs()).max(Lanes::ZERO);
      if SAMPLED {
        repulsion *= sample.weight;
        shape *= sample.weight;
      }
      let mag_in = repelled.select(repulsion, load(&cin[j..]) * shape);
      let mag_out = repelled.select(repulsion, load(&cout[j..]) * shape);
      let (ux, uy) = (dx / distance, dy / distance);
      let k = j - first;
      if OVERLAP == RESOLVE {
        // Poprawka obu cząstek pary: ta cząstka do siebie, sąsiad w przeciwną stronę.
        let overlap = valid & repelled;
        let push = HALF * (distance - REPULSION_R);
        let (px, py) = (overlap.select(push * ux, Lanes::ZERO), overlap.select(push * uy, Lanes::ZERO));
        let one = overlap.select(Lanes::ONE, Lanes::ZERO);
        self.cor_x += px;
        self.cor_y += py;
        self.cor_n += one;
        let (out_x, out_y, out_n) = (load(&cx[k..]) - px, load(&cy[k..]) - py, load(&cn[k..]) + one);
        cx[k..k + LANES].copy_from_slice(&out_x.to_array());
        cy[k..k + LANES].copy_from_slice(&out_y.to_array());
        cn[k..k + LANES].copy_from_slice(&out_n.to_array());
      }
      self.sum_x += valid.select(mag_in * ux, Lanes::ZERO);
      self.sum_y += valid.select(mag_in * uy, Lanes::ZERO);
      let out_x = load(&bx[k..]) - valid.select(mag_out * ux, Lanes::ZERO);
      let out_y = load(&by[k..]) - valid.select(mag_out * uy, Lanes::ZERO);
      bx[k..k + LANES].copy_from_slice(&out_x.to_array());
      by[k..k + LANES].copy_from_slice(&out_y.to_array());
      if AVERAGE {
        let weight = if SAMPLED { sample.weight } else { Lanes::ONE };
        let counted = valid.select(weight, Lanes::ZERO);
        self.count += counted;
        let out_count = load(&bc[k..]) + counted;
        bc[k..k + LANES].copy_from_slice(&out_count.to_array());
      }
      j += every * LANES;
    }
  }

  fn total(&self) -> Vec2 {
    Vec2::new(self.sum_x.reduce_add(), self.sum_y.reduce_add())
  }

  fn count(&self) -> f32 {
    self.count.reduce_add()
  }

  fn correction(&self) -> Vec3 {
    Vec3::new(self.cor_x.reduce_add(), self.cor_y.reduce_add(), self.cor_n.reduce_add())
  }
}

#[cfg(test)]
mod tests;
