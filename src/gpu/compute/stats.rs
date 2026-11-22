//! Statystyki stanu po kroku na GPU (opcja, domyślnie wyłączona): przebieg `reduce_stats` po siłach
//! liczy liczbę cząstek z nieskończonymi wartościami, maksymalną prędkość i energię kinetyczną. Wynik
//! wraca na CPU asynchronicznie, tak jak czasy przebiegów ([`super::timing`]).
//!
//! Wyłączone: ani przebiegu, ani kopii, ani mapowania bufora (bufory istnieją, nic ich nie dotyka).
//! Włączone: jeden przebieg w klatce z krokami (po ostatnim kroku planu), kopia do bufora do odczytu,
//! `map_async` po wysłaniu klatki, a wynik w `GpuStats::latest` w następnej klatce. Nowy przebieg
//! startuje dopiero, gdy poprzednia próbka odczytana; pipeline kompiluje się przy pierwszym włączeniu.
//!
//! Układ bufora `stats` (słowa u32): `[0]` liczba cząstek z nieskończonym lub NaN w `pos`/`vel`,
//! `[1]` maksymalna prędkość jako bity f32, `[2 + g]` suma 0.5·|v|² grupy `g` (bity f32).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::*;

use super::WORKGROUP;
use super::buffers::Buffers;

/// Prośba o statystyki (świat główny) i ostatni wynik.
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuStats {
  pub enabled: bool,
  pub latest: Arc<Mutex<Option<SimStats>>>,
}

/// Statystyki stanu po kroku, liczone na GPU w f32 (energia sumowana po grupach w f64 na CPU).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimStats {
  /// Liczba cząstek.
  pub particles: u32,
  /// Cząstki z nieskończonym albo NaN w `pos` lub `vel`.
  pub nan_count: u32,
  /// Największa `|vel|` po cząstkach skończonych.
  pub max_speed: f32,
  /// Suma `0.5·|vel|²` po cząstkach skończonych (masa 1).
  pub kinetic_energy: f32,
}

/// Grup redukcji (jedna na `WORKGROUP` cząstek), czyli sum częściowych energii.
pub fn partials(particles: u32) -> u32 {
  particles.div_ceil(WORKGROUP)
}

/// Słów u32 w buforze `stats` dla `particles` cząstek.
pub fn words(particles: u32) -> u64 {
  2 + partials(particles) as u64
}

/// Stan odczytu w świecie renderowania: czeka na kopię, mapowanie albo odbiór wyniku.
#[derive(Resource, Default)]
pub struct Reader {
  state: State,
}

#[derive(Default)]
enum State {
  #[default]
  Idle,
  /// Wynik skopiowany do bufora do odczytu w tej klatce.
  Encoded { particles: u32 },
  Mapping {
    particles: u32,
    ready: Arc<AtomicBool>,
  },
}

impl Reader {
  /// Można zlecić nowy przebieg (poprzednia próbka odebrana).
  pub fn idle(&self) -> bool {
    matches!(self.state, State::Idle)
  }

  /// Po przebiegu tej klatki: kopia wyniku do bufora do odczytu.
  pub fn encode(&mut self, encoder: &mut wgpu::CommandEncoder, buffers: &Buffers, particles: u32) {
    encoder.copy_buffer_to_buffer(
      &buffers.stats,
      0,
      &buffers.stats_read,
      0,
      Some(words(particles) * 4),
    );
    self.state = State::Encoded { particles };
  }
}

/// Po wysłaniu klatki: mapowanie bufora z wynikiem.
pub fn map(reader: Option<ResMut<Reader>>, buffers: Option<Res<Buffers>>) {
  let (Some(mut reader), Some(buffers)) = (reader, buffers) else {
    return;
  };
  let State::Encoded { particles } = reader.state else {
    return;
  };
  let ready = Arc::new(AtomicBool::new(false));
  let flag = ready.clone();
  buffers
    .stats_read
    .slice(..words(particles) * 4)
    .map_async(MapMode::Read, move |result| {
      flag.store(result.is_ok(), Ordering::Release)
    });
  reader.state = State::Mapping { particles, ready };
}

/// Gdy mapowanie skończone: statystyki do `GpuStats::latest`.
pub fn collect(
  reader: Option<ResMut<Reader>>,
  buffers: Option<Res<Buffers>>,
  gpu_stats: Option<Res<GpuStats>>,
) {
  let (Some(mut reader), Some(buffers), Some(gpu_stats)) = (reader, buffers, gpu_stats) else {
    return;
  };
  let State::Mapping { particles, ready } = &reader.state else {
    return;
  };
  if !ready.load(Ordering::Acquire) {
    return;
  }
  let particles = *particles;
  let data: Vec<u32> = {
    let view = buffers
      .stats_read
      .slice(..words(particles) * 4)
      .get_mapped_range()
      .expect("bufor statystyk zmapowany");
    bytemuck::cast_slice(&view).to_vec()
  };
  buffers.stats_read.unmap();
  reader.state = State::Idle;
  let kinetic: f64 = data[2..]
    .iter()
    .map(|&bits| f32::from_bits(bits) as f64)
    .sum();
  *gpu_stats.latest.lock().unwrap() = Some(SimStats {
    particles,
    nan_count: data[0],
    max_speed: f32::from_bits(data[1]),
    kinetic_energy: kinetic as f32,
  });
}
