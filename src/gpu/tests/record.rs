//! Nagrania klipów z symulacji bez okna: obraz z kamery offscreen (kolory jak w aplikacji) klatka
//! po klatce do `ffmpeg` (H.264). Czas symulacji jak w aplikacji: `DELTA_TIME` na krok, więc
//! przy 60 kl./s klatka to średnio 1⅔ kroku.
//!
//! Test jest `#[ignore]`, sterowany zmiennymi środowiskowymi:
//! - `REC_OUT` katalog wyniku (wymagany), `REC_N` liczba cząstek (25 000), `REC_SECONDS` długość
//!   klipu (5), `REC_TYPES` liczba typów (3), `REC_SEEDS` ziarna macierzy po przecinku (1,2,3,4),
//!   `REC_OPTIONS` wybory opcji reguł `nazwa=wybór,…`.
//!
//! `REC_OUT=/tmp/clips gate-run --resource cargo --resource gpu -- cargo test --quiet record_clips -- --ignored --nocapture`

use super::*;
use crate::gpu::headless::Offscreen;
use std::io::Write as _;
use std::process::{Command, Stdio};

const SIZE: UVec2 = UVec2::new(1920, 1080);
const FPS: u64 = 60;

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
  std::env::var(name).map_or(default, |s| {
    s.parse()
      .unwrap_or_else(|_| panic!("{name}: zła wartość `{s}`"))
  })
}

/// Jeden klip: `seconds` sekund od stanu początkowego do `path` (mp4).
fn record_clip(
  options: SimOptions,
  spec: ParticleSpec,
  particles: Vec<GpuParticle>,
  world: Vec2,
  seconds: u64,
  path: &std::path::Path,
) -> bool {
  let offscreen = Offscreen {
    size: SIZE,
    quality: RenderQuality::default(),
    app_colors: true,
    real_time: false,
  };
  let Some(mut harness) = Harness::with_render(options, spec, particles, world, Some(offscreen))
  else {
    eprintln!("brak GPU, pomijam");
    return false;
  };
  // Zasoby rysowania trafiają na GPU z opóźnieniem kilku klatek.
  for _ in 0..6 {
    harness.update();
  }
  let mut ffmpeg = Command::new("ffmpeg")
    .args([
      "-y",
      "-loglevel",
      "error",
      "-f",
      "rawvideo",
      "-pix_fmt",
      "rgba",
      "-s",
    ])
    .arg(format!("{}x{}", SIZE.x, SIZE.y))
    .args(["-r", &FPS.to_string(), "-i", "-"])
    .args([
      "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p",
    ])
    .arg(path)
    .stdin(Stdio::piped())
    .spawn()
    .expect("ffmpeg");
  let mut stdin = ffmpeg.stdin.take().unwrap();
  let steps_per_second = (1.0 / DELTA_TIME).round() as u64;
  let mut done = 0;
  for frame in 0..seconds * FPS {
    let target = frame * steps_per_second / FPS;
    harness.step(target - done);
    done = target;
    stdin.write_all(&harness.target_pixels()).unwrap();
  }
  drop(stdin);
  assert!(ffmpeg.wait().unwrap().success(), "ffmpeg: błąd kodowania");
  true
}

#[test]
#[ignore = "nagrania: uruchamiane ręcznie, wymaga ffmpeg"]
fn record_clips() {
  let out = std::path::PathBuf::from(std::env::var("REC_OUT").expect("REC_OUT: katalog wyniku"));
  std::fs::create_dir_all(&out).unwrap();
  let n: usize = env_or("REC_N", 25_000);
  let seconds: u64 = env_or("REC_SECONDS", 5);
  let types: usize = env_or("REC_TYPES", 3);
  let seeds: Vec<u64> = std::env::var("REC_SEEDS")
    .unwrap_or_else(|_| "1,2,3,4".into())
    .split(',')
    .map(|s| s.trim().parse().expect("REC_SEEDS"))
    .collect();
  let mut options = SimOptions::default();
  for pair in std::env::var("REC_OPTIONS")
    .unwrap_or_default()
    .split(',')
    .filter(|s| !s.is_empty())
  {
    let (name, choice) = pair.split_once('=').expect("REC_OPTIONS: nazwa=wybór");
    options = options.with(name, choice);
  }
  // Świat jak ekran 1080p (świat symulacji = rozmiar ekranu w pikselach).
  let world = SIZE.as_vec2();
  for seed in seeds {
    let mut rng = StdRng::seed_from_u64(seed);
    let spec = crate::loading::generate_particle_spec(&mut rng, types);
    let particles = initial_particles(n, types, world, &mut rng);
    let path = out.join(format!("seed{seed}.mp4"));
    let started = std::time::Instant::now();
    if !record_clip(options.clone(), spec, particles, world, seconds, &path) {
      return;
    }
    eprintln!(
      "{} ({:.0} s)",
      path.display(),
      started.elapsed().as_secs_f64()
    );
  }
}
