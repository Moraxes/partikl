//! Makroskala symulacji bez lustra fizyki: z samych pozycji i prędkości (prawdziwy shader GPU,
//! headless) rozkłady w czasie dla zestawu macierzy oddziaływań. Służy do porównania zachowania
//! przed i po zmianie reguł: `measure_macro` zapisuje próbki do katalogu, `compare_macro` porównuje
//! dwa katalogi (W₁ na skali log, KS, kwantyle w czasie).
//!
//! Wielkości na cząstkę (sąsiedzi: wszyscy, wszystkie gatunki, bez siebie):
//! - `speed`: |v|;
//! - `n40`: sąsiedzi w zasięgu oddziaływania (gęstość, zbicie skupisk);
//! - `n4`: sąsiedzi bliżej niż 4 (ścisłe upakowanie);
//! - `nw`: udział własnego gatunku wśród `n40` (posortowanie gatunków);
//! - `order`: lokalne uporządkowanie prędkości `|Σv| / Σ|v|` po sobie i sąsiadach w zasięgu.
//!
//! Testy są `#[ignore]`, sterowane zmiennymi środowiskowymi (katalogi względem `.artifacts/`):
//! - `MACRO_OUT` katalog wyniku `measure_macro` (domyślnie `boid-forces/baseline`);
//! - `MACRO_OPTIONS` wybory opcji reguł `nazwa=wybór,…`;
//! - `MACRO_PARAMS` nadpisania specyfikacji `params.x=1,…` (jak `--set` w aplikacji);
//! - `MACRO_RUNS` filtr macierzy (podciągi nazw po przecinku);
//! - `MACRO_STEPS` ostatni krok (domyślnie 4000), `MACRO_SEED` przesunięcie ziarna cząstek;
//! - `MACRO_A`, `MACRO_B` katalogi do `compare_macro`, `MACRO_FROM` początek okna porównania.
//!
//! `gate-run --resource cargo --resource gpu -- cargo test --quiet measure_macro -- --ignored --nocapture`

use super::*;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Świat jak ekran 4K (świat symulacji = rozmiar ekranu).
pub(super) const MS_WORLD: Vec2 = Vec2::new(3840.0, 2160.0);
pub(super) const MS_N: usize = 100_000;
/// Próbka cząstek na migawkę (sąsiadami są zawsze wszystkie cząstki).
const SAMPLE: usize = 5_000;
const EVERY: u64 = 250;
const CLOSE: f64 = 4.0;
/// Początek okna porównania (stan quasi-ustalony).
const COMPARE_FROM: u64 = 2000;

/// Nazwy wielkości i czy porównywać je na skali `ln(1 + x)`.
const QUANTITIES: [(&str, bool); 5] = [
  ("speed", true),
  ("n40", true),
  ("n4", true),
  ("nw", false),
  ("order", false),
];

/// Macierz `T×T` z funkcji `M[a][b]` = `interactions[a].force_coeffs[b]`: siła typu `a` (sąsiad)
/// na typ `b` (własny), dodatnia = przyciąganie.
pub(super) fn matrix(types: usize, f: impl Fn(usize, usize) -> f32) -> ParticleSpec {
  ParticleSpec::new(
    (0..types)
      .map(|a| Interaction {
        force_coeffs: (0..types).map(|b| f(a, b)).collect(),
      })
      .collect(),
  )
}

/// Zestaw macierzy pomiarowych: nazwa, liczba typów, specyfikacja (z domyślnymi parametrami).
pub(super) fn matrices() -> Vec<(String, usize, ParticleSpec)> {
  let mut out = Vec::new();
  for seed in [11, 22, 33, 44] {
    let mut rng = StdRng::seed_from_u64(seed);
    out.push((format!("random3_s{seed}"), 3, spec(&mut rng, 3)));
  }
  for seed in [55, 66] {
    let mut rng = StdRng::seed_from_u64(seed);
    out.push((format!("random6_s{seed}"), 6, spec(&mut rng, 6)));
  }
  out.push(("all_attract".into(), 3, matrix(3, |_, _| 300.0)));
  out.push(("all_repel".into(), 3, matrix(3, |_, _| -300.0)));
  out.push((
    "self_sort".into(),
    3,
    matrix(3, |a, b| if a == b { 300.0 } else { -300.0 }),
  ));
  out.push((
    "chase".into(),
    3,
    matrix(3, |a, b| {
      if b == (a + 1) % 3 {
        300.0
      } else if a == (b + 1) % 3 {
        -300.0
      } else {
        0.0
      }
    }),
  ));
  out.push(("zero".into(), 3, matrix(3, |_, _| 0.0)));
  out
}

/// Siatka komórek o boku >= zasięgu oddziaływania (torus), do wyszukiwania kandydatów.
pub(super) struct Grid {
  nx: usize,
  ny: usize,
  cells: Vec<Vec<usize>>,
}

impl Grid {
  pub(super) fn new(particles: &[GpuParticle]) -> Self {
    let nx = (MS_WORLD.x / INTERACTION_RADIUS).floor() as usize;
    let ny = (MS_WORLD.y / INTERACTION_RADIUS).floor() as usize;
    let mut grid = Grid {
      nx,
      ny,
      cells: vec![Vec::new(); nx * ny],
    };
    for (i, p) in particles.iter().enumerate() {
      let (cx, cy) = grid.cell(p.pos[0] as f64, p.pos[1] as f64);
      grid.cells[cy * nx + cx].push(i);
    }
    grid
  }

  fn cell(&self, x: f64, y: f64) -> (usize, usize) {
    let fx = ((x + MS_WORLD.x as f64 / 2.0) / MS_WORLD.x as f64).clamp(0.0, 0.999_999);
    let fy = ((y + MS_WORLD.y as f64 / 2.0) / MS_WORLD.y as f64).clamp(0.0, 0.999_999);
    (
      (fx * self.nx as f64) as usize,
      (fy * self.ny as f64) as usize,
    )
  }

  /// Sąsiedzi cząstki `i` w zasięgu oddziaływania (bez niej samej): `f(j, dx, dy, d)`, `(dx, dy)`
  /// od `i` do `j` po torusie.
  pub(super) fn for_neighbours(
    &self,
    particles: &[GpuParticle],
    i: usize,
    mut f: impl FnMut(usize, f64, f64, f64),
  ) {
    let me = &particles[i];
    let (px, py) = (me.pos[0] as f64, me.pos[1] as f64);
    let wrap = |d: f64, size: f64| {
      if d > size / 2.0 {
        d - size
      } else if d < -size / 2.0 {
        d + size
      } else {
        d
      }
    };
    let (cx, cy) = self.cell(px, py);
    for oy in -1i64..=1 {
      for ox in -1i64..=1 {
        let gx = (cx as i64 + ox).rem_euclid(self.nx as i64) as usize;
        let gy = (cy as i64 + oy).rem_euclid(self.ny as i64) as usize;
        for &j in &self.cells[gy * self.nx + gx] {
          let other = &particles[j];
          let dx = wrap(other.pos[0] as f64 - px, MS_WORLD.x as f64);
          let dy = wrap(other.pos[1] as f64 - py, MS_WORLD.y as f64);
          let d = (dx * dx + dy * dy).sqrt();
          if j != i && d > 0.0 && d <= INTERACTION_RADIUS as f64 {
            f(j, dx, dy, d);
          }
        }
      }
    }
  }
}

/// Wiersze dla `ids` liczone równolegle.
pub(super) fn par_rows<R: Default + Clone + Send>(
  ids: &[usize],
  f: impl Fn(usize) -> R + Sync,
) -> Vec<R> {
  let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
  let chunk = ids.len().div_ceil(threads).max(1);
  let mut rows = vec![R::default(); ids.len()];
  std::thread::scope(|scope| {
    for (k, out) in rows.chunks_mut(chunk).enumerate() {
      let f = &f;
      scope.spawn(move || {
        for (o, slot) in out.iter_mut().enumerate() {
          *slot = f(ids[k * chunk + o]);
        }
      });
    }
  });
  rows
}

/// Wielkości makro jednej cząstki (kolejność jak `QUANTITIES`).
fn macro_row(particles: &[GpuParticle], grid: &Grid, i: usize) -> [f64; 5] {
  let me = &particles[i];
  let v = |p: &GpuParticle| (p.vel[0] as f64, p.vel[1] as f64);
  let (vx, vy) = v(me);
  let speed = (vx * vx + vy * vy).sqrt();
  let (mut n40, mut n4, mut same) = (0.0, 0.0, 0.0);
  let (mut sum, mut abs) = ((vx, vy), speed);
  grid.for_neighbours(particles, i, |j, _, _, d| {
    let other = &particles[j];
    n40 += 1.0;
    n4 += f64::from(d < CLOSE);
    same += f64::from(other.species == me.species);
    let (ox, oy) = v(other);
    sum.0 += ox;
    sum.1 += oy;
    abs += (ox * ox + oy * oy).sqrt();
  });
  let order = if abs > 0.0 {
    (sum.0 * sum.0 + sum.1 * sum.1).sqrt() / abs
  } else {
    0.0
  };
  [speed, n40, n4, same / f64::max(n40, 1.0), order]
}

fn env_list(name: &str) -> Vec<String> {
  std::env::var(name)
    .ok()
    .map(|s| {
      s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
    })
    .unwrap_or_default()
}

fn artifacts_dir(rel: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join(".artifacts")
    .join(rel)
}

/// Opcje i nadpisania specyfikacji ze zmiennych `MACRO_OPTIONS`, `MACRO_PARAMS`.
pub(super) fn env_setup(spec: ParticleSpec) -> (SimOptions, ParticleSpec) {
  let mut options = SimOptions::default();
  for pair in env_list("MACRO_OPTIONS") {
    let (name, choice) = pair.split_once('=').expect("MACRO_OPTIONS: nazwa=wybór");
    options = options.with(name, choice);
  }
  let params = env_list("MACRO_PARAMS");
  let overrides: Vec<(&str, &str)> = params
    .iter()
    .map(|p| p.split_once('=').expect("MACRO_PARAMS: ścieżka=wartość"))
    .collect();
  let spec = crate::loading::apply_overrides(spec, &overrides).expect("MACRO_PARAMS");
  (options, spec)
}

pub(super) fn selected(name: &str) -> bool {
  let filter = env_list("MACRO_RUNS");
  filter.is_empty() || filter.iter().any(|f| name.contains(f.as_str()))
}

#[test]
#[ignore = "pomiar: uruchamiany ręcznie, zapisuje do .artifacts/"]
fn measure_macro() {
  let out = std::env::var("MACRO_OUT").unwrap_or_else(|_| "boid-forces/baseline".into());
  let dir = artifacts_dir(&out);
  std::fs::create_dir_all(&dir).unwrap();
  let last: u64 = std::env::var("MACRO_STEPS").map_or(4000, |s| s.parse().unwrap());
  let seed_offset: u64 = std::env::var("MACRO_SEED").map_or(0, |s| s.parse().unwrap());
  let mut csv = String::from("matrix,step,");
  csv += &QUANTITIES.map(|(n, _)| n).join(",");
  csv.push('\n');
  let started = std::time::Instant::now();
  let mut meta = String::new();
  for (k, (name, types, spec)) in matrices().into_iter().enumerate() {
    if !selected(&name) {
      continue;
    }
    let (options, spec) = env_setup(spec);
    if meta.is_empty() {
      meta = format!(
        "options: {options:?}\nparams: {:?}\nseed offset: {seed_offset}\n",
        spec.params
      );
    }
    let mut rng = StdRng::seed_from_u64(1000 + k as u64 + 7919 * seed_offset);
    let particles = initial_particles(MS_N, types, MS_WORLD, &mut rng);
    let Some(mut harness) = Harness::new(options, spec, particles, MS_WORLD) else {
      eprintln!("brak GPU, pomijam");
      return;
    };
    let mut step = 0;
    while step < last {
      let state = harness.run(EVERY);
      step += EVERY;
      assert_invariants(&state, MS_WORLD, &format!("{name} krok {step}"));
      let grid = Grid::new(&state);
      let mut ids: Vec<usize> = (0..MS_N).collect();
      ids.shuffle(&mut rng);
      ids.truncate(SAMPLE);
      for row in par_rows(&ids, |i| macro_row(&state, &grid, i)) {
        write!(csv, "{name},{step}").unwrap();
        for x in row {
          write!(csv, ",{x:.4}").unwrap();
        }
        csv.push('\n');
      }
    }
    eprintln!("{name} gotowe ({:.0} s)", started.elapsed().as_secs_f64());
  }
  std::fs::write(dir.join("samples.csv"), csv).unwrap();
  std::fs::write(dir.join("meta.txt"), meta).unwrap();
}

/// Próbki `matrix -> step -> wielkość -> wartości`.
type Samples = BTreeMap<String, BTreeMap<u64, Vec<Vec<f64>>>>;

fn load_samples(dir: &Path) -> Samples {
  let text = std::fs::read_to_string(dir.join("samples.csv"))
    .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
  let mut out: Samples = BTreeMap::new();
  for line in text.lines().skip(1) {
    let mut cols = line.split(',');
    let name = cols.next().unwrap().to_string();
    let step: u64 = cols.next().unwrap().parse().unwrap();
    let per_q = out
      .entry(name)
      .or_default()
      .entry(step)
      .or_insert_with(|| vec![Vec::new(); QUANTITIES.len()]);
    for (q, x) in cols.enumerate() {
      per_q[q].push(x.parse().unwrap());
    }
  }
  out
}

fn transformed(xs: &[f64], log: bool) -> Vec<f64> {
  let mut v: Vec<f64> = xs
    .iter()
    .map(|&x| if log { x.ln_1p() } else { x })
    .collect();
  v.sort_by(f64::total_cmp);
  v
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
  if sorted.is_empty() {
    return f64::NAN;
  }
  sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Odległość Wassersteina-1 między rozkładami empirycznymi (posortowanymi), przez kwantyle.
pub(super) fn wasserstein1(a: &[f64], b: &[f64]) -> f64 {
  const K: usize = 1000;
  (0..K)
    .map(|i| {
      let q = (i as f64 + 0.5) / K as f64;
      (quantile(a, q) - quantile(b, q)).abs()
    })
    .sum::<f64>()
    / K as f64
}

/// Statystyka Kołmogorowa-Smirnowa (maks. różnica dystrybuant) dla posortowanych próbek.
pub(super) fn ks(a: &[f64], b: &[f64]) -> f64 {
  let (mut i, mut j, mut d) = (0, 0, 0.0f64);
  while i < a.len() && j < b.len() {
    let x = a[i].min(b[j]);
    while i < a.len() && a[i] <= x {
      i += 1;
    }
    while j < b.len() && b[j] <= x {
      j += 1;
    }
    d = d.max((i as f64 / a.len() as f64 - j as f64 / b.len() as f64).abs());
  }
  d
}

#[test]
#[ignore = "porównanie dwóch pomiarów measure_macro"]
fn compare_macro() {
  let a_rel = std::env::var("MACRO_A").unwrap_or_else(|_| "boid-forces/baseline".into());
  let b_rel = std::env::var("MACRO_B").expect("MACRO_B: katalog porównywany");
  let from: u64 = std::env::var("MACRO_FROM").map_or(COMPARE_FROM, |s| s.parse().unwrap());
  let (a, b) = (
    load_samples(&artifacts_dir(&a_rel)),
    load_samples(&artifacts_dir(&b_rel)),
  );
  let mut report = format!(
    "# {b_rel} względem {a_rel}\n\nOkno: kroki >= {from}. W₁ na skali ln(1+x) dla speed/n40/n4, \
     liniowej dla nw/order. Kwantyle: A → B.\n\n\
     | macierz | wielkość | W₁ | KS | p10 | p50 | p90 | p99 |\n|---|---|---|---|---|---|---|---|\n"
  );
  let mut timeline = String::from("matrix,quantity,step,a_p10,a_p50,a_p90,b_p10,b_p50,b_p90\n");
  let mut totals = vec![(0.0, 0usize); QUANTITIES.len()];
  for (name, a_steps) in &a {
    let Some(b_steps) = b.get(name) else {
      continue;
    };
    for (q, (qname, log)) in QUANTITIES.iter().enumerate() {
      let pool = |steps: &BTreeMap<u64, Vec<Vec<f64>>>| -> Vec<f64> {
        steps
          .iter()
          .filter(|(s, _)| **s >= from)
          .flat_map(|(_, v)| v[q].iter().copied())
          .collect()
      };
      let (pa, pb) = (pool(a_steps), pool(b_steps));
      if pa.is_empty() || pb.is_empty() {
        continue;
      }
      let (ta, tb) = (transformed(&pa, *log), transformed(&pb, *log));
      let w = wasserstein1(&ta, &tb);
      totals[q].0 += w;
      totals[q].1 += 1;
      let (ra, rb) = (transformed(&pa, false), transformed(&pb, false));
      write!(
        report,
        "| {name} | {qname} | {w:.3} | {:.3} |",
        ks(&ta, &tb)
      )
      .unwrap();
      for p in [0.1, 0.5, 0.9, 0.99] {
        write!(
          report,
          " {:.3} → {:.3} |",
          quantile(&ra, p),
          quantile(&rb, p)
        )
        .unwrap();
      }
      report.push('\n');
      for (step, va) in a_steps {
        let Some(vb) = b_steps.get(step) else {
          continue;
        };
        let (sa, sb) = (transformed(&va[q], false), transformed(&vb[q], false));
        writeln!(
          timeline,
          "{name},{qname},{step},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}",
          quantile(&sa, 0.1),
          quantile(&sa, 0.5),
          quantile(&sa, 0.9),
          quantile(&sb, 0.1),
          quantile(&sb, 0.5),
          quantile(&sb, 0.9)
        )
        .unwrap();
      }
    }
  }
  report += "\n## Średnie W₁ po macierzach\n\n";
  let mut score = 0.0;
  for ((qname, _), (sum, n)) in QUANTITIES.iter().zip(&totals) {
    let mean = sum / (*n).max(1) as f64;
    score += mean;
    writeln!(report, "- {qname}: {mean:.3}").unwrap();
  }
  writeln!(report, "- suma: {score:.3}").unwrap();
  println!("{report}");
  let dir = artifacts_dir(&b_rel);
  let stem = a_rel.replace('/', "_");
  std::fs::write(dir.join(format!("compare_vs_{stem}.md")), &report).unwrap();
  std::fs::write(dir.join(format!("timeline_vs_{stem}.csv")), timeline).unwrap();
}
