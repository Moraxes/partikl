//! Krok symulacji GPU: czas na krok (od wywołania do końca pracy GPU) dla liczb cząstek
//! `--sizes`, scen `--scenes` i wariantów `SimOptions`. Sceny syntetyczne (`scenes.rs`) mają
//! stałą gęstość: przed każdym pomiarem stan wraca do stanu sceny, więc każdy pomiar (i obie
//! strony porównania przed/po) liczy ten sam stan. Scena `evolved` to dawny pomiar: stan
//! losowy po `--steps` krokach rozbiegu, potem pomiary kolejno.
//!
//! `step_ns` to czas od wywołania do końca pracy GPU, z narzutem klatki Bevy (przy małym `n`
//! dominuje). `gpu_step_ns` i `gpu_<przebieg>_ns` to czysty czas GPU ze znaczników czasu
//! (`partikl::gpu::GpuTimings`); w pomiarze każdy dispatch ma osobny przebieg compute, co
//! trochę zmienia `step_ns` (`--no-gpu-timing` mierzy bez znaczników).
//! Wyniki w Parquet (kolumny: `n`, `scene`, `variant`, `spec_seed`, `step_offset`,
//! `candidates_mean`, `rep`, `step_ns`, `gpu_step_ns`, `gpu_<przebieg>_ns`), porównanie przez
//! `tools/bench-ab/ab.sh`;
//! `cargo bench --bench gpu -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;
mod scenes;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use arrow_array::{
  ArrayRef, Float32Array, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema};
use clap::Parser;
use partikl::gpu::{DEFAULT_MAX_CANDIDATES, MAX_STEPS_PER_FRAME, PassTimes};

use common::{Output, SimArgs, WORLD, git_revision, median, nanos, particle_spec, rng};
use partikl::gpu::RuleSource;
use partikl::gpu::headless::{Harness, StateMeta, load_state, save_state};
use partikl::options::SimOptions;
use scenes::{Scene, candidates_mean};

/// Macierz trybu `--quick`.
const QUICK_SEED: u64 = 23;
/// Mierzone pomiary w trybie `--quick` (bez niego domyślnie 10).
const QUICK_MEASURED: usize = 100;
/// Liczba cząstek w trybie `--quick`: optymalizujemy pod duże `n`.
const QUICK_N: usize = 100_000;

#[derive(Parser, Debug)]
struct Args {
  /// Liczby cząstek.
  #[arg(long, value_delimiter = ',', default_value = "9000,100000")]
  sizes: Vec<usize>,
  /// Sceny (gęstości); domyślnie `uniform,clusters,dense`, a z `--state` lub `--generate-states`
  /// samo `evolved`.
  #[arg(long, value_delimiter = ',')]
  scenes: Option<Vec<Scene>>,
  /// Kroki rozbiegu przed pomiarem sceny `evolved`.
  #[arg(long, default_value_t = 512)]
  steps: u64,
  /// Dla sceny `evolved`: ładuje stan po `--steps` krokach z `--state-dir` (lub generuje i
  /// zapisuje przy pierwszym użyciu) i przywraca go przed każdym powtórzeniem.
  #[arg(long)]
  state: bool,
  /// Z `--state`: generuje i nadpisuje zapisane stany od nowa.
  #[arg(long)]
  regenerate: bool,
  /// Tylko generuje/aktualizuje pliki stanów w `--state-dir` (implikuje `--state`), bez pomiarów
  /// i bez tworzenia pliku wyników `.parquet`.
  #[arg(long)]
  generate_states: bool,
  /// Katalog zapisanych stanów dla `--state`.
  #[arg(long, default_value = ".cache/gpu-states")]
  state_dir: PathBuf,
  /// Kroków w jednym pomiarze (czas dzielony przez tę liczbę). Powyżej `MAX_STEPS_PER_FRAME`
  /// kroki idą w kilku klatkach, a czasu GPU nie ma (pomiar obejmuje jedną klatkę).
  #[arg(long, default_value_t = 16, value_parser = clap::value_parser!(u64).range(1..))]
  batch: u64,
  /// Bez znaczników czasu GPU: krok jednym przebiegiem compute, jak w aplikacji.
  #[arg(long)]
  no_gpu_timing: bool,
  /// Wszystkie warianty `SimOptions` zamiast domyślnego wariantu aplikacji (uśrednianie sił).
  #[arg(long)]
  all_variants: bool,
  /// Ziarno macierzy sił i rozmieszczenia.
  #[arg(long, default_value_t = 0)]
  seed: u64,
  /// Próg próbkowania sąsiadów (klawisz C w aplikacji); zapisany w kolumnie `max_candidates`.
  #[arg(long, default_value_t = DEFAULT_MAX_CANDIDATES, value_parser = clap::value_parser!(u32).range(1..))]
  max_candidates: u32,
  /// Podkroki GPU na takt (`Substeps`); `step_ns` to czas taktu, czyli K podkroków. Kolumna `substeps`.
  #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=partikl::gpu::MAX_SUBSTEPS as i64))]
  substeps: u32,
  /// Liczba typów cząstek.
  #[arg(long, default_value_t = 3)]
  types: usize,
  #[command(flatten)]
  sim: SimArgs,
  /// Niemierzone pomiary przed mierzonymi.
  #[arg(long, default_value_t = 3)]
  warmup: usize,
  /// Mierzone pomiary na przypadek; domyślnie 10, z `--quick` `QUICK_MEASURED`.
  #[arg(long)]
  measured: Option<usize>,
  /// Katalog wyników.
  #[arg(long, default_value = ".artifacts/bench")]
  out_dir: PathBuf,
  /// Ustawienia porównań przed/po (`ab.sh`): `QUICK_N` cząstek, macierz `QUICK_SEED`,
  /// `QUICK_MEASURED` pomiarów. Nadpisuje `--sizes` i `--seed`.
  #[arg(long)]
  quick: bool,
  /// Przekazywane przez `cargo bench`; ignorowane.
  #[arg(long, hide = true)]
  bench: bool,
}

/// Hash FNV-1a 64-bitowy złożony do 32 bitów (8 znaków hex w nazwie pliku stanu).
fn options_hash32(options: &SimOptions, sim: &SimArgs) -> u32 {
  let mut h: u64 = 0xcbf29ce484222325;
  let feed = |h: &mut u64, bytes: &[u8]| {
    for &b in bytes {
      *h ^= b as u64;
      *h = h.wrapping_mul(0x100000001b3);
    }
  };
  feed(&mut h, format!("{options:?}").as_bytes());
  feed(&mut h, b"|");
  feed(&mut h, sim.label().as_bytes());
  ((h >> 32) ^ (h & 0xffff_ffff)) as u32
}

fn state_path(args: &Args, n: usize, seed: u64, options: &SimOptions) -> PathBuf {
  let mut name = format!(
    "n{n}-t{}-s{seed}-k{}-w{}x{}",
    args.types, args.steps, WORLD.x as u32, WORLD.y as u32
  );
  if args.max_candidates != DEFAULT_MAX_CANDIDATES {
    name.push_str(&format!("-c{}", args.max_candidates));
  }
  if args.substeps != 1 {
    name.push_str(&format!("-sub{}", args.substeps));
  }
  if *options != SimOptions::default() || !args.sim.label().is_empty() {
    name.push_str(&format!("-{:08x}", options_hash32(options, &args.sim)));
  }
  name.push_str(".bin");
  args.state_dir.join(name)
}

fn main() {
  let args = Args::parse();
  let use_state = args.state || args.generate_states;
  let scenes = args.scenes.clone().unwrap_or_else(|| {
    if use_state {
      vec![Scene::Evolved]
    } else {
      vec![Scene::Uniform, Scene::Clusters, Scene::Dense]
    }
  });
  let measured = args
    .measured
    .unwrap_or(if args.quick { QUICK_MEASURED } else { 10 });
  let seed = if args.quick { QUICK_SEED } else { args.seed };
  let sizes = if args.quick {
    vec![QUICK_N]
  } else {
    args.sizes.clone()
  };
  let variants: Vec<SimOptions> = if args.all_variants {
    RuleSource::Builtin
      .read()
      .expect("wbudowane reguły")
      .schema
      .combinations()
  } else {
    vec![args.sim.sim_options()]
  };
  if args.all_variants && !args.sim.options.is_empty() {
    panic!("--option nie łączy się z --all-variants");
  }

  if args.generate_states {
    let revision = git_revision();
    for &n in &sizes {
      for &scene in &scenes {
        if scene != Scene::Evolved {
          continue;
        }
        for options in &variants {
          let mut spec = particle_spec(args.types, seed);
          args.sim.apply_params(&mut spec);
          let meta = StateMeta {
            num_particles: n as u32,
            num_types: args.types as u32,
            seed,
            steps: args.steps,
            world: WORLD,
          };
          let path = state_path(&args, n, seed, options);
          if !args.regenerate
            && path.exists()
            && let Ok((_, rev)) = load_state(&path, &meta)
          {
            eprintln!("stan aktualny: {} ({rev})", path.display());
            continue;
          }
          let initial = Scene::Uniform.particles(n, args.types, WORLD, &mut rng(seed));
          let Some(mut harness) = Harness::new(options.clone(), spec, initial, WORLD) else {
            return;
          };
          harness.set_max_candidates(args.max_candidates);
          harness.set_substeps(args.substeps);
          harness.step(args.steps);
          let evolved = harness.run(0);
          save_state(&path, &meta, &revision, &evolved)
            .unwrap_or_else(|e| panic!("zapis stanu {}: {e}", path.display()));
          eprintln!(
            "zapisano stan: {} ({} cząstek, {} kroków, rewizja {revision})",
            path.display(),
            n,
            args.steps
          );
        }
      }
    }
    return;
  }

  let passes: Vec<&str> = partikl::gpu::timing_entries().collect();
  let timed = !args.no_gpu_timing && args.batch <= MAX_STEPS_PER_FRAME as u64;
  let schema = Arc::new(Schema::new(
    vec![
      Field::new("n", DataType::UInt32, false),
      Field::new("scene", DataType::Utf8, false),
      Field::new("variant", DataType::Utf8, false),
      Field::new("spec_seed", DataType::UInt64, false),
      Field::new("step_offset", DataType::UInt32, false),
      Field::new("candidates_mean", DataType::Float32, false),
      Field::new("max_candidates", DataType::UInt32, false),
      Field::new("substeps", DataType::UInt32, false),
      Field::new("rep", DataType::UInt16, false),
      Field::new("step_ns", DataType::UInt64, false),
      Field::new("gpu_step_ns", DataType::UInt64, true),
    ]
    .into_iter()
    .chain(
      passes
        .iter()
        .map(|entry| Field::new(format!("gpu_{entry}_ns"), DataType::UInt64, true)),
    )
    .collect::<Vec<_>>(),
  ));
  let mut output = Output::new(
    "gpu",
    &args.out_dir,
    WORLD,
    args.warmup,
    measured,
    schema.clone(),
  );
  println!(
    "{:>9} {:<9} {:<16} {:>9} {:>9} {:>9} {:>9} {:>9}",
    "n", "scene", "variant", "kandydaci", "krok µs", "GPU µs", "siatka", "siły"
  );
  'cases: for &n in &sizes {
    for &scene in &scenes {
      for options in &variants {
        let mut spec = particle_spec(args.types, seed);
        args.sim.apply_params(&mut spec);
        let (state, mut harness, reset_each_rep, step_offset, candidates) =
          if scene == Scene::Evolved && use_state {
            let meta = StateMeta {
              num_particles: n as u32,
              num_types: args.types as u32,
              seed,
              steps: args.steps,
              world: WORLD,
            };
            let path = state_path(&args, n, seed, options);
            let (state, harness) = if !args.regenerate && path.exists() {
              let (loaded, rev) =
                load_state(&path, &meta).unwrap_or_else(|e| panic!("stan {}: {e}", path.display()));
              eprintln!("stan z {} ({rev})", path.display());
              let Some(mut harness) = Harness::new(options.clone(), spec, loaded.clone(), WORLD)
              else {
                // Zapisane już wiersze trafiają do pliku (`finish` poniżej).
                break 'cases;
              };
              harness.set_max_candidates(args.max_candidates);
              harness.set_substeps(args.substeps);
              (loaded, harness)
            } else {
              let initial = Scene::Uniform.particles(n, args.types, WORLD, &mut rng(seed));
              let Some(mut harness) = Harness::new(options.clone(), spec, initial, WORLD) else {
                break 'cases;
              };
              harness.set_max_candidates(args.max_candidates);
              harness.set_substeps(args.substeps);
              harness.step(args.steps);
              let evolved = harness.run(0);
              let rev = git_revision();
              save_state(&path, &meta, &rev, &evolved)
                .unwrap_or_else(|e| panic!("zapis stanu {}: {e}", path.display()));
              eprintln!("zapisano stan: {} (rewizja {rev})", path.display());
              (evolved, harness)
            };
            let candidates = candidates_mean(&state, WORLD);
            (state, harness, true, args.steps, candidates)
          } else {
            let state = scene.particles(n, args.types, WORLD, &mut rng(seed));
            let Some(mut harness) = Harness::new(options.clone(), spec, state.clone(), WORLD)
            else {
              // Zapisane już wiersze trafiają do pliku (`finish` poniżej).
              break 'cases;
            };
            harness.set_max_candidates(args.max_candidates);
            harness.set_substeps(args.substeps);
            let (step_offset, candidates) = if scene == Scene::Evolved {
              harness.step(args.steps);
              (args.steps, candidates_mean(&harness.run(0), WORLD))
            } else {
              (0, candidates_mean(&state, WORLD))
            };
            (
              state,
              harness,
              scene != Scene::Evolved,
              step_offset,
              candidates,
            )
          };
        harness.set_gpu_timing(timed);
        let mut times = Vec::new();
        let mut gpu: Vec<Option<PassTimes>> = Vec::new();
        for _ in 0..args.warmup + measured {
          if reset_each_rep {
            harness.set_state(state.clone());
            // Pierwszy krok po `set_state` liczy histogram od nowa (recount), a znaczniki GPU są
            // tylko w pierwszym kroku klatki, więc ten krok idzie przed pomiarem; jego czas GPU
            // odbieramy i odrzucamy, żeby nie trafił do pomiaru.
            harness.step(1);
            if timed {
              harness.take_gpu_times();
            }
          }
          let start = Instant::now();
          harness.step(args.batch);
          times.push(nanos(start.elapsed()) / args.batch);
          gpu.push(if timed {
            harness.take_gpu_times()
          } else {
            None
          });
        }
        let samples = &times[args.warmup..];
        let gpu = &gpu[args.warmup..];
        // Nazwa wariantu z opcji lub domyślny opis.
        let variant = if options == &SimOptions::default() {
          "resolve+average".to_string()
        } else {
          format!("{:?}", options)
        };
        let rows = samples.len();
        let batch = RecordBatch::try_new(
          schema.clone(),
          vec![
            Arc::new(UInt32Array::from(vec![n as u32; rows])) as ArrayRef,
            Arc::new(StringArray::from(vec![scene.to_string(); rows])),
            Arc::new(StringArray::from(vec![variant.clone(); rows])),
            Arc::new(UInt64Array::from(vec![seed; rows])),
            Arc::new(UInt32Array::from(vec![step_offset as u32; rows])),
            Arc::new(Float32Array::from(vec![candidates as f32; rows])),
            Arc::new(UInt32Array::from(vec![args.max_candidates; rows])),
            Arc::new(UInt32Array::from(vec![args.substeps; rows])),
            Arc::new(UInt16Array::from((0..rows as u16).collect::<Vec<_>>())),
            Arc::new(UInt64Array::from(samples.to_vec())),
            Arc::new(UInt64Array::from(
              gpu
                .iter()
                .map(|t| t.as_ref().map(|t| t.total_ns() as u64))
                .collect::<Vec<_>>(),
            )),
          ]
          .into_iter()
          .chain(passes.iter().map(|entry| {
            Arc::new(UInt64Array::from(
              gpu
                .iter()
                .map(|t| t.as_ref().map(|t| t.get(entry) as u64))
                .collect::<Vec<_>>(),
            )) as ArrayRef
          }))
          .collect(),
        )
        .unwrap();
        output.write(&batch);
        let micros = |values: Vec<f64>| {
          if values.is_empty() {
            f64::NAN
          } else {
            median(&mut values.clone()) / 1000.0
          }
        };
        let gpu_times: Vec<&PassTimes> = gpu.iter().flatten().collect();
        let gpu_total = micros(gpu_times.iter().map(|t| t.total_ns()).collect());
        let forces = micros(gpu_times.iter().map(|t| t.get("forces")).collect());
        let grid = micros(
          gpu_times
            .iter()
            .map(|t| t.total_ns() - t.get("forces"))
            .collect(),
        );
        let step = micros(samples.iter().map(|&t| t as f64).collect());
        println!(
          "{n:>9} {scene:<9} {variant:<16} {candidates:>9.0} {step:>9.1} {gpu_total:>9.1} {grid:>9.1} {forces:>9.1}"
        );
      }
    }
  }
  println!("wyniki: {}", output.path.display());
  output.finish();
}
