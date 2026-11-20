//! Wspólna część benchmarków `sim` i `grid`: deterministyczne sceny, krokowanie symulacji,
//! parametry z linii poleceń i zapis wyników jako JSONL.
//!
//! Każda próbka to stan symulacji wyznaczony przez `(n, macierz, step_offset)`: `n` cząstek,
//! macierz sił z ziarna `spec_seed`, rozmieszczenie z ziarna `placement_seed`, po
//! `step_offset` krokach. Z jednej trajektorii na macierz bierzemy wszystkie offsety.
//!
//! Wyniki ogląda się w `tools/bench-viz/index.html` (otworzyć w przeglądarce, wczytać pliki JSONL).

#![allow(dead_code)]

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use bevy::app::TaskPoolPlugin;
use bevy::ecs::system::SystemId;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use clap::Parser;
use rand::prelude::*;
use serde_json::{Value, json};

use partikl::config::{DELTA_TIME, INTERACTION_RADIUS};
use partikl::sim::{
  Acceleration, InteractionId, LastPosition, ParticleBundle, SimPlugin, SpatialGrid, compute_forces,
  task_pool_options,
  compute_friction, integrate, update_shape, wrap_around,
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
  /// Mierzone powtórzenia w każdym punkcie.
  #[arg(long, default_value_t = 10)]
  pub measured: usize,
  /// Katalog wyników.
  #[arg(long, default_value = ".data/bench-results")]
  pub out_dir: PathBuf,
  /// Przekazywane przez `cargo bench`; ignorowane.
  #[arg(long, hide = true)]
  pub bench: bool,
}

impl BenchArgs {
  pub fn parse_sorted() -> Self {
    let mut args = Self::parse();
    args.offsets.sort_unstable();
    args.offsets.dedup();
    args
  }

  pub fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
    self.n.iter().flat_map(move |&n| {
      (0..self.matrices).map(move |i| Sample::new(n, self.types, self.seed_base + i))
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

  pub fn json(&self) -> Value {
    json!({
      "n": self.n,
      "types": self.types,
      "spec_seed": self.spec_seed,
      "placement_seed": self.placement_seed,
      "interactions": self.spec().interactions.iter().map(|i| &i.force_coeffs).collect::<Vec<_>>(),
    })
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
  systems: [SystemId; 5],
}

pub const SYSTEM_NAMES: [&str; 5] =
  ["compute_forces", "compute_friction", "integrate", "wrap_around", "update_shape"];

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
      grid.insert_entity(entity, position.x, position.y, interaction);
    }
    app.insert_resource(grid);
    app.finish();
    app.cleanup();

    // Systemy uruchamiane ręcznie w kolejności `SimSet`, bez harmonogramu i czasu Bevy.
    let world = app.world_mut();
    let systems = [
      world.register_system(compute_forces),
      world.register_system(compute_friction),
      world.register_system(integrate),
      world.register_system(wrap_around),
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
  pub fn step_timed(&mut self) -> (Duration, [Duration; 5]) {
    let mut per_system = [Duration::ZERO; 5];
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

/// Zapis wyników: jedna linia JSON na rekord, z wspólnymi metadanymi przebiegu.
pub struct Output {
  writer: BufWriter<File>,
  pub path: PathBuf,
  pub run: Value,
}

impl Output {
  pub fn new(bench: &str, args: &BenchArgs) -> Self {
    fs::create_dir_all(&args.out_dir).expect("cannot create output directory");
    let started = chrono::Local::now();
    let path = args.out_dir.join(format!("{bench}-{}.jsonl", started.format("%Y-%m-%d-%H-%M-%S")));
    let writer = BufWriter::new(File::create(&path).expect("cannot create output file"));
    let run = json!({
      "bench": bench,
      "revision": git_revision(),
      "started": started.to_rfc3339(),
      "warmup": args.warmup,
      "measured": args.measured,
      "world": [WORLD.x, WORLD.y],
    });
    Output { writer, path, run }
  }

  pub fn write(&mut self, mut record: Value) {
    let object = record.as_object_mut().expect("record must be an object");
    object.insert("run".into(), self.run.clone());
    serde_json::to_writer(&mut self.writer, &record).unwrap();
    self.writer.write_all(b"\n").unwrap();
    self.writer.flush().unwrap();
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
        "bench": output.run["bench"],
        "revision": output.run["revision"],
        "started": output.run["started"],
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
