//! Aplikacja bez okna z symulacją GPU i własnym kołem aktualizacji: testy i benchmarki.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bevy::app::SubApps;
use bevy::render::RenderPlugin;
use bevy::render::gpu_readback::{ReadbackComplete, ReadbackOnce};
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{PollType, TextureFormat, TextureUsages};
use bevy::render::renderer::RenderDevice;
use bevy::render::storage::ShaderBuffer;
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;

use super::material::ParticleMaterial;
use super::*;
use crate::options::SimOptions;

/// Jedna aplikacja GPU naraz w procesie (wspólne pule wątków i dziennik).
static LOCK: Mutex<()> = Mutex::new(());

/// Rysowanie bez okna: kamera renderuje cały świat do obrazu o rozmiarze `size` (w pikselach
/// fizycznych) z podaną jakością. Do benchmarku renderu i testów rysowania.
#[derive(Clone, Copy, Debug)]
pub struct Offscreen {
  pub size: UVec2,
  pub quality: RenderQuality,
}

/// Aplikacja bez okna, z własnym kołem aktualizacji; `None` bez adaptera GPU.
pub struct Harness {
  apps: SubApps,
  _guard: MutexGuard<'static, ()>,
}

impl Harness {
  pub fn new(options: SimOptions, spec: ParticleSpec, particles: Vec<GpuParticle>, world: Vec2) -> Option<Harness> {
    Self::with_render(options, spec, particles, world, None)
  }

  /// Jak `new`, ale z kamerą rysującą do obrazu (`Some`): każda klatka (`update`, `step`) renderuje
  /// też cząstki.
  pub fn with_render(
    options: SimOptions,
    spec: ParticleSpec,
    particles: Vec<GpuParticle>,
    world: Vec2,
    offscreen: Option<Offscreen>,
  ) -> Option<Harness> {
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
      app.insert_resource(Initial(Some(particles), world, offscreen));
      app.insert_resource(Request(None)).insert_resource(ImageRequest(None));
      app.add_systems(Startup, spawn_test_sim).add_systems(
        Update,
        (
          request_readback.run_if(resource_exists::<GpuSim>),
          request_image.run_if(resource_exists::<OffscreenTarget>),
        ),
      );
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

  /// Zmienia opcje symulacji w trakcie, jak klawisze w `ui`: od następnego kroku działa
  /// wariant sił dla nowych opcji.
  pub fn set_options(&mut self, options: SimOptions) {
    *self.world().resource_mut::<SimOptions>() = options;
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
    // Stan na GPU jest w kolejności siatki; testy i benchmarki widzą kolejność id.
    let mut particles: Vec<GpuParticle> = bytemuck::cast_slice(&bytes).to_vec();
    particles.sort_by_key(|p| p.id);
    particles
  }

  /// Piksele obrazu, do którego rysuje kamera (`with_render`): RGBA8, wiersz po wierszu od góry.
  pub fn target_pixels(&mut self) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let slot: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    self.world().insert_resource(ImageRequest(Some(slot.clone())));
    while slot.lock().unwrap().is_none() {
      assert!(Instant::now() < deadline, "brak odpowiedzi readbacku obrazu");
      self.update();
    }
    let bytes = slot.lock().unwrap().take().unwrap();
    // Wiersze odczytu tekstury są dopełnione do `COPY_BYTES_PER_ROW_ALIGNMENT` (256 B).
    let size = self.world().resource::<OffscreenTarget>().1;
    let row = (size.x * 4) as usize;
    let stride = row.next_multiple_of(256);
    assert_eq!(bytes.len(), stride * size.y as usize, "nieoczekiwany rozmiar odczytu obrazu");
    bytes.chunks(stride).flat_map(|line| &line[..row]).copied().collect()
  }
}

/// Obraz, do którego rysuje kamera offscreen.
#[derive(Resource)]
struct OffscreenTarget(Handle<Image>, UVec2);

/// Prośba o readback obrazu docelowego.
#[derive(Resource)]
struct ImageRequest(Option<Arc<Mutex<Option<Vec<u8>>>>>);

fn request_image(mut commands: Commands, mut request: ResMut<ImageRequest>, target: Res<OffscreenTarget>) {
  if let Some(sink) = request.0.take() {
    commands
      .spawn(ReadbackOnce::texture(target.0.clone()))
      .observe(move |event: On<ReadbackComplete>| *sink.lock().unwrap() = Some(event.data.clone()));
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
struct Initial(Option<Vec<GpuParticle>>, Vec2, Option<Offscreen>);

fn spawn_test_sim(
  mut commands: Commands,
  mut initial: ResMut<Initial>,
  spec: Res<ParticleSpec>,
  buffers: ResMut<Assets<ShaderBuffer>>,
  meshes: ResMut<Assets<Mesh>>,
  materials: ResMut<Assets<ParticleMaterial>>,
  mut images: ResMut<Assets<Image>>,
) {
  let colors = vec![Color::WHITE; spec.interactions.len()];
  let world = initial.1;
  let analytic_aa = initial.2.is_some_and(|o| o.quality.analytic_aa);
  let look = ParticleLook { size: 2.0, colors, analytic_aa };
  spawn_sim(&mut commands, buffers, meshes, materials, initial.0.take().unwrap(), &spec, world, look);
  if let Some(Offscreen { size, quality }) = initial.2 {
    let mut image = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
    // Odczyt obrazu (`target_pixels`) kopiuje teksturę.
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = images.add(image);
    commands.insert_resource(OffscreenTarget(target.clone(), size));
    // Cały świat na całym obrazie, więc cząstka ma ten sam rozmiar w pikselach co w aplikacji
    // na ekranie o tym rozmiarze (`size / world` pikseli na jednostkę).
    spawn_camera(&mut commands, Some(target), ScalingMode::Fixed { width: world.x, height: world.y }, &quality);
  }
}


