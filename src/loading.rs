use chrono::offset::Utc;

use rand::{Rng, RngExt};

use ron::options::Options;
use ron::ser::PrettyConfig;

use serde::Deserialize;

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::args;
use crate::spec::{Interaction, ParticleSpec};

/// Błąd wczytania albo zapisu specyfikacji; komunikat zawiera ścieżkę pliku.
#[derive(Debug)]
pub enum LoadError {
  Io { path: PathBuf, source: std::io::Error },
  Parse { path: PathBuf, source: ron::error::SpannedError },
  Write { path: PathBuf, source: ron::Error },
  Malformed { path: PathBuf, reason: String },
}

impl Display for LoadError {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    match self {
      LoadError::Io { path, source } => write!(f, "{}: {source}", path.display()),
      LoadError::Parse { path, source } => write!(f, "{}: nie da się wczytać specyfikacji: {source}", path.display()),
      LoadError::Write { path, source } => write!(f, "{}: nie da się zapisać specyfikacji: {source}", path.display()),
      LoadError::Malformed { path, reason } => write!(f, "{}: błędna specyfikacja: {reason}", path.display()),
    }
  }
}

impl Error for LoadError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      LoadError::Io { source, .. } => Some(source),
      LoadError::Parse { source, .. } => Some(source),
      LoadError::Write { source, .. } => Some(source),
      LoadError::Malformed { .. } => None,
    }
  }
}

/// Zawartość pliku `.ron`: pełna specyfikacja `(interactions: [...], params: (...))` albo, jak w
/// starszych plikach, sama lista wierszy macierzy sił.
#[derive(Deserialize)]
#[serde(untagged)]
enum SpecFile {
  Full(ParticleSpec),
  Matrix(Vec<Interaction>),
}

pub fn get_particle_spec(program_args: &args::ProgramArgs) -> Result<ParticleSpec, LoadError> {
  if let Some(path) = &program_args.interaction_spec {
    load_spec(path)
  } else {
    use rand::SeedableRng;

    let mut rng = if let Some(seed) = program_args.interaction_seed {
      // Ziarno u64 powtórzone na całą długość seeda (zachowuje dotychczasowe wyniki).
      let mut full_seed = [0u8; 32];
      full_seed
        .chunks_exact_mut(8)
        .for_each(|chunk| chunk.copy_from_slice(&seed.to_le_bytes()));
      rand::rngs::SmallRng::from_seed(full_seed)
    } else {
      rand::rngs::SmallRng::from_rng(&mut rand::rng())
    };
    let type_count = program_args.num_types;
    let particle_spec = generate_particle_spec(&mut rng, type_count);
    if !program_args.no_dump_interaction_spec {
      let path = PathBuf::from(format!("spec-{}.ron", Utc::now().format("%F-%H-%M-%S")));
      dump_spec(&path, &particle_spec)?;
    }
    Ok(particle_spec)
  }
}

/// Wczytuje i sprawdza specyfikację z pliku `.ron`.
pub fn load_spec(path: &Path) -> Result<ParticleSpec, LoadError> {
  let file = File::open(path).map_err(|source| LoadError::Io { path: path.into(), source })?;
  let parsed: SpecFile = Options::default()
    .from_reader(file)
    .map_err(|source| LoadError::Parse { path: path.into(), source })?;
  let spec = match parsed {
    SpecFile::Full(spec) => spec,
    SpecFile::Matrix(interactions) => ParticleSpec::new(interactions),
  };
  validate_spec(&spec).map_err(|reason| LoadError::Malformed { path: path.into(), reason })?;
  Ok(spec)
}

/// Zapisuje specyfikację (macierz i parametry reguł) do pliku `.ron`.
pub fn dump_spec(path: &Path, spec: &ParticleSpec) -> Result<(), LoadError> {
  let file = File::create(path).map_err(|source| LoadError::Io { path: path.into(), source })?;
  Options::default()
    .to_io_writer_pretty(file, spec, PrettyConfig::default())
    .map_err(|source| LoadError::Write { path: path.into(), source })
}

fn validate_spec(spec: &ParticleSpec) -> Result<(), String> {
  if spec.interactions.is_empty() {
    return Err("brak typów cząstek".into());
  }
  validate_interaction_list(&spec.interactions).map_err(|e| e.to_string())?;
  spec.params.validate()
}

fn validate_interaction_list(interactions: &[Interaction]) -> Result<(), MalformedInteractionError> {
  let total_interactions = interactions.len();
  interactions
    .iter()
    .try_for_each(|interaction| validate_single_interaction(interaction, total_interactions))
}

fn validate_single_interaction(
  interaction: &Interaction,
  total_interactions: usize,
) -> Result<(), MalformedInteractionError> {
  let total_coeffs = interaction.force_coeffs.len();
  if total_coeffs == total_interactions {
    Ok(())
  } else {
    Err(MalformedInteractionError {
      total_interactions,
      total_coeffs,
    })
  }
}

fn generate_particle_spec(rng: &mut impl Rng, type_count: usize) -> ParticleSpec {
  let interactions = (0..type_count)
    .map(|_| generate_single_interaction(rng, type_count))
    .collect();
  ParticleSpec::new(interactions)
}

fn generate_single_interaction(rng: &mut impl Rng, type_count: usize) -> Interaction {
  Interaction {
    force_coeffs: (0..type_count)
      .map(|_| 1000.0 * rng.random::<f32>() - 500.0)
      .collect(),
  }
}

#[derive(Debug)]
struct MalformedInteractionError {
  total_interactions: usize,
  total_coeffs: usize,
}

impl Display for MalformedInteractionError {
  fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "expected {} coefficients, got {}",
      self.total_interactions, self.total_coeffs
    ))
  }
}

impl Error for MalformedInteractionError {}

#[cfg(test)]
mod tests;
