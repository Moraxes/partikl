//! Aplikacja bez okna z symulacją GPU i własnym kołem aktualizacji: testy i benchmarki.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bevy::app::SubApps;
use bevy::render::RenderPlugin;
use bevy::render::gpu_readback::{ReadbackComplete, ReadbackOnce};
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::PollType;
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use super::material::ParticleMaterial;
use super::*;
use crate::options::SimOptions;

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
          .disable::<WinitPlugin>()
          // Render w tym samym wywołaniu `update` co świat główny: po `update` i `poll(Wait)`
          // praca kroków jest już wysłana i skończona (pomiar czasu w benchu tego wymaga).
          .disable::<PipelinedRenderingPlugin>(),
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
      // Tylko brak adaptera oznacza „pomiń”; każdy inny panic to błąd do zgłoszenia.
      Err(payload) if is_missing_gpu(&*payload) => {
        eprintln!("pomijam: brak adaptera GPU");
        None
      }
      Err(payload) => std::panic::resume_unwind(payload),
    }
  }

  /// Tryb deterministyczny (sortowanie komórek po id).
  pub fn set_deterministic(&mut self, deterministic: bool) {
    self.world().resource_mut::<GpuParams>().deterministic = deterministic;
  }

  /// Podmienia stan cząstek (ta sama liczba cząstek) i wysyła go na GPU, bez kroku symulacji.
  /// Benchmark mierzy dzięki temu każdy pomiar od tego samego stanu.
  pub fn set_state(&mut self, particles: Vec<GpuParticle>) {
    // `GpuSim` powstaje w `Startup` (pierwsza klatka).
    if !self.world().contains_resource::<GpuSim>() {
      self.update();
    }
    let sim = self.world().resource::<GpuSim>().clone();
    assert_eq!(particles.len(), sim.num_particles as usize, "inna liczba cząstek");
    let buffer = ShaderBuffer::new(particles, bevy::asset::RenderAssetUsages::RENDER_WORLD);
    self.world().resource_mut::<Assets<ShaderBuffer>>().insert(&sim.state, buffer).unwrap();
    // Wysłanie nowego bufora (ekstrakcja i przygotowanie w świecie renderowania).
    self.update();
  }

  pub fn world(&mut self) -> &mut World {
    self.apps.main.world_mut()
  }

  pub fn update(&mut self) {
    self.apps.update();
    self.apps.main.world().resource::<RenderDevice>().wgpu_device().poll(PollType::Wait { submission_index: None, timeout: None }).unwrap();
  }

  /// Wykonuje `steps` kroków i czeka na ich zakończenie na GPU. Klatka wykonuje najwyżej
  /// `MAX_STEPS_PER_FRAME` kroków, więc dłuższe przebiegi idą porcjami.
  pub fn step(&mut self, steps: u64) {
    let mut left = steps;
    while left > 0 {
      let chunk = left.min(MAX_STEPS_PER_FRAME as u64);
      self.step_chunk(chunk);
      left -= chunk;
    }
  }

  /// Włącza pomiar czasu GPU przebiegów: od teraz klatka z krokami (do
  /// `MAX_STEPS_PER_FRAME`) ma znaczniki czasu, a wynik odbiera `take_gpu_times`.
  pub fn set_gpu_timing(&mut self, enabled: bool) {
    self.world().resource_mut::<GpuTimings>().enabled = enabled;
  }

  /// Czas GPU przebiegów ostatniej klatki z krokami; czeka (najwyżej kilka klatek bez kroków)
  /// na odczyt znaczników. `None`: pomiar wyłączony, urządzenie bez znaczników czasu albo
  /// próbka odrzucona (zerowy znacznik, na M5 w pierwszej klatce z pomiarem).
  /// Osobno od `step`, żeby czekanie na odczyt nie wchodziło do czasu kroku.
  pub fn take_gpu_times(&mut self) -> Option<PassTimes> {
    let latest = self.world().resource::<GpuTimings>().latest.clone();
    for _ in 0..4 {
      if let Some(times) = latest.lock().unwrap().take() {
        return Some(times);
      }
      self.update();
    }
    None
  }

  fn step_chunk(&mut self, steps: u64) {
    let before = self.world().resource::<GpuProgress>().0.load(Ordering::SeqCst);
    self.world().resource_mut::<GpuParams>().step_count += steps;
    let deadline = Instant::now() + Duration::from_secs(120);
    while self.world().resource::<GpuProgress>().0.load(Ordering::SeqCst) < before + steps {
      assert!(Instant::now() < deadline, "GPU nie wykonał kroków w 120 s (pipeline się nie skompilował?)");
      self.update();
    }
  }

  /// Wykonuje `steps` kroków (czeka na kompilację shaderów) i zwraca stan cząstek z GPU.
  pub fn run(&mut self, steps: u64) -> Vec<GpuParticle> {
    self.step(steps);
    let deadline = Instant::now() + Duration::from_secs(120);
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

/// Panic z `bevy_render`, gdy nie ma adaptera (`GPU_NOT_FOUND_ERROR_MESSAGE`).
fn is_missing_gpu(payload: &(dyn std::any::Any + Send)) -> bool {
  let message = payload.downcast_ref::<&str>().copied().or_else(|| payload.downcast_ref::<String>().map(String::as_str));
  message.is_some_and(|m| m.contains("Unable to find a GPU"))
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


