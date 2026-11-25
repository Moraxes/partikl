//! Pomiar rzędów wielkości członów sił mieszanych w `integrate_particle`
//! (`rules/default_rules.wesl`): `attraction` i `boid` (= sep + align + coh), na stanach z
//! prawdziwej symulacji GPU (headless). Człony liczone na CPU z migawki, wzorem shadera (bez
//! próbkowania sąsiadów: brute force po siatce komórek, kwantyzacja f16 prędkości sąsiada).
//!
//! Test jest `#[ignore]`: uruchamiany ręcznie, zapisuje surowe dane i statystyki do
//! `.artifacts/force-scales/`. Uruchomienie:
//! `cargo test --quiet force_scales -- --ignored --nocapture`

use super::default_rules::quantize_f16;
use super::*;
use std::fmt::Write as _;

/// Świat jak ekran 4K (świat symulacji = rozmiar ekranu), liczby cząstek do porównania.
const FS_WORLD: Vec2 = Vec2::new(3840.0, 2160.0);
const COUNTS: [usize; 2] = [1_000_000, 100_000];
/// Statystyki liczone na losowej próbce cząstek (sąsiadami są zawsze wszystkie cząstki).
const SUBSAMPLE: usize = 20_000;
const TYPES: usize = 3;
const SEEDS: [u64; 6] = [101, 202, 303, 404, 505, 606];
const SNAPSHOTS: [u64; 4] = [0, 200, 1000, 3000];

#[derive(Clone, Copy, Default)]
struct Row {
  species: u32,
  speed: f64,
  nw: f64,
  attraction: f64,
  boid: f64,
  sep: f64,
  align: f64,
  coh: f64,
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
  let (mut f, mut count, mut same) = ((0.0, 0.0), 0.0, 0.0);
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
    let coeff = spec.interactions[other.species as usize].force_coeffs[me.species as usize] as f64;
    let att = coeff * (1.0 - ((d - u.kernel_middle as f64) / u.kernel_width as f64).abs()).max(0.0);
    f.0 += att * dirx;
    f.1 += att * diry;
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
  let scale = if count > 0.0 {
    (u.average_neighbours as f64 / count).min(1.0)
  } else {
    1.0
  };
  let attraction = (f.0 * scale, f.1 * scale);
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
  Row {
    species: me.species,
    speed: (vx * vx + vy * vy).sqrt(),
    nw: same / count.max(1.0),
    attraction: norm(attraction),
    boid: norm(boid),
    sep: norm(sep),
    align: norm(align),
    coh: norm(coh),
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

/// `p10,p50,p90,p99` posortowanej kopii.
fn pcts(mut v: Vec<f64>) -> [f64; 4] {
  v.sort_by(|a, b| a.total_cmp(b));
  [0.1, 0.5, 0.9, 0.99].map(|q| percentile(&v, q))
}

fn stat_line(label: &str, group: &str, rows: &[&Row]) -> String {
  let n = rows.len();
  let col = |f: fn(&Row) -> f64| pcts(rows.iter().map(|r| f(r)).collect());
  let ratio: Vec<f64> = rows
    .iter()
    .filter(|r| r.boid > 0.0)
    .map(|r| r.attraction / r.boid)
    .collect();
  let zero_frac = rows.iter().filter(|r| r.boid == 0.0).count() as f64 / n.max(1) as f64;
  let mut s = format!("{label},{group},{n}");
  for p in [
    col(|r| r.attraction),
    col(|r| r.boid),
    col(|r| r.sep),
    col(|r| r.align),
    col(|r| r.coh),
    pcts(ratio),
  ] {
    for x in p {
      write!(s, ",{x:.4}").unwrap();
    }
  }
  write!(s, ",{zero_frac:.4}").unwrap();
  s
}

#[test]
#[ignore = "pomiar: uruchamiany ręcznie, zapisuje do .artifacts/force-scales/"]
fn measure_force_scales() {
  let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(".artifacts/force-scales");
  std::fs::create_dir_all(&dir).unwrap();
  for n in COUNTS {
    if !measure(n, &dir) {
      return;
    }
  }
}

/// Zwraca `false`, gdy brak GPU.
fn measure(n: usize, dir: &std::path::Path) -> bool {
  let mut raw = String::from("seed,step,id,species,speed,nw,attraction,boid,sep,align,coh\n");
  let mut all: Vec<(u64, u64, Vec<Row>)> = Vec::new();
  for seed in SEEDS {
    let mut rng = StdRng::seed_from_u64(seed);
    let spec = spec(&mut rng, TYPES);
    let particles = initial_particles(n, TYPES, FS_WORLD, &mut rng);
    let initial = particles.clone();
    let Some(mut harness) = Harness::new(SimOptions::default(), spec.clone(), particles, FS_WORLD)
    else {
      eprintln!("brak GPU, pomijam");
      return false;
    };
    let mut done = 0;
    for step in SNAPSHOTS {
      // Krok 0: stan początkowy (odczyt z GPU przed pierwszym krokiem nie działa).
      let state = if step == 0 {
        initial.clone()
      } else {
        harness.run(step - done)
      };
      done = step;
      assert_invariants(&state, FS_WORLD, &format!("n {n} seed {seed} krok {step}"));
      let mut ids: Vec<usize> = (0..n).collect();
      ids.shuffle(&mut rng);
      ids.truncate(SUBSAMPLE.min(n));
      let rows = snapshot_rows(&spec, &state, &ids);
      for (id, r) in ids.iter().zip(&rows) {
        writeln!(
          raw,
          "{seed},{step},{id},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4}",
          r.species, r.speed, r.nw, r.attraction, r.boid, r.sep, r.align, r.coh
        )
        .unwrap();
      }
      eprintln!("n {n} seed {seed} krok {step} gotowe");
      all.push((seed, step, rows));
    }
  }
  std::fs::write(dir.join(format!("raw_per_particle_n{n}.csv")), raw).unwrap();
  let mut out = String::from(
    "label,group,n,\
     att_p10,att_p50,att_p90,att_p99,boid_p10,boid_p50,boid_p90,boid_p99,\
     sep_p10,sep_p50,sep_p90,sep_p99,align_p10,align_p50,align_p90,align_p99,\
     coh_p10,coh_p50,coh_p90,coh_p99,ratio_p10,ratio_p50,ratio_p90,ratio_p99,boid_zero_frac\n",
  );
  let nw_bins = [
    ("nw<0.1", 0.0, 0.1),
    ("nw0.1-0.3", 0.1, 0.3),
    ("nw0.3-0.6", 0.3, 0.6),
    ("nw>0.6", 0.6, 9.0),
  ];
  let v_bins = [
    ("v<10", 0.0, 10.0),
    ("v10-40", 10.0, 40.0),
    ("v40-100", 40.0, 100.0),
    ("v>100", 100.0, 1e9),
  ];
  let mut emit = |label: String, rows: Vec<&Row>| {
    out += &stat_line(&label, "all", &rows);
    out.push('\n');
    for (name, lo, hi) in nw_bins {
      let sub: Vec<&Row> = rows
        .iter()
        .copied()
        .filter(|r| r.nw >= lo && r.nw < hi)
        .collect();
      out += &stat_line(&label, name, &sub);
      out.push('\n');
    }
    for (name, lo, hi) in v_bins {
      let sub: Vec<&Row> = rows
        .iter()
        .copied()
        .filter(|r| r.speed >= lo && r.speed < hi)
        .collect();
      out += &stat_line(&label, name, &sub);
      out.push('\n');
    }
  };
  for (seed, step, rows) in &all {
    emit(format!("seed{seed}/step{step}"), rows.iter().collect());
  }
  for seed in SEEDS {
    emit(
      format!("seed{seed}/allsteps"),
      all
        .iter()
        .filter(|(s, ..)| *s == seed)
        .flat_map(|(.., r)| r)
        .collect(),
    );
  }
  for step in SNAPSHOTS {
    emit(
      format!("pooled/step{step}"),
      all
        .iter()
        .filter(|(_, t, _)| *t == step)
        .flat_map(|(.., r)| r)
        .collect(),
    );
  }
  emit(
    "pooled/all".into(),
    all.iter().flat_map(|(.., r)| r).collect(),
  );
  std::fs::write(dir.join(format!("summary_n{n}.csv")), out).unwrap();
  true
}
