//! Indeks przestrzenny na pozycjach ze stanów symulacji (te same `n`, offsety i macierze co
//! w `sim`). Mierzy budowę, zapytania o sąsiadów dla każdej cząstki i przeniesienie cząstek
//! o jeden krok symulacji. Tylko publiczne API `SpatialGrid`.
//!
//! Stany są zapisywane w `.data/bench-cache/grid-states/`, więc kolejne przebiegi (np. przed
//! i po zmianie siatki) mierzą identyczne dane. `--regenerate` wymusza ponowną symulację.
//! `cargo bench --bench grid -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use bevy::prelude::*;
use clap::Parser;
use serde_json::{Value, json};

use common::{BenchArgs, Output, Progress, Sample, Sim, WORLD, nanos};
use partikl::config::INTERACTION_RADIUS;
use partikl::sim::{InteractionId, SpatialGrid};

#[derive(Parser, Debug)]
struct GridArgs {
  #[command(flatten)]
  common: BenchArgs,
  /// Ignoruje zapisane stany i symuluje od nowa.
  #[arg(long)]
  regenerate: bool,
  /// Katalog zapisanych stanów.
  #[arg(long, default_value = ".data/bench-cache/grid-states")]
  cache_dir: PathBuf,
}

/// Pozycje cząstek w danym stanie i krok później (do pomiaru przenoszenia).
struct State {
  positions: Vec<Vec2>,
  next: Vec<Vec2>,
  revision: String,
}

fn cache_path(dir: &PathBuf, s: &Sample, offset: usize) -> PathBuf {
  dir.join(format!(
    "n{}-t{}-s{}-p{}-o{offset}.json",
    s.n, s.types, s.spec_seed, s.placement_seed
  ))
}

fn to_json(points: &[Vec2]) -> Value {
  json!(points.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>())
}

fn from_json(value: &Value) -> Vec<Vec2> {
  value
    .as_array()
    .unwrap()
    .iter()
    .map(|p| Vec2::new(p[0].as_f64().unwrap() as f32, p[1].as_f64().unwrap() as f32))
    .collect()
}

fn load(path: &PathBuf) -> Option<State> {
  let value: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
  Some(State {
    positions: from_json(&value["positions"]),
    next: from_json(&value["next"]),
    revision: value["revision"].as_str()?.to_string(),
  })
}

/// Stany dla wszystkich offsetów jednej trajektorii: z cache albo z symulacji.
fn states(args: &GridArgs, sample: &Sample) -> Vec<State> {
  let offsets = &args.common.offsets;
  let paths: Vec<PathBuf> = offsets.iter().map(|&o| cache_path(&args.cache_dir, sample, o)).collect();
  if !args.regenerate {
    let cached: Option<Vec<State>> = paths.iter().map(load).collect();
    if let Some(states) = cached {
      return states;
    }
  }

  fs::create_dir_all(&args.cache_dir).expect("cannot create cache directory");
  let revision = common::git_revision();
  let mut sim = Sim::new(sample);
  let mut steps_done = 0;
  let mut states = vec![];
  for (&offset, path) in offsets.iter().zip(&paths) {
    while steps_done < offset {
      sim.step();
      steps_done += 1;
    }
    let positions = sim.positions();
    sim.step();
    steps_done += 1;
    let next = sim.positions();
    let value = json!({
      "revision": revision,
      "positions": to_json(&positions),
      "next": to_json(&next),
    });
    fs::write(path, serde_json::to_vec(&value).unwrap()).expect("cannot write cache");
    states.push(State { positions, next, revision: revision.clone() });
  }
  states
}

fn build(entities: &[Entity], positions: &[Vec2]) -> SpatialGrid {
  let mut grid = SpatialGrid::new(WORLD.x, WORLD.y, INTERACTION_RADIUS);
  for (entity, p) in entities.iter().zip(positions) {
    grid.insert_entity(*entity, p.x, p.y, InteractionId(0));
  }
  grid
}

/// Czasy `measured` powtórzeń po `warmup` niemierzonych. `setup` poza pomiarem.
fn measure<S>(
  args: &BenchArgs,
  mut setup: impl FnMut() -> S,
  mut routine: impl FnMut(&mut S),
) -> Vec<u64> {
  let mut times = vec![];
  for i in 0..args.warmup + args.measured {
    let mut input = setup();
    let start = Instant::now();
    routine(&mut input);
    let elapsed = start.elapsed();
    black_box(&input);
    if i >= args.warmup {
      times.push(nanos(elapsed));
    }
  }
  times
}

const OPS: [&str; 3] = ["build", "neighbours", "move"];

const COLUMNS: [&str; 6] = ["n", "offset", "op", "p10 µs", "med µs", "p90 µs"];

fn summary_rows(summary: &BTreeMap<(usize, usize, &str), Vec<f64>>) -> Vec<Vec<String>> {
  summary
    .iter()
    .map(|((n, offset, op), values)| {
      let mut values = values.clone();
      vec![
        n.to_string(),
        offset.to_string(),
        op.to_string(),
        format!("{:.1}", common::quantile(&mut values, 0.1) / 1e3),
        format!("{:.1}", common::median(&mut values) / 1e3),
        format!("{:.1}", common::quantile(&mut values, 0.9) / 1e3),
      ]
    })
    .collect()
}

fn main() {
  let args = GridArgs::parse();
  let mut common_args = args.common.clone();
  common_args.offsets.sort_unstable();
  common_args.offsets.dedup();
  let args = GridArgs { common: common_args, ..args };
  let c = &args.common;

  let mut output = Output::new("grid", c);
  let groups = c.n.iter().map(|&n| (format!("n={n}"), n as f64, c.matrices as usize)).collect();
  let mut progress = Progress::new(&output, groups, &COLUMNS);
  let mut summary: BTreeMap<(usize, usize, &str), Vec<f64>> = BTreeMap::new();
  let mut world = World::new();

  for sample in c.samples() {
    let group = c.n.iter().position(|&n| n == sample.n).unwrap();
    progress.begin(&format!("n={} spec_seed={}", sample.n, sample.spec_seed));
    let entities: Vec<Entity> = (0..sample.n).map(|_| world.spawn_empty().id()).collect();
    for (state, &offset) in states(&args, &sample).iter().zip(&c.offsets) {
      let grid = build(&entities, &state.positions);
      let candidates_mean = common::candidates_mean(&grid, &state.positions);

      // Siatka zwalniana poza pomiarem.
      let build_ns = measure(c, || None, |grid| {
        *grid = Some(build(&entities, &state.positions));
      });
      // Każdy kandydat jest odwiedzany, jak w `compute_forces` (`count()` na spłaszczonych
      // wycinkach nie iterowałby elementów).
      let neighbours_ns = measure(c, || (), |_| {
        let mut checksum = 0u64;
        for p in &state.positions {
          for entity in grid.get_entities_by_position(p.x, p.y) {
            checksum = checksum.wrapping_add(black_box(entity).to_bits());
          }
        }
        black_box(checksum);
      });
      let move_ns = measure(
        c,
        || build(&entities, &state.positions),
        |grid| {
          for ((e, old), new) in entities.iter().zip(&state.positions).zip(&state.next) {
            grid.move_entity(*e, old.x, old.y, new.x, new.y);
          }
        },
      );

      for (op, ns) in OPS.iter().zip([&build_ns, &neighbours_ns, &move_ns]) {
        let mut record = sample.json();
        let object = record.as_object_mut().unwrap();
        object.insert("op".into(), json!(op));
        object.insert("step_offset".into(), json!(offset));
        object.insert("candidates_mean".into(), json!(candidates_mean));
        object.insert("state_revision".into(), json!(state.revision));
        object.insert("ns".into(), json!(ns));
        output.write(record);
        let mut values: Vec<f64> = ns.iter().map(|&x| x as f64).collect();
        summary.entry((sample.n, offset, op)).or_default().push(common::median(&mut values));
      }
    }
    for e in entities {
      world.despawn(e);
    }
    progress.finish_sample(group, summary_rows(&summary));
  }
  progress.finish();

  println!("\nSiatka: rozkład median po macierzach ({} na n)", c.matrices);
  println!("{}", common::format_table(&COLUMNS, &summary_rows(&summary)));
  println!("\nWyniki: {}", output.path.display());
}
