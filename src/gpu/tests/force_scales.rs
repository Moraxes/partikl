//! Rozkłady sił z `integrate_particle` (`rules/default_rules.wesl`) na stanach z prawdziwej
//! symulacji GPU (headless), dla zestawu macierzy z `macro_stats`. Siły liczone na CPU z migawki,
//! lustrem shadera (bez próbkowania sąsiadów: wszyscy sąsiedzi, kwantyzacja f16 prędkości sąsiada).
//!
//! Lustro sprawdza się samo: z policzonych sił przewiduje prędkość po jednym kroku i porównuje ją
//! z GPU dla cząstek bez próbkowania (mało sąsiadów). Gdy shader się zmieni, a lustro nie, pomiar
//! się wywraca zamiast dawać złe dane.
//!
//! Test jest `#[ignore]`; zmienne środowiskowe jak w `macro_stats` (`MACRO_OPTIONS`, `MACRO_PARAMS`,
//! `MACRO_RUNS`), wynik w `.artifacts/<FORCES_OUT>` (domyślnie `boid-forces/forces`):
//! `gate-run --resource cargo --resource gpu -- cargo test --quiet measure_forces -- --ignored --nocapture`

use super::default_rules::quantize_f16;
use super::macro_stats::{Grid, MS_N, MS_WORLD, env_setup, matrices, par_rows, selected};
use super::*;
use std::fmt::Write as _;

/// Próbka cząstek na migawkę.
const SAMPLE: usize = 20_000;
const SNAPSHOTS: [u64; 3] = [2000, 3000, 4000];
/// Cząstki do sprawdzenia lustra: tylko takie, dla których GPU nie próbkuje sąsiadów.
const CHECK_SAMPLE: usize = 2_000;

type V = (f64, f64);

fn add(a: V, b: V) -> V {
  (a.0 + b.0, a.1 + b.1)
}

fn scale(k: f64, a: V) -> V {
  (k * a.0, k * a.1)
}

fn norm(v: V) -> f64 {
  (v.0 * v.0 + v.1 * v.1).sqrt()
}

/// Warianty opcji reguł (z `MACRO_OPTIONS`).
#[derive(Clone, Copy)]
struct Mode {
  attraction_sum: bool,
}

fn mode() -> Mode {
  let opts = std::env::var("MACRO_OPTIONS").unwrap_or_default();
  Mode {
    attraction_sum: opts.contains("attraction=sum"),
  }
}

/// Siły jednej cząstki (tak, jak wchodzą do sumy) i dane pomocnicze.
#[derive(Clone, Copy, Default)]
struct Forces {
  attraction: V,
  separation: V,
  alignment: V,
  cohesion: V,
  total: V,
  /// Poprawka prędkości z rozsuwania (`corr / dt`).
  overlap_dv: V,
  n_sep: f64,
  neighbours: f64,
  kernel_w: f64,
}

/// Lustro `pair_interact` i sił z `integrate_particle`.
fn forces(
  spec: &ParticleSpec,
  mode: Mode,
  particles: &[GpuParticle],
  grid: &Grid,
  i: usize,
) -> Forces {
  let u = &spec.params;
  let me = &particles[i];
  let vel = (me.vel[0] as f64, me.vel[1] as f64);
  let (mut att, mut kernel_w) = ((0.0, 0.0), 0.0);
  let (mut push, mut n_sep) = ((0.0, 0.0), 0.0);
  let (mut fvel, mut offset, mut n_flock) = ((0.0, 0.0), (0.0, 0.0), 0.0);
  let (mut corr, mut n_overlap) = ((0.0, 0.0), 0.0);
  let mut all = 0.0;
  grid.for_neighbours(particles, i, |j, dx, dy, d| {
    let other = &particles[j];
    let dir = (dx / d, dy / d);
    let coeff = spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
    let h = (1.0 - ((d - u.kernel_middle as f64) / u.kernel_width as f64).abs()).max(0.0);
    att = add(att, scale(coeff * h, dir));
    kernel_w += h;
    let r = u.separation_radius as f64;
    if d < r {
      push = add(push, scale(d - r, dir));
      n_sep += 1.0;
    }
    let is_same = other.species == me.species;
    if is_same && d < u.vision_radius as f64 {
      let ov = (
        quantize_f16(other.vel[0].clamp(-65504.0, 65504.0)),
        quantize_f16(other.vel[1].clamp(-65504.0, 65504.0)),
      );
      fvel = add(fvel, ov);
      offset = add(offset, (dx, dy));
      n_flock += 1.0;
    }
    let reach = u.repulsion_radius as f64;
    if d < reach {
      corr = add(corr, scale(0.5 * (d - reach), dir));
      n_overlap += 1.0;
    }
    all += 1.0;
  });
  let attraction = if mode.attraction_sum {
    scale(u.attraction_scale as f64 / u.attraction_sum_ref as f64, att)
  } else {
    scale(u.attraction_scale as f64 / (kernel_w + 2.0), att)
  };
  let separation = scale(
    u.separation_stiffness as f64 / (1.0 + n_sep / u.separation_saturation as f64),
    push,
  );
  let (alignment, cohesion) = if n_flock > 0.0 {
    (
      scale(
        u.alignment_rate as f64,
        add(scale(1.0 / n_flock, fvel), scale(-1.0, vel)),
      ),
      scale(u.cohesion_stiffness as f64 / n_flock, offset),
    )
  } else {
    ((0.0, 0.0), (0.0, 0.0))
  };
  let total = add(add(separation, attraction), add(alignment, cohesion));
  let overlap_dv = if n_overlap > 0.0 {
    scale(1.0 / (n_overlap * u.dt as f64), corr)
  } else {
    (0.0, 0.0)
  };
  Forces {
    attraction,
    separation,
    alignment,
    cohesion,
    total,
    overlap_dv,
    n_sep,
    neighbours: all,
    kernel_w,
  }
}

/// Prędkość po kroku z sił (jak koniec `integrate_particle`).
fn predicted_vel(spec: &ParticleSpec, me: &GpuParticle, f: &Forces) -> V {
  let u = &spec.params;
  let dt = u.dt as f64;
  let mut v = add((me.vel[0] as f64, me.vel[1] as f64), f.overlap_dv);
  if v.0 * v.0 + v.1 * v.1 < u.velocity_threshold as f64 {
    v = (0.0, 0.0);
  }
  let free = add(v, scale(dt, f.total));
  let mut v = scale(1.0 / (1.0 + u.friction as f64 * dt * norm(free)), free);
  let speed = norm(v);
  if speed >= 1e-3 {
    let dv = (u.target_speed as f64 - speed) * u.drive_rate as f64 * dt;
    v = add(v, scale(dv / speed, v));
  }
  v
}

/// Wielkość z wiersza.
type Column = fn(&Forces) -> f64;

const COLUMNS: [(&str, Column); 9] = [
  ("attraction", |f| norm(f.attraction)),
  ("separation", |f| norm(f.separation)),
  ("alignment", |f| norm(f.alignment)),
  ("cohesion", |f| norm(f.cohesion)),
  ("boid", |f| {
    norm(add(add(f.separation, f.alignment), f.cohesion))
  }),
  ("total", |f| norm(f.total)),
  ("n_sep", |f| f.n_sep),
  ("neighbours", |f| f.neighbours),
  ("kernel_w", |f| f.kernel_w),
];

fn percentile(sorted: &[f64], q: f64) -> f64 {
  if sorted.is_empty() {
    return f64::NAN;
  }
  sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Wiersze podsumowania (kwantyle, średnia, odsetek zer) dla jednej grupy.
fn summary_lines(label: &str, rows: &[Forces], out: &mut String) {
  for (name, f) in COLUMNS {
    let mut v: Vec<f64> = rows.iter().map(f).collect();
    v.sort_by(f64::total_cmp);
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    let zeros = v.iter().filter(|x| **x == 0.0).count() as f64 / v.len().max(1) as f64;
    write!(out, "{label},{name},{}", v.len()).unwrap();
    for q in [0.01, 0.1, 0.25, 0.5, 0.75, 0.9, 0.99] {
      write!(out, ",{:.4}", percentile(&v, q)).unwrap();
    }
    writeln!(out, ",{mean:.4},{zeros:.4}").unwrap();
  }
}

/// Liczba kandydatów na sąsiadów każdej cząstki na GPU: cząstki w sąsiedztwie
/// `(2·GRID_REACH + 1)²` komórek siatki silnika (powyżej `DEFAULT_MAX_CANDIDATES` GPU próbkuje).
fn gpu_candidates(state: &[GpuParticle]) -> Vec<u32> {
  let geo = Geometry::new(MS_WORLD);
  let (nx, ny) = (geo.dims.x as i64, geo.dims.y as i64);
  let cell = |p: &GpuParticle| {
    let c = ((Vec2::from(p.pos) + MS_WORLD * 0.5) / geo.cell_size).floor();
    ((c.x as i64).clamp(0, nx - 1), (c.y as i64).clamp(0, ny - 1))
  };
  let mut counts = vec![0u32; (nx * ny) as usize];
  for p in state {
    let (x, y) = cell(p);
    counts[(y * nx + x) as usize] += 1;
  }
  let reach = GRID_REACH as i64;
  state
    .iter()
    .map(|p| {
      let (x, y) = cell(p);
      let mut sum = 0;
      for oy in -reach..=reach {
        for ox in -reach..=reach {
          let (gx, gy) = ((x + ox).rem_euclid(nx), (y + oy).rem_euclid(ny));
          sum += counts[(gy * nx + gx) as usize];
        }
      }
      sum
    })
    .collect()
}

/// Lustro względem GPU: jeden krok z `state`, porównanie prędkości cząstek bez próbkowania.
fn check_mirror(
  harness: &mut Harness,
  spec: &ParticleSpec,
  mode: Mode,
  state: &[GpuParticle],
  rng: &mut StdRng,
  name: &str,
) {
  let grid = Grid::new(state);
  let candidates = gpu_candidates(state);
  let mut ids: Vec<usize> = (0..state.len())
    .filter(|&i| candidates[i] <= DEFAULT_MAX_CANDIDATES)
    .collect();
  ids.shuffle(rng);
  ids.truncate(CHECK_SAMPLE);
  let rows = par_rows(&ids, |i| forces(spec, mode, state, &grid, i));
  let next = harness.run(1);
  let mut checked = 0;
  let mut worst = 0.0f64;
  for (&i, f) in ids.iter().zip(&rows) {
    checked += 1;
    let want = predicted_vel(spec, &state[i], f);
    let got = (next[i].vel[0] as f64, next[i].vel[1] as f64);
    let err = norm(add(want, scale(-1.0, got)));
    let tol = 1e-2 + 1e-3 * norm(want);
    worst = worst.max(err / tol);
    assert!(
      err <= tol,
      "{name}: lustro niezgodne z GPU dla cząstki {i}: przewidziane {want:?}, GPU {got:?} \
       (v0 {:?}, sąsiadów {}, att {:?}, sep {:?}, align {:?}, coh {:?}, overlap {:?})",
      state[i].vel,
      f.neighbours,
      f.attraction,
      f.separation,
      f.alignment,
      f.cohesion,
      f.overlap_dv
    );
  }
  assert!(
    checked > CHECK_SAMPLE / 4,
    "{name}: za mało cząstek do sprawdzenia lustra ({checked})"
  );
  eprintln!("{name}: lustro zgodne na {checked} cząstkach (najgorzej {worst:.2} tolerancji)");
}

#[test]
#[ignore = "pomiar: uruchamiany ręcznie, zapisuje do .artifacts/"]
fn measure_forces() {
  let out = std::env::var("FORCES_OUT").unwrap_or_else(|_| "boid-forces/forces".into());
  let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join(".artifacts")
    .join(out);
  std::fs::create_dir_all(&dir).unwrap();
  let mode = mode();
  let mut summary = String::from("matrix,quantity,n,p1,p10,p25,p50,p75,p90,p99,mean,zero_frac\n");
  let mut pooled: Vec<Forces> = Vec::new();
  for (k, (name, types, spec)) in matrices().into_iter().enumerate() {
    if !selected(&name) {
      continue;
    }
    let (options, spec) = env_setup(spec);
    let mut rng = StdRng::seed_from_u64(1000 + k as u64);
    let particles = initial_particles(MS_N, types, MS_WORLD, &mut rng);
    let Some(mut harness) = Harness::new(options, spec.clone(), particles, MS_WORLD) else {
      eprintln!("brak GPU, pomijam");
      return;
    };
    let mut done = 0;
    let mut rows: Vec<Forces> = Vec::new();
    for step in SNAPSHOTS {
      let state = harness.run(step - done);
      done = step;
      let grid = Grid::new(&state);
      let mut ids: Vec<usize> = (0..MS_N).collect();
      ids.shuffle(&mut rng);
      ids.truncate(SAMPLE);
      rows.extend(par_rows(&ids, |i| forces(&spec, mode, &state, &grid, i)));
      if step == SNAPSHOTS[0] {
        check_mirror(&mut harness, &spec, mode, &state, &mut rng, &name);
        done += 1;
      }
    }
    summary_lines(&name, &rows, &mut summary);
    pooled.extend(rows);
    eprintln!("{name} gotowe");
  }
  summary_lines("pooled", &pooled, &mut summary);
  std::fs::write(dir.join("summary.csv"), summary).unwrap();
}
