//! Wspólna część benchmarków: scena (świat, macierz sił z ziarna), statystyki i zapis wyników
//! jako Parquet (format w `results`).
//!
//! Wyniki ogląda się w `tools/bench-viz/index.html` (otworzyć w przeglądarce, wczytać pliki
//! `.parquet`), porównuje przez `tools/bench-ab/ab.sh` albo zapytaniami, np.
//! `duckdb -c "select … from '<katalog>/gpu-*.parquet'"`. Stare wyniki benchów CPU (`sim`,
//! `grid`; usunięte razem z silnikiem CPU) w `.artifacts/bench` nadal czyta `bench-viz`.

#![allow(dead_code)]

pub mod results;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use bevy::prelude::*;
use rand::prelude::*;

use clap::Args;
use partikl::gpu::{RuleParams, RuleSource};
use partikl::options::SimOptions;
use partikl::spec::{Interaction, ParticleSpec};
use results::{ResultsFile, Run};

/// Rozmiar świata benchmarków (stały, żeby wyniki były porównywalne między maszynami).
pub const WORLD: Vec2 = Vec2::new(2560.0, 1440.0);

pub fn rng(seed: u64) -> StdRng {
  StdRng::seed_from_u64(seed)
}

/// Macierz sił z tym samym rozkładem co `loading::generate_particle_spec`.
pub fn particle_spec(types: usize, seed: u64) -> ParticleSpec {
  let mut rng = rng(seed);
  let interactions = (0..types)
    .map(|_| Interaction {
      force_coeffs: (0..types)
        .map(|_| rng.random_range(-500.0..500.0))
        .collect(),
    })
    .collect();
  ParticleSpec::new(interactions)
}

/// Flagi wspólne dla benchów: opcje reguł i częściowe nadpisanie parametrów (`#[command(flatten)]`).
#[derive(Args, Debug, Clone, Default)]
pub struct SimArgs {
  /// Opcja reguł `nazwa=wybór` (powtarzalna), nakładana na domyślne, np. `--option resolve_kick=vel`.
  #[arg(long = "option", value_name = "NAZWA=WYBÓR")]
  pub options: Vec<String>,
  /// Częściowe nadpisanie `RuleParams` w RON, np. `--params "(repulsion_radius: 1.0)"`.
  #[arg(long)]
  pub params: Option<String>,
}

impl SimArgs {
  /// Opcje z flag `--option` (błąd dla nieznanej opcji albo wyboru kończy program).
  pub fn sim_options(&self) -> SimOptions {
    let schema = RuleSource::Builtin.read().expect("wbudowane reguły").schema;
    SimOptions::from_overrides(&schema, &self.options).unwrap_or_else(|e| panic!("--option: {e}"))
  }

  /// Nakłada `--params` na parametry specyfikacji (`dt` i próg ruchu zostają, jak w plikach `.ron`).
  pub fn apply_params(&self, spec: &mut ParticleSpec) {
    let Some(text) = &self.params else { return };
    let over: RuleParams = ron::from_str(text).unwrap_or_else(|e| panic!("--params: {e}"));
    // Pola nieobecne w tekście mają wartości domyślne (`serde(default)`), a spec benchów ma domyślne
    // parametry, więc wystarczy podmienić całość; `dt` i próg ruchu (`serde(skip)`) zostają.
    let (dt, threshold) = (spec.params.dt, spec.params.velocity_threshold);
    spec.params = over;
    spec.params.dt = dt;
    spec.params.velocity_threshold = threshold;
  }

  /// Opis wybranych opcji i parametrów (do kolumny `variant`); pusty, gdy nic nie ustawiono.
  pub fn label(&self) -> String {
    let mut parts: Vec<String> = self.options.clone();
    if let Some(params) = &self.params {
      parts.push(format!(
        "params={}",
        params.split_whitespace().collect::<Vec<_>>().join("")
      ));
    }
    if parts.is_empty() {
      String::new()
    } else {
      format!(" [{}]", parts.join(" "))
    }
  }
}

pub fn nanos(d: Duration) -> u64 {
  d.as_nanos() as u64
}

pub fn median(values: &mut [f64]) -> f64 {
  values.sort_by(|a, b| a.total_cmp(b));
  let m = values.len() / 2;
  if values.len().is_multiple_of(2) {
    (values[m - 1] + values[m]) / 2.0
  } else {
    values[m]
  }
}

/// Commit z flagą `-dirty`, jeśli drzewo ma niezatwierdzone zmiany.
pub fn git_revision() -> String {
  let run = |args: &[&str]| {
    Command::new("git")
      .args(args)
      .output()
      .ok()
      .filter(|o| o.status.success())
  };
  let Some(hash) = run(&["rev-parse", "--short", "HEAD"]) else {
    return "unknown".into();
  };
  let hash = String::from_utf8_lossy(&hash.stdout).trim().to_string();
  let dirty =
    run(&["status", "--porcelain", "--untracked-files=no"]).is_some_and(|o| !o.stdout.is_empty());
  if dirty { format!("{hash}-dirty") } else { hash }
}

/// Plik wyników przebiegu (`results`): `<bench>-<start>.parquet` w `out_dir`.
pub struct Output {
  file: ResultsFile,
  pub path: PathBuf,
  pub run: Run,
}

impl Output {
  /// `world`, `warmup` i `measured` trafiają do metadanych pliku (opis przebiegu).
  pub fn new(
    bench: &str,
    out_dir: &Path,
    world: Vec2,
    warmup: usize,
    measured: usize,
    schema: SchemaRef,
  ) -> Self {
    fs::create_dir_all(out_dir).expect("cannot create output directory");
    let started = chrono::Local::now();
    let path = out_dir.join(format!(
      "{bench}-{}.parquet",
      started.format("%Y-%m-%d-%H-%M-%S")
    ));
    let run = Run {
      bench: bench.to_string(),
      revision: git_revision(),
      started: started.to_rfc3339(),
      warmup,
      measured,
      world: [world.x, world.y],
    };
    let file = ResultsFile::create(path.clone(), schema, &run);
    Output { file, path, run }
  }

  pub fn write(&mut self, batch: &RecordBatch) {
    self.file.write(batch);
  }

  /// Domyka plik; bez tego zostaje tylko `<plik>.parquet.partial`.
  pub fn finish(self) {
    self.file.finish();
  }
}
