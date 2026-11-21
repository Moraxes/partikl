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
  /// Format głównej tekstury kamery: true = HDR (Rgba16Float, tonemapping), false = LDR
  /// (domyślny w aplikacji).
  #[arg(long, value_delimiter = ',', default_value = "false")]
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
  /// Zamiast pomiaru: galeria porównań wyglądu w katalogu (PNG): ten sam stan (po
  /// `--gallery-steps` krokach rozbiegu, `--sizes`, `--seed`) w LDR i HDR oraz bez i z
  /// analitycznym AA, z kolorami jak w aplikacji, panele obok siebie (lewy = domyślny wygląd
  /// aplikacji), w skali 1:1 i 4×.
  #[arg(long)]
  gallery: Option<PathBuf>,
  /// Tryby pomiaru: draw (sam render) i draw+step (klatka z krokiem).
  #[arg(long, value_delimiter = ',', default_value = "draw,draw+step")]
  modes: Vec<String>,
  /// Kroki symulacji przed pomiarem (stan w kolejności siatki).
  #[arg(long, default_value_t = 5)]
  steps: u64,
  /// Kroki rozbiegu stanu w trybie `--gallery`.
  #[arg(long, default_value_t = 800)]
  gallery_steps: u64,
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

/// Wycinek `w`×`h` od `(x0, y0)` powiększony `scale` razy (najbliższy sąsiad), RGBA8.
fn panel(pixels: &[u8], x0: u32, y0: u32, w: u32, h: u32, scale: u32) -> Vec<u8> {
  let mut out = Vec::with_capacity((w * h * scale * scale * 4) as usize);
  for y in 0..h * scale {
    for x in 0..w * scale {
      let i = (((y0 + y / scale) * TARGET.x + x0 + x / scale) * 4) as usize;
      out.extend_from_slice(&pixels[i..i + 4]);
    }
  }
  out
}

/// Dwa panele `w`×`h` obok siebie, rozdzielone szarym pasem.
fn side_by_side(left: &[u8], right: &[u8], w: u32, h: u32) -> (Vec<u8>, u32) {
  const GAP: u32 = 16;
  let mut out = Vec::new();
  for y in 0..h as usize {
    let row = y * w as usize * 4;
    out.extend_from_slice(&left[row..row + w as usize * 4]);
    out.extend((0..GAP).flat_map(|_| [128u8, 128, 128, 255]));
    out.extend_from_slice(&right[row..row + w as usize * 4]);
  }
  (out, 2 * w + GAP)
}

fn save_png(path: &std::path::Path, width: u32, height: u32, rgba: Vec<u8>) {
  use bevy::asset::RenderAssetUsages;
  use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
  let image = Image::new(
    Extent3d { width, height, depth_or_array_layers: 1 },
    TextureDimension::D2,
    rgba,
    TextureFormat::Rgba8UnormSrgb,
    RenderAssetUsages::default(),
  );
  image.try_into_dynamic().expect("PNG").save(path).expect("cannot save image");
}

/// Galeria porównań wyglądu: jeden stan (po rozbiegu), rysowany w parach wariantów.
fn gallery(args: &Args, dir: &std::path::Path) {
  std::fs::create_dir_all(dir).expect("cannot create gallery directory");
  let options = SimOptions { average_forces: true, overlap: Overlap::default() };
  let n = args.sizes[0];
  let spec = || particle_spec(args.types, args.seed);
  let initial = Scene::Uniform.particles(n, args.types, WORLD, &mut rng(args.seed));
  // Rozbieg: stan z prawdziwej dynamiki (struktury), potem ten sam stan dla każdego wariantu.
  let Some(mut harness) = Harness::new(options, spec(), initial, WORLD) else { return };
  let state = harness.run(args.gallery_steps);
  drop(harness);
  let render = |quality: RenderQuality| -> Vec<u8> {
    let offscreen = Offscreen { size: TARGET, quality, app_colors: true };
    let mut harness = Harness::with_render(options, spec(), state.clone(), WORLD, Some(offscreen)).expect("GPU");
    for _ in 0..6 {
      harness.update();
    }
    harness.target_pixels()
  };
  let base = RenderQuality::default();
  let pairs = [
    ("ldr_vs_hdr", base, RenderQuality { hdr: true, ..base }),
    ("noaa_vs_aa", base, RenderQuality { analytic_aa: true, ..base }),
  ];
  // Środek obrazu: 900×700 pikseli fizycznych w skali 1:1, 225×175 w skali 4×.
  let (cx, cy) = (TARGET.x / 2, TARGET.y / 2);
  for (name, left, right) in pairs {
    let (a, b) = (render(left), render(right));
    for (scale, w, h) in [(1u32, 900u32, 700u32), (4, 225, 175)] {
      let (x0, y0) = (cx - w / 2, cy - h / 2);
      let (pa, pb) = (panel(&a, x0, y0, w, h, scale), panel(&b, x0, y0, w, h, scale));
      let (image, width) = side_by_side(&pa, &pb, w * scale, h * scale);
      let path = dir.join(format!("{name}_{scale}x.png"));
      save_png(&path, width, h * scale, image);
      println!("{}", path.display());
    }
  }
}

fn main() {
  let args = Args::parse();
  if let Some(dir) = &args.gallery {
    gallery(&args, dir);
    return;
  }
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
          let offscreen = Offscreen { size: TARGET, quality, app_colors: false };
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
