//! Skąd się biorą hipergęste skupiska (tysiące cząstek w promieniu separacji). Ten sam stan
//! początkowy i macierz, warianty różniące się jednym mechanizmem:
//! - `base`: domyślne reguły (`overlap=unweighted`, `overlap_damp=off`), `dt`;
//! - `weighted`: `overlap=weighted` (suma poprawek rozsuwania z wagą próbkowania);
//! - `dt/4`: 4 podkroki na takt (ten sam czas symulacji);
//! - `weighted dt/4`: oba naraz;
//! - `no-sampling`: bez próbkowania sąsiadów (każdy kandydat, waga 1);
//! - `no-sampling dt/4`, `weighted dt/2`: kontrolne;
//! - `damp…`: `overlap_damp=force` (siła dzielona przez `1 + liczba nakładających się`).
//!
//! Wielkości na cząstkę (CPU, wszyscy sąsiedzi): `n_sep` sąsiedzi bliżej niż `separation_radius`,
//! `n_rep` bliżej niż `repulsion_radius` (nakładające się), `n40` w zasięgu oddziaływania,
//! `speed`, `every` (krok próbkowania na GPU z liczby kandydatów siatki silnika).
//!
//! Test jest `#[ignore]`. Macierze: `CLUSTER_SPECS` (ścieżki `.ron` po przecinku, względem
//! katalogu repo), domyślnie trzy `spec-*.ron` użytkownika z 2026-11-26, a gdy ich nie ma, macierze
//! losowe z `macro_stats`. `CLUSTER_VARIANTS` filtruje warianty (nazwy po przecinku). Wynik:
//! tabela na stderr i `.artifacts/clusters/summary.csv`.
//! `cargo test --release --quiet measure_clusters -- --ignored --nocapture`

use super::force_scales::gpu_candidates;
use super::macro_stats::{Grid, MS_N, MS_WORLD, matrices, par_rows};
use super::*;
use std::fmt::Write as _;
use std::path::Path;

const SAMPLE: usize = 5_000;
const SNAPSHOTS: [u64; 4] = [500, 1000, 2000, 3000];
const DEFAULT_SPECS: [&str; 3] = [
  "spec-2026-11-26-21-32-50.ron",
  "spec-2026-11-26-21-32-09.ron",
  "spec-2026-11-26-20-03-03.ron",
];

struct Variant {
  name: &'static str,
  /// Wybory opcji reguł (`nazwa`, `wybór`).
  options: &'static [(&'static str, &'static str)],
  substeps: u32,
  sampling: bool,
}

const VARIANTS: [Variant; 10] = [
  Variant {
    name: "base",
    options: &[],
    substeps: 1,
    sampling: true,
  },
  Variant {
    name: "weighted",
    options: &[("overlap", "weighted")],
    substeps: 1,
    sampling: true,
  },
  Variant {
    name: "dt/4",
    options: &[],
    substeps: 4,
    sampling: true,
  },
  Variant {
    name: "weighted dt/4",
    options: &[("overlap", "weighted")],
    substeps: 4,
    sampling: true,
  },
  Variant {
    name: "no-sampling",
    options: &[],
    substeps: 1,
    sampling: false,
  },
  Variant {
    name: "no-sampling dt/4",
    options: &[],
    substeps: 4,
    sampling: false,
  },
  Variant {
    name: "weighted dt/2",
    options: &[("overlap", "weighted")],
    substeps: 2,
    sampling: true,
  },
  Variant {
    name: "damp",
    options: &[("overlap_damp", "force")],
    substeps: 1,
    sampling: true,
  },
  Variant {
    name: "damp weighted",
    options: &[("overlap_damp", "force"), ("overlap", "weighted")],
    substeps: 1,
    sampling: true,
  },
  Variant {
    name: "damp weighted dt/2",
    options: &[("overlap_damp", "force"), ("overlap", "weighted")],
    substeps: 2,
    sampling: true,
  },
];

/// Wielkości jednej cząstki: `[n_sep, n_rep, n40, speed, every]`.
type Row = [f64; 5];
const COLUMNS: [&str; 5] = ["n_sep", "n_rep", "n40", "speed", "every"];

fn specs() -> Vec<(String, ParticleSpec)> {
  let root = Path::new(env!("CARGO_MANIFEST_DIR"));
  let paths: Vec<String> = match std::env::var("CLUSTER_SPECS") {
    Ok(s) => s.split(',').map(|x| x.trim().to_string()).collect(),
    Err(_) => DEFAULT_SPECS
      .iter()
      .filter(|p| root.join(p).exists())
      .map(|p| p.to_string())
      .collect(),
  };
  if paths.is_empty() {
    return matrices()
      .into_iter()
      .filter(|(name, _, _)| name.starts_with("random"))
      .map(|(name, _, spec)| (name, spec))
      .collect();
  }
  paths
    .into_iter()
    .map(|p| {
      let spec = crate::loading::load_spec(&root.join(&p)).unwrap_or_else(|e| panic!("{p}: {e:?}"));
      let name = p
        .trim_start_matches("spec-")
        .trim_end_matches(".ron")
        .to_string();
      (name, spec)
    })
    .collect()
}

fn row(spec: &ParticleSpec, state: &[GpuParticle], grid: &Grid, every: &[f64], i: usize) -> Row {
  let u = &spec.params;
  let (sep, rep) = (u.separation_radius as f64, u.repulsion_radius as f64);
  let (mut n_sep, mut n_rep, mut n40) = (0.0, 0.0, 0.0);
  grid.for_neighbours(state, i, |_, _, _, d| {
    n40 += 1.0;
    n_sep += f64::from(d < sep);
    n_rep += f64::from(d < rep);
  });
  let v = state[i].vel;
  let speed = ((v[0] * v[0] + v[1] * v[1]) as f64).sqrt();
  [n_sep, n_rep, n40, speed, every[i]]
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
  sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

#[test]
#[ignore = "pomiar: uruchamiany ręcznie, zapisuje do .artifacts/"]
fn measure_clusters() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(".artifacts/clusters");
  std::fs::create_dir_all(&dir).unwrap();
  let filter: Vec<String> = std::env::var("CLUSTER_VARIANTS")
    .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
    .unwrap_or_default();
  let mut csv = String::from("matrix,variant,step,quantity,p50,p90,p99,max,mean,frac_ge_20\n");
  for (k, (name, spec)) in specs().into_iter().enumerate() {
    let types = spec.interactions.len();
    let mut rng = StdRng::seed_from_u64(500 + k as u64);
    let initial = initial_particles(MS_N, types, MS_WORLD, &mut rng);
    for variant in &VARIANTS {
      if !filter.is_empty() && !filter.iter().any(|f| f == variant.name) {
        continue;
      }
      let mut options = SimOptions::default();
      for (name, choice) in variant.options {
        options = options.with(name, choice);
      }
      let Some(mut harness) = Harness::new(options, spec.clone(), initial.clone(), MS_WORLD) else {
        eprintln!("brak GPU, pomijam");
        return;
      };
      harness.set_substeps(variant.substeps);
      if !variant.sampling {
        harness.set_max_candidates(u32::MAX / 2);
      }
      let started = std::time::Instant::now();
      let mut done = 0;
      for step in SNAPSHOTS {
        let state = harness.run(step - done);
        done = step;
        let grid = Grid::new(&state);
        let cap = if variant.sampling {
          DEFAULT_MAX_CANDIDATES
        } else {
          u32::MAX / 2
        };
        let every: Vec<f64> = gpu_candidates(&state)
          .into_iter()
          .map(|c| c.div_ceil(cap).max(1) as f64)
          .collect();
        let mut ids: Vec<usize> = (0..MS_N).collect();
        ids.shuffle(&mut rng);
        ids.truncate(SAMPLE);
        let rows = par_rows(&ids, |i| row(&spec, &state, &grid, &every, i));
        let finite = state
          .iter()
          .all(|p| p.pos.iter().chain(&p.vel).all(|x| x.is_finite()));
        let mut line = format!(
          "{name:>20} {:>14} krok {step:>4}{}:",
          variant.name,
          if finite { "" } else { " NaN!" }
        );
        for (c, col) in COLUMNS.iter().enumerate() {
          let mut v: Vec<f64> = rows.iter().map(|r| r[c]).collect();
          v.sort_by(f64::total_cmp);
          let mean = v.iter().sum::<f64>() / v.len() as f64;
          let ge20 = v.iter().filter(|x| **x >= 20.0).count() as f64 / v.len() as f64;
          let (p50, p90, p99, max) = (
            quantile(&v, 0.5),
            quantile(&v, 0.9),
            quantile(&v, 0.99),
            v[v.len() - 1],
          );
          writeln!(
            csv,
            "{name},{},{step},{col},{p50},{p90},{p99},{max},{mean:.3},{ge20:.4}",
            variant.name
          )
          .unwrap();
          write!(line, "  {col} {p50:.0}/{p90:.0}/{p99:.0}/{max:.0}").unwrap();
          if *col == "n_sep" {
            write!(line, " (≥20: {:.1}%)", 100.0 * ge20).unwrap();
          }
        }
        eprintln!("{line}");
      }
      eprintln!("  ({:.0} s)", started.elapsed().as_secs_f64());
    }
  }
  std::fs::write(dir.join("summary.csv"), csv).unwrap();
}
