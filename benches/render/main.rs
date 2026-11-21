//! Koszt rysowania cząstek bez okna: kamera renderuje do obrazu o rozmiarze ekranu (`TARGET`,
//! świat `WORLD` na całym obrazie, więc cząstka ma ten sam rozmiar w pikselach co w aplikacji).
//! Bez okna nie ma vsync, fokusu ani kompozytora; pomiar to czas klatki (aktualizacja, render
//! i czekanie na GPU) w trybach:
//! - `draw`: sam render, bez kroków; stan po kilku krokach `--steps` (w kolejności siatki),
//! - `draw+step`: klatka z jednym krokiem symulacji (jak aplikacja przy 100 klatkach na sekundę).
//!
//! Warianty: MSAA (`--msaa`), format HDR kamery (`--hdr`). Wyniki w Parquet (`frame_ns`),
//! porównanie przed/po: `AB_BENCH=render tools/bench-ab/ab.sh`. Rozmiar ekranu i świata to
//! stałe (laptop 3600×2338 przy skali 2), żeby wyniki były porównywalne między przebiegami.
//! `cargo bench --bench render -- --help` pokazuje parametry.

#[path = "../common/mod.rs"]
mod common;
#[path = "../gpu/scenes.rs"]
mod scenes;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use arrow_array::{ArrayRef, Float32Array, RecordBatch, StringArray, UInt16Array, UInt32Array, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use bevy::prelude::*;
use clap::Parser;

use common::{Output, median, nanos, particle_spec, rng};
use partikl::gpu::RenderQuality;
use partikl::gpu::headless::{Harness, Offscreen};
use partikl::options::{Overlap, SimOptions};
use scenes::{Scene, candidates_mean};

/// Świat jak na ekranie 3600×2338 przy skali 2 (jednostki logiczne).
const WORLD: Vec2 = Vec2::new(1800.0, 1169.0);
/// Rozmiar obrazu w pikselach fizycznych.
const TARGET: UVec2 = UVec2::new(3600, 2338);
const QUICK_MEASURED: usize = 100;

#[derive(Parser, Debug)]
struct Args {
  /// Liczby cząstek.
  #[arg(long, value_delimiter = ',', default_value = "500000")]
  sizes: Vec<usize>,
  /// Sceny (gęstości).
  #[arg(long, value_delimiter = ',', default_value = "uniform")]
  scenes: Vec<Scene>,
  /// Próbki MSAA kamery: 1 (wyłączone, domyślne w aplikacji), 2 albo 4.
  #[arg(long, value_delimiter = ',', default_value = "1")]
  msaa: Vec<u8>,
  /// Format głównej tekstury kamery: true = HDR (Rgba16Float), false = LDR.
  #[arg(long, value_delimiter = ',', default_value = "true")]
  hdr: Vec<bool>,
  /// Tonemapping i dithering kamery (domyślne ustawienie Bevy); false = surowe kolory.
  #[arg(long, value_delimiter = ',', default_value = "true")]
  tonemap: Vec<bool>,
  /// Analityczne wygładzanie krawędzi w fragment shaderze (alpha blending), z MSAA 1.
  #[arg(long, value_delimiter = ',', default_value = "false")]
  aa: Vec<bool>,
  /// Katalog na zrzuty (PNG, pełny obraz) pierwszej klatki trybu `draw` każdego wariantu.
  #[arg(long)]
  screenshots: Option<PathBuf>,
  /// Tryby pomiaru: draw (sam render) i draw+step (klatka z krokiem).
  #[arg(long, value_delimiter = ',', default_value = "draw,draw+step")]
  modes: Vec<String>,
  /// Kroki symulacji przed pomiarem (stan w kolejności siatki).
  #[arg(long, default_value_t = 5)]
  steps: u64,
  /// Ziarno macierzy sił i rozmieszczenia.
  #[arg(long, default_value_t = 0)]
  seed: u64,
  /// Liczba typów cząstek.
  #[arg(long, default_value_t = 3)]
  types: usize,
  /// Niemierzone klatki przed mierzonymi.
  #[arg(long, default_value_t = 10)]
  warmup: usize,
  /// Mierzone klatki na przypadek; domyślnie 60, z `--quick` `QUICK_MEASURED`.
  #[arg(long)]
  measured: Option<usize>,
  /// Katalog wyników.
  #[arg(long, default_value = ".artifacts/bench")]
  out_dir: PathBuf,
  /// Ustawienia porównań przed/po (`ab.sh`): `QUICK_MEASURED` klatek.
  #[arg(long)]
  quick: bool,
  /// Przekazywane przez `cargo bench`; ignorowane.
  #[arg(long, hide = true)]
  bench: bool,
}

fn msaa(samples: u8) -> Msaa {
  match samples {
    1 => Msaa::Off,
    2 => Msaa::Sample2,
    4 => Msaa::Sample4,
    other => panic!("MSAA {other}: obsługiwane 1, 2, 4"),
  }
}

/// Iloczyn trzech list (wszystkie kombinacje).
fn iproduct<'a, A, B, C>(a: &'a [A], b: &'a [B], c: &'a [C]) -> impl Iterator<Item = (&'a A, &'a B, &'a C)> {
  a.iter().flat_map(move |x| b.iter().flat_map(move |y| c.iter().map(move |z| (x, y, z))))
}

/// Zapisuje ostatnią zrenderowaną klatkę jako PNG.
fn save_screenshot(harness: &mut Harness, dir: &std::path::Path, name: &str) {
  use bevy::asset::RenderAssetUsages;
  use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
  std::fs::create_dir_all(dir).expect("cannot create screenshot directory");
  let pixels = harness.target_pixels();
  let image = Image::new(
    Extent3d { width: TARGET.x, height: TARGET.y, depth_or_array_layers: 1 },
    TextureDimension::D2,
    pixels,
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::default(),
  );
  image.try_into_dynamic().expect("PNG").save(dir.join(name)).expect("cannot save screenshot");
}

fn main() {
  let args = Args::parse();
  let measured = args.measured.unwrap_or(if args.quick { QUICK_MEASURED } else { 60 });
  let schema = Arc::new(Schema::new(vec![
    Field::new("n", DataType::UInt32, false),
    Field::new("scene", DataType::Utf8, false),
    Field::new("variant", DataType::Utf8, false),
    Field::new("spec_seed", DataType::UInt64, false),
    Field::new("step_offset", DataType::UInt32, false),
    Field::new("candidates_mean", DataType::Float32, false),
    Field::new("rep", DataType::UInt16, false),
    Field::new("frame_ns", DataType::UInt64, false),
  ]));
  let mut output = Output::new("render", &args.out_dir, WORLD, args.warmup, measured, schema.clone());
  let options = SimOptions { average_forces: true, overlap: Overlap::default() };
  println!("{:>9} {:<9} {:<26} {:>9} {:>10}", "n", "scene", "variant", "kandydaci", "klatka ms");
  'cases: for &n in &args.sizes {
    for &scene in &args.scenes {
      for &samples in &args.msaa {
        for (&hdr, &tonemapping, &analytic_aa) in iproduct(&args.hdr, &args.tonemap, &args.aa) {
          let state = scene.particles(n, args.types, WORLD, &mut rng(args.seed));
          let candidates = candidates_mean(&state, WORLD);
          let spec = particle_spec(args.types, args.seed);
          let quality = RenderQuality { msaa: msaa(samples), hdr, tonemapping, analytic_aa };
          let offscreen = Offscreen { size: TARGET, quality };
          let Some(mut harness) = Harness::with_render(options, spec, state, WORLD, Some(offscreen)) else {
            break 'cases;
          };
          harness.step(args.steps);
          for mode in &args.modes {
            let mut frame = || {
              let start = Instant::now();
              match mode.as_str() {
                "draw" => harness.update(),
                "draw+step" => harness.step(1),
                other => panic!("tryb {other}: draw albo draw+step"),
              }
              nanos(start.elapsed())
            };
            let times: Vec<u64> = (0..args.warmup + measured).map(|_| frame()).collect();
            let samples_ns = &times[args.warmup..];
            let variant = format!(
              "msaa{samples} {} {} {} {mode}",
              if hdr { "hdr" } else { "ldr" },
              if tonemapping { "tm" } else { "raw" },
              if analytic_aa { "aa" } else { "noaa" },
            );
            if let (Some(dir), "draw") = (&args.screenshots, mode.as_str()) {
              save_screenshot(&mut harness, dir, &format!("{n}-{scene}-{}.png", variant.replace(' ', "_")));
            }
            let rows = samples_ns.len();
            let batch = RecordBatch::try_new(
              schema.clone(),
              vec![
                Arc::new(UInt32Array::from(vec![n as u32; rows])) as ArrayRef,
                Arc::new(StringArray::from(vec![scene.to_string(); rows])),
                Arc::new(StringArray::from(vec![variant.clone(); rows])),
                Arc::new(UInt64Array::from(vec![args.seed; rows])),
                Arc::new(UInt32Array::from(vec![args.steps as u32; rows])),
                Arc::new(Float32Array::from(vec![candidates as f32; rows])),
                Arc::new(UInt16Array::from((0..rows as u16).collect::<Vec<_>>())),
                Arc::new(UInt64Array::from(samples_ns.to_vec())),
              ],
            )
            .unwrap();
            output.write(&batch);
            let mut floats: Vec<f64> = samples_ns.iter().map(|&t| t as f64).collect();
            println!("{n:>9} {scene:<9} {variant:<26} {candidates:>9.0} {:>10.2}", median(&mut floats) / 1e6);
          }
        }
      }
    }
  }
  println!("wyniki: {}", output.path.display());
  output.finish();
}
