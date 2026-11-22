use bevy::prelude::Resource;

use crate::gpu::{DEFAULT_MAX_CANDIDATES, RuleSource};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Resource)]
#[command(version, name = "partikl")]
pub struct ProgramArgs {
  /// Exit after this many seconds.
  ///
  /// This is useful for profiling/benchmarking.
  #[arg(long)]
  pub exit_after: Option<f64>,

  /// Seed used for generating the interaction spec.
  ///
  /// By default, the interaction specification is saved to a timestamped file
  /// after being generated (i.e. on every startup), to avoid interesting
  /// simulations being lost. If you wish to prevent this behaviour, use
  /// --no-dump-interaction-spec.
  ///
  /// Note that it's usually more convenient to use the <interaction-spec> arg.
  #[arg(long)]
  pub interaction_seed: Option<u64>,

  /// Path to the interaction specification file.
  ///
  /// This file contains the force coefficient matrix and other parameters of
  /// the simulation that are considered "interesting", meaning they influence
  /// large-scale patterns that emerge during the simulation. Particle color,
  /// arrangement, initial velocity, and dish size are not configurable yet.
  ///
  /// If present, --no-dump-interaction-spec is assumed, and --num-types is
  /// ignored.
  #[arg()]
  pub interaction_spec: Option<PathBuf>,

  /// Replace the built-in rules (forces, integration) with a WESL module read from this file.
  ///
  /// The file follows the contract described at the top of
  /// `src/gpu/shaders/rules/default_rules.wesl`, which is a good starting point. It is watched:
  /// saving it recompiles the rules while the simulation runs.
  #[arg(long, value_name = "PATH")]
  pub rules: Option<PathBuf>,

  /// Number of particle types, used when generating the interaction spec.
  #[arg(short = 't', long, default_value_t = 3)]
  pub num_types: usize,

  /// Number of particles to be generated initially.
  #[arg(short = 'n', long, default_value_t = 100000)]
  pub num_particles: usize,

  /// Prevents the interaction spec from being dumped.
  #[arg(long)]
  pub no_dump_interaction_spec: bool,

  /// Radius of each particle.
  #[arg(short = 's', long, default_value_t = 2.0)]
  pub particle_size: f32,

  /// Smooth the particle edges in the fragment shader (costs about 1 ms per frame at 500k
  /// particles; the edges are otherwise sharp). Toggled at runtime with A.
  #[arg(long)]
  pub antialias: bool,

  /// Start in HDR (tonemapped colours, subdued; better on HDR/OLED screens). Toggled at runtime
  /// with B.
  #[arg(long)]
  pub hdr: bool,

  /// Sort particles within grid cells so runs are bit-for-bit reproducible (slower).
  #[arg(long)]
  pub gpu_deterministic: bool,

  /// Set a rules option at startup, as NAME=CHOICE (repeatable).
  ///
  /// The rules declare their options (see `//@option` in
  /// `src/gpu/shaders/rules/default_rules.wesl`); the built-in rules have `average_forces`
  /// (on, off) and `overlap` (spring, resolve, adaptive_range). Options toggle at runtime with
  /// the key the rules declare (N and O for the built-in ones). Unset options take the first
  /// choice.
  #[arg(long = "option", value_name = "NAME=CHOICE")]
  pub options: Vec<String>,

  /// Neighbour sampling cap: above this many candidates per particle, the engine samples them
  /// (each sampled one weighted by the stride). Cycles through 128, 256, 512, 1024 with C.
  #[arg(long, value_name = "N", default_value_t = DEFAULT_MAX_CANDIDATES, value_parser = clap::value_parser!(u32).range(1..))]
  pub max_candidates: u32,
}

impl ProgramArgs {
  /// Źródło modułu reguł.
  pub fn rule_source(&self) -> RuleSource {
    self.rules.clone().map_or(RuleSource::Builtin, RuleSource::File)
  }
}

#[cfg(test)]
mod tests;
