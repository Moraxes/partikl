//! Czas GPU przebiegów kroku ze znaczników czasu (timestamp queries).
//!
//! Na Metalu (Apple) znaczniki da się stawiać tylko na granicach przebiegów compute
//! (`TIMESTAMP_QUERY` bez `TIMESTAMP_QUERY_INSIDE_PASSES`), więc w klatce z pomiarem każdy
//! dispatch pierwszego kroku dostaje własny przebieg ze znacznikiem na początku i końcu;
//! pozostałe kroki klatki idą jednym przebiegiem. Przebieg ze znacznikami ma narzut (na M5 ok.
//! 20 µs, wliczony w czas krótkich przebiegów), dlatego pomiar włącza tylko benchmark
//! (`GpuTimings::enabled`); aplikacja liczy krok jednym przebiegiem.
//!
//! Cykl jednej próbki (świat renderowania):
//! 1. `step::run`: przebiegi ze znacznikami, `resolve_query_set` i kopia do bufora do odczytu,
//! 2. [`map`] (po wysłaniu klatki): `map_async` bufora,
//! 3. [`collect`] (w następnej klatce, gdy mapowanie skończone): czasy do `GpuTimings::latest`.
//!
//! Bez `TIMESTAMP_QUERY` na urządzeniu pomiar jest niedostępny (`Timer` nie powstaje). Próbka
//! z zerowym znacznikiem (na M5 w pierwszej klatce z pomiarem) przepada.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::RenderApp;

use super::pipelines::{FORCES, GRID_PASSES};

/// Prośba o pomiar (świat główny) i jego ostatni wynik.
#[derive(Resource, Clone, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub struct GpuTimings {
  pub enabled: bool,
  pub latest: Arc<Mutex<Option<PassTimes>>>,
}

/// Czas GPU kroku, osobno dla każdego przebiegu (entry point), z pierwszego kroku klatki.
#[derive(Clone, Debug, PartialEq)]
pub struct PassTimes {
  /// Zmierzonych kroków (1: pierwszy krok klatki).
  pub steps: u32,
  /// `(entry point, ns na krok)` w kolejności przebiegów; przebiegi pominięte w tej klatce
  /// (`sort_cells` bez trybu deterministycznego, `count` poza pierwszym krokiem po wgraniu stanu)
  /// mają 0.
  pub per_step_ns: Vec<(&'static str, f64)>,
}

impl PassTimes {
  /// Suma przebiegów: czas GPU kroku (bez przerw między przebiegami).
  pub fn total_ns(&self) -> f64 {
    self.per_step_ns.iter().map(|(_, ns)| ns).sum()
  }

  pub fn get(&self, entry: &str) -> f64 {
    self.per_step_ns.iter().find(|(e, _)| *e == entry).map_or(0.0, |(_, ns)| *ns)
  }
}

/// Wszystkie przebiegi kroku w kolejności wykonania.
pub fn entries() -> impl Iterator<Item = &'static str> {
  GRID_PASSES.iter().map(|p| p.entry).chain([FORCES.entry])
}

/// Dwa znaczniki (początek, koniec) na dispatch jednego kroku.
const MAX_QUERIES: u32 = 2 * (GRID_PASSES.len() as u32 + 1);

#[derive(Resource)]
pub struct Timer {
  pub query_set: wgpu::QuerySet,
  resolve: Buffer,
  read: Buffer,
  period_ns: f64,
  state: State,
}

enum State {
  Idle,
  /// Znaczniki zakodowane w tej klatce: entry point każdego dispatchu.
  Encoded { steps: u32, dispatches: Vec<&'static str> },
  Mapping { steps: u32, dispatches: Vec<&'static str>, ready: Arc<AtomicBool> },
}

impl Timer {
  /// Można zacząć nową próbkę (poprzednia odczytana).
  pub fn idle(&self) -> bool {
    matches!(self.state, State::Idle)
  }

  /// Znaczniki `index`-tego dispatchu w klatce.
  pub fn writes(&self, index: u32) -> wgpu::ComputePassTimestampWrites<'_> {
    wgpu::ComputePassTimestampWrites {
      query_set: &self.query_set,
      beginning_of_pass_write_index: Some(2 * index),
      end_of_pass_write_index: Some(2 * index + 1),
    }
  }

  /// Po dispatchach klatki: wyniki do bufora do odczytu.
  pub fn finish_frame(&mut self, encoder: &mut wgpu::CommandEncoder, steps: u32, dispatches: Vec<&'static str>) {
    let queries = 2 * dispatches.len() as u32;
    encoder.resolve_query_set(&self.query_set, 0..queries, &self.resolve, 0);
    encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, Some(queries as u64 * 8));
    self.state = State::Encoded { steps, dispatches };
  }
}

pub fn init(mut commands: Commands, device: Res<RenderDevice>, queue: Res<RenderQueue>) {
  if !device.features().contains(WgpuFeatures::TIMESTAMP_QUERY) {
    info!("brak TIMESTAMP_QUERY: czas GPU przebiegów niedostępny");
    return;
  }
  let size = MAX_QUERIES as u64 * 8;
  let buffer = |label, usage| device.create_buffer(&BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
  commands.insert_resource(Timer {
    query_set: device.wgpu_device().create_query_set(&wgpu::QuerySetDescriptor {
      label: Some("partikl_timestamps"),
      ty: wgpu::QueryType::Timestamp,
      count: MAX_QUERIES,
    }),
    resolve: buffer("partikl_timestamps_resolve", BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC),
    read: buffer("partikl_timestamps_read", BufferUsages::MAP_READ | BufferUsages::COPY_DST),
    period_ns: queue.get_timestamp_period() as f64,
    state: State::Idle,
  });
}

/// Po wysłaniu klatki: mapowanie bufora z wynikami.
pub fn map(timer: Option<ResMut<Timer>>) {
  let Some(mut timer) = timer else { return };
  let State::Encoded { steps, dispatches } = std::mem::replace(&mut timer.state, State::Idle) else { return };
  let ready = Arc::new(AtomicBool::new(false));
  let flag = ready.clone();
  let bytes = 2 * dispatches.len() as u64 * 8;
  timer.read.slice(..bytes).map_async(MapMode::Read, move |result| flag.store(result.is_ok(), Ordering::Release));
  timer.state = State::Mapping { steps, dispatches, ready };
}

/// Gdy mapowanie skończone: czasy przebiegów do `GpuTimings::latest`.
pub fn collect(timer: Option<ResMut<Timer>>, timings: Option<Res<GpuTimings>>) {
  let (Some(mut timer), Some(timings)) = (timer, timings) else { return };
  let State::Mapping { ready, .. } = &timer.state else { return };
  if !ready.load(Ordering::Acquire) {
    return;
  }
  let State::Mapping { steps, dispatches, .. } = std::mem::replace(&mut timer.state, State::Idle) else { unreachable!() };
  let bytes = 2 * dispatches.len() as u64 * 8;
  let ticks: Vec<u64> = {
    let view = timer.read.slice(..bytes).get_mapped_range().expect("bufor znaczników zmapowany");
    bytemuck::cast_slice(&view).to_vec()
  };
  timer.read.unmap();
  // Na M5 w pierwszej klatce z pomiarem znaczniki ostatniego przebiegu bywają zerowe
  // (przyczyna nieustalona); takiej próbki nie zgłaszamy.
  if ticks.contains(&0) {
    return;
  }
  let mut per_step_ns: Vec<(&'static str, f64)> = entries().map(|e| (e, 0.0)).collect();
  for (i, entry) in dispatches.iter().enumerate() {
    let ns = ticks[2 * i + 1].saturating_sub(ticks[2 * i]) as f64 * timer.period_ns;
    per_step_ns.iter_mut().find(|(e, _)| e == entry).unwrap().1 += ns / steps as f64;
  }
  *timings.latest.lock().unwrap() = Some(PassTimes { steps, per_step_ns });
}
