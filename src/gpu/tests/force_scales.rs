//! Rozkłady członów sił z aktualnego shadera `integrate_particle` (`rules/default_rules.wesl`)
//! na stanach z prawdziwej symulacji GPU (headless), po ustabilizowaniu się układu, dla kilku
//! macierzy oddziaływań. Człony liczone na CPU z migawki, wzorem shadera (bez próbkowania
//! sąsiadów: brute force po siatce komórek, kwantyzacja f16 prędkości sąsiada).
//!
//! Test jest `#[ignore]`: uruchamiany ręcznie, zapisuje surowe dane i statystyki do
//! `.artifacts/force-distributions-2026-11-26/`. Uruchomienie:
//! `cargo test --quiet measure_force_distributions -- --ignored --nocapture`

use super::default_rules::quantize_f16;
use super::*;
use std::fmt::Write as _;

/// Świat jak ekran 4K (świat symulacji = rozmiar ekranu).
const FS_WORLD: Vec2 = Vec2::new(3840.0, 2160.0);
const N: usize = 100_000;
/// Rozkład liczony na losowej próbce cząstek (sąsiadami są zawsze wszystkie cząstki).
const SUBSAMPLE: usize = 20_000;
/// Mała próbka do śledzenia ustabilizowania (mediany w czasie).
const TRACK_SAMPLE: usize = 5_000;
/// Kroki, w których logujemy mediany; próbki rozkładu z ostatnich trzech.
const CHECKPOINTS: [u64; 6] = [500, 1000, 2000, 3000, 3500, 4000];
const SAMPLED_FROM: u64 = 3000;

#[derive(Clone, Copy, Default)]
struct Row {
  species: u32,
  speed: f64,
  nw: f64,
  vw: f64,
  gain: f64,
  attraction: f64,
  sep: f64,
  align: f64,
  coh: f64,
  boid: f64,
  eff_boid: f64,
  eff_attraction: f64,
  force: f64,
  neighbours: f64,
  kernel_w: f64,
}

fn norm(v: (f64, f64)) -> f64 {
  (v.0 * v.0 + v.1 * v.1).sqrt()
}

/// Człony sił jednej cząstki, lustro `pair_interact` + początku `integrate_particle`.
fn terms(spec: &ParticleSpec, particles: &[GpuParticle], grid: &Grid, i: usize) -> Row {
  let u = &spec.params;
  let me = &particles[i];
  let (px, py) = (me.pos[0] as f64, me.pos[1] as f64);
  let (vx, vy) = (me.vel[0] as f64, me.vel[1] as f64);
  let mut f = (0.0f64, 0.0f64);
  let (mut count, mut same, mut kernel_w) = (0.0f64, 0.0f64, 0.0f64);
  let (mut s, mut sep_n) = ((0.0, 0.0), 0.0);
  let (mut a, mut align_w) = ((0.0, 0.0), 0.0);
  let (mut c, mut view_n) = ((0.0, 0.0), 0.0);
  let wrap = |d: f64, size: f64| {
    if d > size / 2.0 {
      d - size
    } else if d < -size / 2.0 {
      d + size
    } else {
      d
    }
  };
  grid.for_candidates(px, py, |j| {
    let other = &particles[j];
    let dx = wrap(other.pos[0] as f64 - px, FS_WORLD.x as f64);
    let dy = wrap(other.pos[1] as f64 - py, FS_WORLD.y as f64);
    let d = (dx * dx + dy * dy).sqrt();
    if j == i || d == 0.0 || d > INTERACTION_RADIUS as f64 {
      return;
    }
    let (dirx, diry) = (dx / d, dy / d);
    // Macierz: `interactions[sąsiad].force_coeffs[własny typ]`, dodatni = przyciąganie (dir: ja -> sąsiad).
    let coeff = spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
    let h = (1.0 - ((d - u.kernel_middle as f64) / u.kernel_width as f64).abs()).max(0.0);
    f.0 += coeff * h * dirx;
    f.1 += coeff * h * diry;
    kernel_w += h;
    let same_species = me.species == other.species;
    same += f64::from(same_species);
    count += 1.0;
    if same_species && d < u.separation_radius as f64 {
      let sd = d.max(1e-3);
      s.0 += u.separation_weight as f64 * (-dirx / sd);
      s.1 += u.separation_weight as f64 * (-diry / sd);
      sep_n += 1.0;
    }
    if same_species && d < u.vision_radius as f64 {
      let (wa, wc) = (u.alignment_weight as f64, u.coherence_weight as f64);
      a.0 += wa * quantize_f16(other.vel[0]);
      a.1 += wa * quantize_f16(other.vel[1]);
      align_w += wa;
      c.0 += wc * dx;
      c.1 += wc * dy;
      view_n += 1.0;
    }
  });
  let attraction = (f.0 / (kernel_w + 2.0), f.1 / (kernel_w + 2.0));
  let mean = |v: (f64, f64), n: f64| {
    if n > 0.0 {
      (v.0 / n, v.1 / n)
    } else {
      (0.0, 0.0)
    }
  };
  let sep = mean(s, sep_n);
  let align = mean((a.0 - vx * align_w, a.1 - vy * align_w), view_n);
  let coh = mean(c, view_n);
  let boid = (sep.0 + align.0 + coh.0, sep.1 + align.1 + coh.1);
  let nw = 0.1 + same / count.max(1.0);
  let vw = (2.0 + vx * vx + vy * vy).log2();
  let gain = (nw * vw).sqrt();
  let force = (
    gain * boid.0 + 10.0 * attraction.0 / gain,
    gain * boid.1 + 10.0 * attraction.1 / gain,
  );
  Row {
    species: me.species,
    speed: (vx * vx + vy * vy).sqrt(),
    nw,
    vw,
    gain,
    attraction: norm(attraction),
    sep: norm(sep),
    align: norm(align),
    coh: norm(coh),
    boid: norm(boid),
    eff_boid: gain * norm(boid),
    eff_attraction: 10.0 * norm(attraction) / gain,
    force: norm(force),
    neighbours: count,
    kernel_w,
  }
}

/// Siatka komórek o boku >= zasięgu oddziaływania (torus), do wyszukiwania kandydatów.
struct Grid {
  nx: usize,
  ny: usize,
  cells: Vec<Vec<usize>>,
}

impl Grid {
  fn new(particles: &[GpuParticle]) -> Self {
    let nx = (FS_WORLD.x / INTERACTION_RADIUS).floor() as usize;
    let ny = (FS_WORLD.y / INTERACTION_RADIUS).floor() as usize;
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
    let fx = ((x + FS_WORLD.x as f64 / 2.0) / FS_WORLD.x as f64).clamp(0.0, 0.999_999);
    let fy = ((y + FS_WORLD.y as f64 / 2.0) / FS_WORLD.y as f64).clamp(0.0, 0.999_999);
    (
      (fx * self.nx as f64) as usize,
      (fy * self.ny as f64) as usize,
    )
  }

  fn for_candidates(&self, x: f64, y: f64, mut f: impl FnMut(usize)) {
    let (cx, cy) = self.cell(x, y);
    for dy in -1i64..=1 {
      for dx in -1i64..=1 {
        let gx = (cx as i64 + dx).rem_euclid(self.nx as i64) as usize;
        let gy = (cy as i64 + dy).rem_euclid(self.ny as i64) as usize;
        self.cells[gy * self.nx + gx].iter().for_each(|&j| f(j));
      }
    }
  }
}

fn snapshot_rows(spec: &ParticleSpec, particles: &[GpuParticle], ids: &[usize]) -> Vec<Row> {
  let grid = Grid::new(particles);
  let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
  let chunk = ids.len().div_ceil(threads);
  let mut rows = vec![Row::default(); ids.len()];
  std::thread::scope(|scope| {
    for (k, out) in rows.chunks_mut(chunk).enumerate() {
      let grid = &grid;
      scope.spawn(move || {
        for (o, slot) in out.iter_mut().enumerate() {
          *slot = terms(spec, particles, grid, ids[k * chunk + o]);
        }
      });
    }
  });
  rows
}

fn percentile(sorted: &[f64], q: f64) -> f64 {
  if sorted.is_empty() {
    return f64::NAN;
  }
  sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Wielkość z wiersza.
type Column = fn(&Row) -> f64;

/// Kolumny rozkładu: nazwa i selektor.
const COLUMNS: [(&str, Column); 14] = [
  ("speed", |r| r.speed),
  ("nw", |r| r.nw),
  ("vw", |r| r.vw),
  ("gain_sqrt_nw_vw", |r| r.gain),
  ("attraction", |r| r.attraction),
  ("a_sep", |r| r.sep),
  ("a_align", |r| r.align),
  ("a_coh", |r| r.coh),
  ("boid", |r| r.boid),
  ("eff_boid", |r| r.eff_boid),
  ("eff_attraction", |r| r.eff_attraction),
  ("force_total", |r| r.force),
  ("neighbours", |r| r.neighbours),
  ("kernel_w", |r| r.kernel_w),
];

fn median(rows: &[Row], f: fn(&Row) -> f64) -> f64 {
  let mut v: Vec<f64> = rows.iter().map(f).collect();
  v.sort_by(|a, b| a.total_cmp(b));
  percentile(&v, 0.5)
}

/// Wiersze podsumowania (kwantyle, średnia, odsetek dokładnych zer) dla jednej grupy.
fn summary_lines(label: &str, rows: &[&Row], out: &mut String) {
  for (name, f) in COLUMNS {
    let mut v: Vec<f64> = rows.iter().map(|r| f(r)).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    let mean = v.iter().sum::<f64>() / v.len().max(1) as f64;
    let zeros = v.iter().filter(|x| **x == 0.0).count() as f64 / v.len().max(1) as f64;
    write!(out, "{label},{name},{}", v.len()).unwrap();
    for q in [0.01, 0.1, 0.5, 0.9, 0.99] {
      write!(out, ",{:.5}", percentile(&v, q)).unwrap();
    }
    writeln!(out, ",{mean:.5},{zeros:.5}").unwrap();
  }
}

fn matrix(types: usize, f: impl Fn(usize, usize) -> f32) -> ParticleSpec {
  ParticleSpec::new(
    (0..types)
      .map(|a| Interaction {
        force_coeffs: (0..types).map(|b| f(a, b)).collect(),
      })
      .collect(),
  )
}

#[test]
#[ignore = "pomiar: uruchamiany ręcznie, zapisuje do .artifacts/force-distributions-2026-11-26/"]
fn measure_force_distributions() {
  let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .join(".artifacts/force-distributions-2026-11-26");
  std::fs::create_dir_all(&dir).unwrap();
  // `M[a][b]` = `interactions[a].force_coeffs[b]`: siła typu `a` (sąsiad) na typ `b` (własny),
  // dodatnia = przyciąganie.
  let mut runs: Vec<(String, usize, Option<u64>)> = Vec::new();
  for seed in [11, 22, 33, 44] {
    runs.push((format!("random3_s{seed}"), 3, Some(seed)));
  }
  for seed in [55, 66] {
    runs.push((format!("random6_s{seed}"), 6, Some(seed)));
  }
  for name in ["all_attract", "all_repel", "self_sort", "chase", "zero"] {
    runs.push((name.into(), 3, None));
  }
  let mut raw = String::from("matrix,step,id,species,");
  raw += &COLUMNS.map(|(n, _)| n).join(",");
  raw.push('\n');
  let mut stab = String::from("matrix,step,median_speed,median_attraction\n");
  let mut summary = String::from("matrix,quantity,n,p1,p10,p50,p90,p99,mean,zero_frac\n");
  let mut pooled: Vec<Row> = Vec::new();
  for (name, types, seed) in &runs {
    let mut rng = StdRng::seed_from_u64(seed.unwrap_or(1));
    let spec = match (seed, name.as_str()) {
      (Some(_), _) => spec(&mut rng, *types),
      (None, "all_attract") => matrix(3, |_, _| 300.0),
      (None, "all_repel") => matrix(3, |_, _| -300.0),
      (None, "self_sort") => matrix(3, |a, b| if a == b { 300.0 } else { -300.0 }),
      (None, "chase") => matrix(3, |a, b| {
        if b == (a + 1) % 3 {
          300.0
        } else if a == (b + 1) % 3 {
          -300.0
        } else {
          0.0
        }
      }),
      _ => matrix(3, |_, _| 0.0),
    };
    let particles = initial_particles(N, *types, FS_WORLD, &mut rng);
    let Some(mut harness) = Harness::new(SimOptions::default(), spec.clone(), particles, FS_WORLD)
    else {
      eprintln!("brak GPU, pomijam");
      return;
    };
    let mut done = 0;
    let mut run_rows: Vec<Row> = Vec::new();
    for step in CHECKPOINTS {
      let state = harness.run(step - done);
      done = step;
      assert_invariants(&state, FS_WORLD, &format!("{name} krok {step}"));
      let mut ids: Vec<usize> = (0..N).collect();
      ids.shuffle(&mut rng);
      let track = snapshot_rows(&spec, &state, &ids[..TRACK_SAMPLE]);
      writeln!(
        stab,
        "{name},{step},{:.4},{:.4}",
        median(&track, |r| r.speed),
        median(&track, |r| r.attraction)
      )
      .unwrap();
      if step >= SAMPLED_FROM {
        ids.truncate(SUBSAMPLE);
        let rows = snapshot_rows(&spec, &state, &ids);
        for (id, r) in ids.iter().zip(&rows) {
          write!(raw, "{name},{step},{id},{}", r.species).unwrap();
          for (_, f) in COLUMNS {
            write!(raw, ",{:.5}", f(r)).unwrap();
          }
          raw.push('\n');
        }
        run_rows.extend(rows);
      }
      eprintln!("{name} krok {step} gotowe");
    }
    summary_lines(name, &run_rows.iter().collect::<Vec<_>>(), &mut summary);
    pooled.extend(run_rows);
  }
  summary_lines("pooled", &pooled.iter().collect::<Vec<_>>(), &mut summary);
  std::fs::write(dir.join("raw_per_particle.csv"), raw).unwrap();
  std::fs::write(dir.join("stabilization.csv"), stab).unwrap();
  std::fs::write(dir.join("summary.csv"), summary).unwrap();
}
