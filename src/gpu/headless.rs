//! Aplikacja bez okna z symulacją GPU i własnym kołem aktualizacji: testy i benchmarki.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bevy::app::SubApps;
use bevy::render::RenderPlugin;
use bevy::render::gpu_readback::{ReadbackComplete, ReadbackOnce};
use bevy::render::render_resource::PollType;
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use super::material::ParticleMaterial;
use super::*;
use crate::sim::SimOptions;

/// Jedna aplikacja GPU naraz w procesie (wspólne pule wątków i dziennik).
static LOCK: Mutex<()> = Mutex::new(());

/// Aplikacja bez okna, z własnym kołem aktualizacji; `None` bez adaptera GPU.
pub struct Harness {
  apps: SubApps,
  _guard: MutexGuard<'static, ()>,
}

impl Harness {
  pub fn new(options: SimOptions, spec: ParticleSpec, particles: Vec<GpuParticle>, world: Vec2) -> Option<Harness> {
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      let mut app = App::new();
      app.add_plugins(
        DefaultPlugins
          .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, ..default() })
          .set(RenderPlugin { synchronous_pipeline_compilation: true, ..default() })
          .disable::<WinitPlugin>(),
      );
      app.insert_resource(options).insert_resource(spec).add_plugins(GpuPlugin);
      // Kroki liczy test, nie czas stały.
      app.insert_resource(Time::<Fixed>::from_seconds(1.0e9));
      app.insert_resource(Initial(Some(particles), world));
      app.insert_resource(Request(None));
      app.add_systems(Startup, spawn_test_sim).add_systems(Update, request_readback.run_if(resource_exists::<GpuSim>));
      app.finish();
      app.cleanup();
      Harness { apps: std::mem::take(app.sub_apps_mut()), _guard: guard }
    }));
    match result {
      Ok(harness) => Some(harness),
      Err(_) => {
        eprintln!("pomijam: brak adaptera GPU");
        None
      }
    }
  }

  pub fn world(&mut self) -> &mut World {
    self.apps.main.world_mut()
  }

  pub fn update(&mut self) {
    self.apps.update();
    self.apps.main.world().resource::<RenderDevice>().wgpu_device().poll(PollType::Wait { submission_index: None, timeout: None }).unwrap();
  }

  /// Wykonuje `steps` kroków (do `MAX_STEPS_PER_FRAME` naraz) i czeka na ich zakończenie na GPU.
  pub fn step(&mut self, steps: u64) {
    let before = self.world().resource::<GpuProgress>().0.load(Ordering::SeqCst);
    self.world().resource_mut::<GpuParams>().step_count += steps;
    let deadline = Instant::now() + Duration::from_secs(120);
    while self.world().resource::<GpuProgress>().0.load(Ordering::SeqCst) < before + steps {
      assert!(Instant::now() < deadline, "GPU nie wykonał kroków w 120 s (błąd shadera?)");
      self.update();
    }
  }

  /// Wykonuje `steps` kroków (czeka na kompilację shaderów) i zwraca stan cząstek z GPU.
  pub fn run(&mut self, steps: u64) -> Vec<GpuParticle> {
    let before = self.world().resource::<GpuProgress>().0.load(Ordering::SeqCst);
    self.world().resource_mut::<GpuParams>().step_count += steps;
    let deadline = Instant::now() + Duration::from_secs(120);
    while self.world().resource::<GpuProgress>().0.load(Ordering::SeqCst) < before + steps {
      assert!(Instant::now() < deadline, "GPU nie wykonał kroków w 120 s (błąd shadera?)");
      self.update();
      std::thread::sleep(Duration::from_millis(5));
    }
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    self.world().insert_resource(Request(Some(slot.clone())));
    while slot.lock().unwrap().is_none() {
      assert!(Instant::now() < deadline, "brak odpowiedzi readbacku");
      self.update();
    }
    let bytes = slot.lock().unwrap().take().unwrap();
    bytemuck::cast_slice(&bytes).to_vec()
  }
}

/// Prośba o readback stanu; `ReadbackOnce` musi powstać w trakcie klatki (czyszczone w `First`).
#[derive(Resource)]
struct Request(Option<Arc<Mutex<Option<Vec<u8>>>>>);

fn request_readback(mut commands: Commands, mut request: ResMut<Request>, sim: Res<GpuSim>) {
  if let Some(sink) = request.0.take() {
    commands
      .spawn(ReadbackOnce::buffer(sim.state.clone()))
      .observe(move |event: On<ReadbackComplete>| *sink.lock().unwrap() = Some(event.data.clone()));
  }
}

#[derive(Resource)]
struct Initial(Option<Vec<GpuParticle>>, Vec2);

fn spawn_test_sim(
  mut commands: Commands,
  mut initial: ResMut<Initial>,
  spec: Res<ParticleSpec>,
  buffers: ResMut<Assets<ShaderBuffer>>,
  meshes: ResMut<Assets<Mesh>>,
  materials: ResMut<Assets<ParticleMaterial>>,
) {
  let colors = vec![Color::WHITE; spec.interactions.len()];
  spawn_sim(&mut commands, buffers, meshes, materials, initial.0.take().unwrap(), &spec, initial.1, 2.0, colors);
}


