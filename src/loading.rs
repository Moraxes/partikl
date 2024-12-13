use chrono::offset::Utc;

use rand::Rng;

use ron::de::from_reader;
use ron::ser::{to_writer_pretty, PrettyConfig};

use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::path::PathBuf;

use crate::args;
use crate::core;

pub fn get_particle_spec(program_args: &args::ProgramArgs) -> core::ParticleSpec {
  if let Some(path) = &program_args.interaction_spec {
    let file = File::open(path).expect(&format!("failed to open file at {:?}", path));
    let interaction_list = from_reader(file).expect(&format!("failed to parse file at {:?}", path));
    validate_interaction_list(interaction_list).expect("malformed spec")
  } else {
    use rand::SeedableRng;

    let mut rng = if let Some(seed) = program_args.interaction_seed {
      let b = seed.to_le_bytes();
      let more_bytes = [
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[0], b[1], b[2], b[3], b[4], b[5], b[6],
        b[7], b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[0], b[1], b[2], b[3], b[4], b[5],
        b[6], b[7],
      ];
      rand::rngs::SmallRng::from_seed(more_bytes)
    } else {
      rand::rngs::SmallRng::from_entropy()
    };
    let type_count = program_args.num_types;
    let particle_spec = generate_particle_spec(&mut rng, type_count);
    if !program_args.no_dump_interaction_spec {
      let path = PathBuf::from(format!("spec-{}.ron", Utc::now().format("%F-%H-%M-%S")));
      let file = File::create(&path).unwrap();
      to_writer_pretty(file, &particle_spec.interactions, PrettyConfig::default())
        .expect(&format!("failed to write to {:?}", path));
    }
    particle_spec
  }
}

fn validate_interaction_list(
  interactions: Vec<core::Interaction>,
) -> Result<core::ParticleSpec, impl Error> {
  let total_interactions = interactions.len();
  interactions
    .iter()
    .map(|interaction| validate_single_interaction(interaction, total_interactions))
    .fold(Ok(()), |acc, r| acc.and(r))
    .and_then(|_| {
      Ok(core::ParticleSpec {
        interactions,
        ..Default::default()
      })
    })
}

fn validate_single_interaction(
  interaction: &core::Interaction,
  total_interactions: usize,
) -> Result<(), impl Error> {
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

fn generate_particle_spec(rng: &mut impl Rng, type_count: usize) -> core::ParticleSpec {
  let interactions = (0..type_count)
    .map(|_| generate_single_interaction(rng, type_count))
    .fold(vec![], |mut acc, i| {
      acc.push(i);
      acc
    });
  core::ParticleSpec {
    interactions,
    ..Default::default()
  }
}

fn generate_single_interaction(rng: &mut impl Rng, type_count: usize) -> core::Interaction {
  core::Interaction {
    force_coeffs: (0..type_count)
      .map(|_| 1000.0 * rng.gen::<f32>() - 500.0)
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
