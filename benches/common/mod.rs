//! Wspólna część benchmarków `sim` i `grid`: deterministyczne sceny, krokowanie symulacji,
//! parametry z linii poleceń i zapis wyników jako Parquet (format w `results`).
//!
//! Każda próbka to stan symulacji wyznaczony przez `(n, macierz, step_offset)`: `n` cząstek,
//! macierz sił z ziarna `spec_seed`, rozmieszczenie z ziarna `placement_seed`, po
//! `step_offset` krokach. Z jednej trajektorii na macierz bierzemy wszystkie offsety.
//!
//! Wyniki ogląda się w `tools/bench-viz/index.html` (otworzyć w przeglądarce, wczytać pliki
//! `.parquet`) albo zapytaniami, np. `duckdb -c "select … from '<katalog>/sim-*.parquet'"`.

#![allow(dead_code)]

pub mod results;

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use bevy::app::TaskPoolPlugin;
use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use clap::Parser;
use rand::prelude::*;
use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use serde_json::{Value, json};

use results::{ResultsFile, Run};

use partikl::config::{DELTA_TIME, INTERACTION_RADIUS};
use partikl::sim::{
  Acceleration, InteractionId, LastPosition, ParticleBundle, SimOptions, SimPlugin, SpatialGrid,
  register_forces, task_pool_options,
  compute_friction, integrate, rebuild_grid, update_shape, wrap_around,
};
use partikl::spec::{Interaction, ParticleSpec};

/// Rozmiar świata, jak domyślne okno w `main.rs`.
pub const WORLD: Vec2 = Vec2::new(2560.0, 1440.0);

#[derive(Parser, Debug, Clone)]
pub struct BenchArgs {
  /// Liczby cząstek.
  #[arg(long, value_delimiter = ',', default_value = "1000,3000,9000")]
  pub n: Vec<usize>,
  /// Liczby kroków symulacji przed pomiarem.
  #[arg(long, value_delimiter = ',', default_value = "256,1024,4096")]
  pub offsets: Vec<usize>,
  /// Liczba macierzy sił (i rozmieszczeń) na każde `n`.
  #[arg(long, default_value_t = 32)]
  pub matrices: u64,
  /// Pierwsze ziarno; macierz `i` używa `seed_base + i`.
  #[arg(long, default_value_t = 0)]
  pub seed_base: u64,
  /// Liczba typów cząstek.
  #[arg(long, default_value_t = 3)]
  pub types: usize,
  /// Niemierzone powtórzenia przed pomiarem (zimny cache po zmianie stanu).
  #[arg(long, default_value_t = 3)]
  pub warmup: usize,
  /// Mierzone powtórzenia w każdym punkcie; domyślnie 10, w trybie szybkim `QUICK_MEASURED`.
  #[arg(long = "measured")]
  measured_arg: Option<usize>,
  /// Mierzone powtórzenia po `normalize`.
  #[arg(skip)]
  pub measured: usize,
  /// Katalog wyników.
  #[arg(long, default_value = ".artifacts/bench")]
  pub out_dir: PathBuf,
  /// Tryb szybki do sprawdzania pomysłów: tylko `n` = 9000, tylko największy offset, trzy
  /// macierze z `QUICK_SEEDS` i `QUICK_MEASURED` powtórzeń. Nadpisuje `--n`, `--matrices` i
  /// `--seed-base`.
  #[arg(long)]
  pub quick: bool,
  /// Przekazywane przez `cargo bench`; ignorowane.
  #[arg(long, hide = true)]
  pub bench: bool,
}

/// `n` w trybie szybkim: optymalizujemy pod duże `n`.
pub const QUICK_N: usize = 9000;

/// Domyślne mierzone powtórzenia w trybie szybkim. Każda z trzech macierzy jest osobnym
/// przypadkiem, więc wnioski opierają się na rozkładzie powtórzeń w jednym punkcie, a nie na
/// rozrzucie między macierzami; 100 kroków to ok. 0,1 s przy n = 9000.
pub const QUICK_MEASURED: usize = 100;

/// Macierze trybu szybkiego (3 typy): wolny, typowy i szybki wzrost średniej liczby
/// kandydatów na sąsiada przy n = 9000 między offsetem 256 a 4096. Wybrane z 18 przebiegów
/// `sim` z 2026-11-20 (32 macierze, różne rewizje); kolejność macierzy według tego wzrostu
/// prawie się nie zmienia między przebiegami (Spearman ≥ 0,97), choć trajektorie nie są
/// identyczne co do bitu.
/// - 23: ×1,0 (ok. 36 kandydatów na każdym offsecie), jak 7 z 32 macierzy;
/// - 6: ×3,3 (42 → 140), mediana wzrostu;
/// - 4: ×26 (46 → 1211), jedna z najszybszych, z małym rozrzutem między przebiegami.
pub const QUICK_SEEDS: [u64; 3] = [23, 6, 4];

impl BenchArgs {
  pub fn parse_sorted() -> Self {
    let mut args = Self::parse();
    args.normalize();
    args
  }

  /// Sortuje offsety i stosuje `--quick`.
  pub fn normalize(&mut self) {
    self.offsets.sort_unstable();
    self.offsets.dedup();
    self.measured = self.measured_arg.unwrap_or(if self.quick { QUICK_MEASURED } else { 10 });
    if self.quick {
      self.n = vec![QUICK_N];
      self.offsets = self.offsets.last().into_iter().copied().collect();
    }
  }

  /// Ziarna macierzy, te same dla każdego `n`.
  pub fn seeds(&self) -> Vec<u64> {
    if self.quick {
      QUICK_SEEDS.to_vec()
    } else {
      (0..self.matrices).map(|i| self.seed_base + i).collect()
    }
  }

  pub fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
    let seeds = self.seeds();
    self.n.iter().flat_map(move |&n| {
      seeds.clone().into_iter().map(move |seed| Sample::new(n, self.types, seed))
    })
  }
}

/// Jedna trajektoria: `n` cząstek, macierz i rozmieszczenie z ziaren.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
  pub n: usize,
  pub types: usize,
  pub spec_seed: u64,
  pub placement_seed: u64,
}

impl Sample {
  fn new(n: usize, types: usize, seed: u64) -> Self {
    // Osobne strumienie dla macierzy i rozmieszczenia, ten sam indeks macierzy dla każdego `n`.
    Sample { n, types, spec_seed: seed, placement_seed: seed + (1 << 32) }
  }

  pub fn spec(&self) -> ParticleSpec {
    particle_spec(self.types, self.spec_seed)
  }

}

pub fn rng(seed: u64) -> StdRng {
  StdRng::seed_from_u64(seed)
}

/// Macierz sił z tym samym rozkładem co `loading::generate_particle_spec`.
pub fn particle_spec(types: usize, seed: u64) -> ParticleSpec {
  let mut rng = rng(seed);
  let interactions = (0..types)
    .map(|_| Interaction {
      force_coeffs: (0..types).map(|_| rng.random_range(-500.0..500.0)).collect(),
    })
    .collect();
  ParticleSpec { interactions }
}

/// Symulacja bez okna i renderu. Cząstki jak w `render::init_particles`: jednostajnie w
/// świecie, prędkość startowa ±125 na oś.
pub struct Sim {
  pub app: App,
  systems: [SystemId; SYSTEMS],
}

/// Liczba systemów kroku (kolumny `<system>_ns` w wynikach).
pub const SYSTEMS: usize = 6;

pub const SYSTEM_NAMES: [&str; SYSTEMS] =
  ["compute_forces", "compute_friction", "integrate", "wrap_around", "rebuild_grid", "update_shape"];

impl Sim {
  pub fn new(sample: &Sample) -> Self {
    let mut app = App::new();
    app
      .add_plugins((
        MinimalPlugins.set(TaskPoolPlugin { task_pool_options: task_pool_options() }),
        StatesPlugin,
        SimPlugin,
      )).insert_resource(sample.spec());

    let mut rng = rng(sample.placement_seed);
    let mut grid = SpatialGrid::new(WORLD.x, WORLD.y, INTERACTION_RADIUS);
    let mut placed = Vec::with_capacity(sample.n);
    for _ in 0..sample.n {
      let position = Vec2::new(
        rng.random_range(-WORLD.x / 2.0..WORLD.x / 2.0),
        rng.random_range(-WORLD.y / 2.0..WORLD.y / 2.0),
      );
      let velocity = Vec2::new(rng.random_range(-125.0..125.0), rng.random_range(-125.0..125.0));
      let interaction = InteractionId(rng.random_range(0..sample.types));
      let entity = app
        .world_mut()
        .spawn((
          ParticleBundle {
            last_pos: LastPosition(position - DELTA_TIME as f32 * velocity),
            acceleration: Acceleration(Vec2::ZERO),
            interaction,
          },
          Transform::from_translation(position.extend(0.0)),
        ))
        .id();
      placed.push((entity, position, interaction));
    }
    grid.rebuild(placed);
    app.insert_resource(grid);
    app.finish();
    app.cleanup();

    // Systemy uruchamiane ręcznie w kolejności `SimSet`, bez harmonogramu i czasu Bevy.
    let world = app.world_mut();
    let systems = [
      register_forces(world, SimOptions::default()),
      world.register_system(compute_friction),
      world.register_system(integrate),
      world.register_system(wrap_around),
      world.register_system(rebuild_grid),
      world.register_system(update_shape),
    ];
    Sim { app, systems }
  }

  pub fn step(&mut self) {
    for id in self.systems {
      self.app.world_mut().run_system(id).unwrap();
    }
  }

  /// Krok z czasem całego kroku i każdego systemu osobno.
  pub fn step_timed(&mut self) -> (Duration, [Duration; SYSTEMS]) {
    let mut per_system = [Duration::ZERO; SYSTEMS];
    let start = Instant::now();
    for (i, id) in self.systems.into_iter().enumerate() {
      let t = Instant::now();
      self.app.world_mut().run_system(id).unwrap();
      per_system[i] = t.elapsed();
    }
    (start.elapsed(), per_system)
  }

  pub fn positions(&mut self) -> Vec<Vec2> {
    self
      .app
      .world_mut()
      .query_filtered::<&Transform, With<Acceleration>>()
      .iter(self.app.world())
      .map(|t| t.translation.truncate())
      .collect()
  }

  /// Średnia liczba kandydatów na sąsiada zwracanych przez siatkę (z samą cząstką).
  pub fn candidates_mean(&mut self) -> f64 {
    let positions = self.positions();
    candidates_mean(self.app.world().resource::<SpatialGrid>(), &positions)
  }
}

pub fn candidates_mean(grid: &SpatialGrid, positions: &[Vec2]) -> f64 {
  let total: usize = positions.iter().map(|p| grid.get_entities_by_position(p.x, p.y).count()).sum();
  total as f64 / positions.len().max(1) as f64
}

pub fn nanos(d: Duration) -> u64 {
  d.as_nanos() as u64
}

pub fn median(values: &mut [f64]) -> f64 {
  values.sort_by(|a, b| a.total_cmp(b));
  let m = values.len() / 2;
  if values.len() % 2 == 0 { (values[m - 1] + values[m]) / 2.0 } else { values[m] }
}

pub fn quantile(values: &mut [f64], q: f64) -> f64 {
  values.sort_by(|a, b| a.total_cmp(b));
  values[((values.len() - 1) as f64 * q).round() as usize]
}

/// Commit z flagą `-dirty`, jeśli drzewo ma niezatwierdzone zmiany.
pub fn git_revision() -> String {
  let run = |args: &[&str]| {
    Command::new("git").args(args).output().ok().filter(|o| o.status.success())
  };
  let Some(hash) = run(&["rev-parse", "--short", "HEAD"]) else {
    return "unknown".into();
  };
  let hash = String::from_utf8_lossy(&hash.stdout).trim().to_string();
  let dirty = run(&["status", "--porcelain", "--untracked-files=no"])
    .is_some_and(|o| !o.stdout.is_empty());
  if dirty { format!("{hash}-dirty") } else { hash }
}

/// Plik wyników przebiegu (`results`): `<bench>-<start>.parquet` w `out_dir`.
pub struct Output {
  file: ResultsFile,
  pub path: PathBuf,
  pub run: Run,
}

impl Output {
  pub fn new(bench: &str, args: &BenchArgs, schema: SchemaRef) -> Self {
    fs::create_dir_all(&args.out_dir).expect("cannot create output directory");
    let started = chrono::Local::now();
    let path = args.out_dir.join(format!("{bench}-{}.parquet", started.format("%Y-%m-%d-%H-%M-%S")));
    let run = Run {
      bench: bench.to_string(),
      revision: git_revision(),
      started: started.to_rfc3339(),
      warmup: args.warmup,
      measured: args.measured,
      world: [WORLD.x, WORLD.y],
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

/// Postęp przebiegu: linia na stderr po każdej próbce i plik `<wyniki>.progress.json`
/// (zapisywany atomowo), który czytają narzędzia zewnętrzne, np. panel w Claude Code.
///
/// Format pliku (`schema: 1`): `bench`, `pid`, `revision`, `started`, `updated`, `state`
/// (`running` / `done`), `results`, `total`, `done`, `current`, `elapsed_s`, `eta_s`,
/// `groups` (`label`, `total`, `done`, `elapsed_s`), `columns` i `rows` (bieżąca tabela
/// podsumowania, komórki jako tekst).
pub struct Progress {
  start: Instant,
  path: PathBuf,
  header: Value,
  groups: Vec<Group>,
  current: String,
  columns: Vec<String>,
  rows: Vec<Vec<String>>,
}

struct Group {
  label: String,
  /// Względny rozmiar próbki (np. `n`) do szacowania grup jeszcze nie zaczętych.
  size: f64,
  total: usize,
  done: usize,
  elapsed: f64,
}

impl Progress {
  /// `groups`: `(etykieta, rozmiar próbki, liczba próbek)` w kolejności wykonywania.
  pub fn new(output: &Output, groups: Vec<(String, f64, usize)>, columns: &[&str]) -> Self {
    let progress = Progress {
      start: Instant::now(),
      path: output.path.with_extension("progress.json"),
      header: json!({
        "schema": 1,
        "bench": output.run.bench,
        "revision": output.run.revision,
        "started": output.run.started,
        "pid": std::process::id(),
        "results": output.path.display().to_string(),
      }),
      groups: groups
        .into_iter()
        .map(|(label, size, total)| Group { label, size, total, done: 0, elapsed: 0.0 })
        .collect(),
      current: String::new(),
      columns: columns.iter().map(|c| c.to_string()).collect(),
      rows: vec![],
    };
    progress.write("running");
    progress
  }

  /// Próbka w toku; zapisuje plik, żeby było widać, nad czym trwa praca.
  pub fn begin(&mut self, label: &str) {
    self.current = label.to_string();
    self.write("running");
  }

  /// Koniec próbki w grupie `group`, z nową tabelą podsumowania.
  pub fn finish_sample(&mut self, group: usize, rows: Vec<Vec<String>>) {
    let elapsed_total = self.start.elapsed().as_secs_f64();
    let accounted: f64 = self.groups.iter().map(|g| g.elapsed).sum();
    let g = &mut self.groups[group];
    g.done += 1;
    g.elapsed += elapsed_total - accounted;
    self.rows = rows;
    let (done, total) = self.counts();
    eprintln!(
      "[{done}/{total}] {}  elapsed {:.0}s, eta {:.0}s",
      self.current,
      elapsed_total,
      self.eta()
    );
    self.write("running");
  }

  pub fn finish(&mut self) {
    self.current.clear();
    self.write("done");
  }

  fn counts(&self) -> (usize, usize) {
    (self.groups.iter().map(|g| g.done).sum(), self.groups.iter().map(|g| g.total).sum())
  }

  /// Grupy zaczęte: średnia ich próbek; niezaczęte: średnia ostatniej zaczętej grupy
  /// przeskalowana jak `rozmiar^1.5` (między liniowym a kwadratowym kosztem sił).
  fn eta(&self) -> f64 {
    let mut last: Option<(f64, f64)> = None;
    let mut eta = 0.0;
    for g in &self.groups {
      let remaining = (g.total - g.done) as f64;
      if g.done > 0 {
        let avg = g.elapsed / g.done as f64;
        last = Some((avg, g.size));
        eta += avg * remaining;
      } else if let Some((avg, size)) = last {
        eta += avg * (g.size / size).powf(1.5) * remaining;
      }
    }
    eta
  }

  fn write(&self, state: &str) {
    let (done, total) = self.counts();
    let mut value = self.header.clone();
    let object = value.as_object_mut().unwrap();
    object.insert("state".into(), json!(state));
    object.insert("updated".into(), json!(chrono::Local::now().to_rfc3339()));
    object.insert("total".into(), json!(total));
    object.insert("done".into(), json!(done));
    object.insert("current".into(), json!(self.current));
    object.insert("elapsed_s".into(), json!(self.start.elapsed().as_secs_f64()));
    object.insert("eta_s".into(), json!(self.eta()));
    object.insert(
      "groups".into(),
      json!(self.groups.iter().map(|g| json!({
        "label": g.label, "total": g.total, "done": g.done, "elapsed_s": g.elapsed,
      })).collect::<Vec<_>>()),
    );
    object.insert("columns".into(), json!(self.columns));
    object.insert("rows".into(), json!(self.rows));
    let tmp = self.path.with_extension("tmp");
    if fs::write(&tmp, serde_json::to_vec(&value).unwrap()).is_ok() {
      let _ = fs::rename(&tmp, &self.path);
    }
  }
}

/// Wiersze tabeli jako tekst, wyrównane do szerokości kolumn.
pub fn format_table(columns: &[&str], rows: &[Vec<String>]) -> String {
  let widths: Vec<usize> = (0..columns.len())
    .map(|i| rows.iter().map(|r| r[i].chars().count()).chain([columns[i].chars().count()]).max().unwrap())
    .collect();
  let line = |cells: Vec<&str>| {
    cells.iter().zip(&widths).map(|(c, w)| format!("{c:>w$}")).collect::<Vec<_>>().join("  ")
  };
  let mut out = vec![line(columns.to_vec())];
  out.extend(rows.iter().map(|r| line(r.iter().map(|c| c.as_str()).collect())));
  out.join("\n")
}
