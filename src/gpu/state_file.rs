//! Zapis i odczyt binarnych migawek stanu cząstek dla powtarzalnych benchmarków (`--state`).

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use bevy::prelude::Vec2;

use super::GpuParticle;

const MAGIC: [u8; 8] = *b"PRTKST01";
const HEADER_LEN: usize = 46;

/// Metadane zapisanego stanu cząstek; wczytywanie wymaga dokładnej zgodności z oczekiwanymi.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateMeta {
  pub num_particles: u32,
  pub num_types: u32,
  pub seed: u64,
  pub steps: u64,
  pub world: Vec2,
}

/// Zapisuje stan `particles` atomowo do `path` (przez plik `.partial`).
pub fn save_state(
  path: &Path,
  meta: &StateMeta,
  revision: &str,
  particles: &[GpuParticle],
) -> Result<(), String> {
  if particles.len() != meta.num_particles as usize {
    return Err(format!(
      "liczba cząstek {} nie zgadza się z metadanymi {}",
      particles.len(),
      meta.num_particles
    ));
  }
  let rev_bytes = revision.as_bytes();
  let rev_len = u16::try_from(rev_bytes.len())
    .map_err(|_| format!("opis rewizji za długi ({} B)", rev_bytes.len()))?;

  if let Some(parent) = path.parent()
    && !parent.as_os_str().is_empty()
  {
    fs::create_dir_all(parent)
      .map_err(|e| format!("nie da się utworzyć katalogu {}: {e}", parent.display()))?;
  }

  let mut partial_os = path.as_os_str().to_os_string();
  partial_os.push(".partial");
  let partial = Path::new(&partial_os);

  let mut header = Vec::with_capacity(HEADER_LEN + rev_bytes.len());
  header.extend_from_slice(&MAGIC);
  header.extend_from_slice(&(size_of::<GpuParticle>() as u32).to_le_bytes());
  header.extend_from_slice(&meta.num_particles.to_le_bytes());
  header.extend_from_slice(&meta.num_types.to_le_bytes());
  header.extend_from_slice(&meta.seed.to_le_bytes());
  header.extend_from_slice(&meta.steps.to_le_bytes());
  header.extend_from_slice(&meta.world.x.to_le_bytes());
  header.extend_from_slice(&meta.world.y.to_le_bytes());
  header.extend_from_slice(&rev_len.to_le_bytes());
  header.extend_from_slice(rev_bytes);
  debug_assert_eq!(header.len(), HEADER_LEN + rev_bytes.len());

  let write_result = (|| -> std::io::Result<()> {
    let mut file = File::create(partial)?;
    file.write_all(&header)?;
    file.write_all(bytemuck::cast_slice(particles))?;
    file.flush()
  })();

  if let Err(e) = write_result {
    let _ = fs::remove_file(partial);
    return Err(format!("nie da się zapisać {}: {e}", partial.display()));
  }

  fs::rename(partial, path).map_err(|e| {
    let _ = fs::remove_file(partial);
    format!(
      "nie da się przenieść {} -> {}: {e}",
      partial.display(),
      path.display()
    )
  })
}

/// Wczytuje stan z `path` i sprawdza nagłówek oraz zgodność z `expected`.
/// Zwraca `(cząstki, rewizja)`.
pub fn load_state(path: &Path, expected: &StateMeta) -> Result<(Vec<GpuParticle>, String), String> {
  let bytes = fs::read(path).map_err(|e| format!("nie da się odczytać {}: {e}", path.display()))?;
  if bytes.len() < HEADER_LEN {
    return Err(format!(
      "plik {} za krótki na nagłówek ({} B < {HEADER_LEN} B)",
      path.display(),
      bytes.len()
    ));
  }
  if bytes[..8] != MAGIC {
    return Err(format!("nieznany nagłówek w {}", path.display()));
  }

  let read_u16 = |off: usize| u16::from_le_bytes(bytes[off..off + 2].try_into().unwrap());
  let read_u32 = |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
  let read_u64 = |off: usize| u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
  let read_f32 = |off: usize| f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());

  let particle_size = read_u32(8);
  let expected_particle_size = size_of::<GpuParticle>() as u32;
  if particle_size != expected_particle_size {
    return Err(format!(
      "niezgodny rozmiar GpuParticle w {}: jest {particle_size}, oczekiwano {expected_particle_size}",
      path.display()
    ));
  }

  let meta = StateMeta {
    num_particles: read_u32(12),
    num_types: read_u32(16),
    seed: read_u64(20),
    steps: read_u64(28),
    world: Vec2::new(read_f32(36), read_f32(40)),
  };
  let rev_len = read_u16(44) as usize;
  let rev_end = HEADER_LEN + rev_len;
  if bytes.len() < rev_end {
    return Err(format!(
      "ucięta rewizja w {}: {} B < {rev_end} B",
      path.display(),
      bytes.len()
    ));
  }
  let revision = std::str::from_utf8(&bytes[HEADER_LEN..rev_end])
    .map_err(|e| format!("błędne UTF-8 rewizji w {}: {e}", path.display()))?
    .to_string();

  if meta.num_particles != expected.num_particles
    || meta.num_types != expected.num_types
    || meta.seed != expected.seed
    || meta.steps != expected.steps
    || meta.world.x.to_bits() != expected.world.x.to_bits()
    || meta.world.y.to_bits() != expected.world.y.to_bits()
  {
    return Err(format!(
      "niezgodne metadane w {}: zapisano {meta:?}, oczekiwano {expected:?}",
      path.display()
    ));
  }

  let payload = &bytes[rev_end..];
  let expected_payload = (expected.num_particles as usize)
    .checked_mul(size_of::<GpuParticle>())
    .ok_or_else(|| "przepełnienie rozmiaru payloadu".to_string())?;
  if payload.len() != expected_payload {
    return Err(format!(
      "niezgodny rozmiar danych w {}: jest {} B, oczekiwano {expected_payload} B",
      path.display(),
      payload.len()
    ));
  }

  let (chunks, _) = payload.as_chunks::<{ size_of::<GpuParticle>() }>();
  let particles = chunks
    .iter()
    .map(|chunk| bytemuck::pod_read_unaligned(chunk))
    .collect();
  Ok((particles, revision))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicU64, Ordering};

  static COUNTER: AtomicU64 = AtomicU64::new(0);

  fn temp_path(name: &str) -> PathBuf {
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
      "partikl-state-{}-{}-{name}.bin",
      std::process::id(),
      seq
    ))
  }

  fn sample_meta() -> StateMeta {
    StateMeta {
      num_particles: 3,
      num_types: 4,
      seed: 23,
      steps: 512,
      world: Vec2::new(2560.0, 1440.0),
    }
  }

  fn sample_particles() -> Vec<GpuParticle> {
    vec![
      GpuParticle {
        pos: [-123.45, 678.9],
        vel: [12.5, -0.0],
        species: 2,
        id: 17,
      },
      GpuParticle {
        pos: [0.000125, -999.75],
        vel: [-42.25, 31.125],
        species: 0,
        id: 3,
      },
      GpuParticle {
        pos: [512.0, 0.0],
        vel: [0.0, 0.0],
        species: 3,
        id: 99,
      },
    ]
  }

  #[test]
  fn round_trip_preserves_particles_bit_for_bit() {
    let path = temp_path("round-trip");
    let meta = sample_meta();
    let particles = sample_particles();
    save_state(&path, &meta, "970d6ce-dirty", &particles).expect("save_state");

    let mut partial_os = path.as_os_str().to_os_string();
    partial_os.push(".partial");
    assert!(
      !Path::new(&partial_os).exists(),
      "plik tymczasowy .partial powinien zniknąć po rename"
    );

    let (loaded, revision) = load_state(&path, &meta).expect("load_state");
    assert_eq!(revision, "970d6ce-dirty");
    assert_eq!(loaded.len(), particles.len());
    assert_eq!(
      bytemuck::cast_slice::<GpuParticle, u8>(&loaded),
      bytemuck::cast_slice::<GpuParticle, u8>(&particles)
    );

    let _ = fs::remove_file(&path);
  }

  #[test]
  fn rejects_bad_magic_and_truncated_file() {
    let path = temp_path("corrupt");
    let meta = sample_meta();
    let particles = sample_particles();
    save_state(&path, &meta, "abc1234", &particles).expect("save_state");
    let valid = fs::read(&path).expect("read valid file");

    let mut bad_magic = valid.clone();
    bad_magic[0] = b'X';
    fs::write(&path, &bad_magic).unwrap();
    assert!(load_state(&path, &meta).is_err());

    fs::write(&path, &valid[..10]).unwrap();
    assert!(load_state(&path, &meta).is_err());

    fs::write(&path, &valid[..valid.len() - 1]).unwrap();
    assert!(load_state(&path, &meta).is_err());

    let _ = fs::remove_file(&path);
  }

  #[test]
  fn rejects_mismatched_metadata() {
    let path = temp_path("mismatch");
    let meta = sample_meta();
    let particles = sample_particles();
    save_state(&path, &meta, "abc1234", &particles).expect("save_state");

    let wrong_n = StateMeta {
      num_particles: 4,
      ..meta
    };
    assert!(load_state(&path, &wrong_n).is_err());

    let wrong_seed = StateMeta { seed: 99, ..meta };
    assert!(load_state(&path, &wrong_seed).is_err());

    let wrong_steps = StateMeta { steps: 256, ..meta };
    assert!(load_state(&path, &wrong_steps).is_err());

    let wrong_world = StateMeta {
      world: Vec2::new(1920.0, 1080.0),
      ..meta
    };
    assert!(load_state(&path, &wrong_world).is_err());

    let _ = fs::remove_file(&path);
  }
}
