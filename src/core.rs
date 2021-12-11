use bevy::prelude::*;

use std::collections::HashMap;

pub use crate::args::*;

pub const DELTA_TIME: f64 = 0.01;
pub const VELOCITY_THRESHOLD: f32 = 0.0001;

#[derive(Component, Default, Debug)]
pub struct Acceleration(pub Vec3);
#[derive(Component, Default, Debug)]
pub struct LastPosition(pub Vec3);

#[derive(Bundle, Default)]
pub struct ParticleBundle {
  #[bundle]
  pub mesh: MeshBundle,
  pub last_pos: LastPosition,
  pub acceleration: Acceleration,
  pub category: CategoryId
}

#[derive(Default)]
pub struct Categories(pub Vec<Category>);
#[derive(Default)]
pub struct Category {
  pub force_coeffs: Vec<f32>,
  pub color: [f32; 3],
  pub mesh_handle: Handle<Mesh>
}
#[derive(Component, Default, Clone, Copy)]
pub struct CategoryId(pub usize);

#[derive(Copy, Clone)]
pub struct SimRegion {
  top_right: Vec2
}

impl SimRegion {
  pub fn new(width: f32, height: f32) -> SimRegion {
    SimRegion { top_right: Vec2::new(width/2.0, height/2.0) }
  }

  pub fn get_corrected_position_delta(&self, origin: Vec3, target: Vec3) -> Vec3 {
    let delta = target - origin;
    delta + self.get_wrap_around_adjustment(delta)
  }

  pub fn get_wrap_around_adjustment(&self, point: Vec3) -> Vec3 {
    let mut adjustment = Vec3::ZERO;
    if point.x > self.top_right.x {
      adjustment.x = -self.top_right.x;
    } else if point.x < -self.top_right.x {
      adjustment.x = self.top_right.x;
    }
    if point.y > self.top_right.y {
      adjustment.y = -self.top_right.y;
    } else if point.y < -self.top_right.y {
      adjustment.y = self.top_right.y;
    }
    2.0 * adjustment
  }
}

#[derive(Default)]
pub struct SpatialIndex {
  pub granularity: f32,
  // pub left: f32,
  // pub right: f32,
  // pub top: f32,
  // pub bottom: f32,
  pub index: HashMap<(i32, i32), Vec<Entity>>,
}

impl SpatialIndex {
  const OFFSETS: [i32; 3] = [-1, 0, 1];
  const EMPTY_BUCKET: &'static [Entity] = &[];

  pub fn insert_entity(&mut self, entity: Entity, x: f32, y: f32) {
    self.index.entry(self.bucket_coords(x, y)).or_default().push(entity)
  }

  pub fn remove_entity(&mut self, entity: Entity, x_old: f32, y_old: f32) {
    let mut bucket_old = self.index.get_mut(&self.bucket_coords(x_old, y_old))
      .expect("no bucket");
    let idx = bucket_old.iter().position(|&e| e == entity)
      .expect("entity not in bucket");
    bucket_old.swap_remove(idx);
  }

  pub fn move_entity(&mut self, entity: Entity, x_old: f32, y_old: f32, x_new: f32, y_new: f32) {
    let old_bucket = self.bucket_coords(x_old, y_old);
    let new_bucket = self.bucket_coords(x_new, y_new);
    if old_bucket == new_bucket {
      return;
    }
    self.remove_entity(entity, x_old, y_old);
    self.insert_entity(entity, x_new, y_new);
  }

  pub fn get_bucket(&self, x: f32, y: f32) -> &[Entity] {
    // TODO: correctly handle region boundary
    let (ix, iy) = self.bucket_coords(x, y);
    self.index.get(&(ix, iy))
      .map(|vec| vec.as_slice())
      .unwrap_or(Self::EMPTY_BUCKET)
  }

  pub fn get_bucket_with_boundary(&self, x: f32, y: f32) -> impl Iterator<Item=Entity> + '_ {
    // TODO: correctly handle region boundary
    let (ix, iy) = self.bucket_coords(x, y);
    Self::OFFSETS.iter().cloned()
      .flat_map(move |xoff|
        Self::OFFSETS.iter().map(move |&yoff| (ix + xoff, iy + yoff)))
      .flat_map(|offset|
        self.index.get(&offset))
      .flatten()
      .cloned()
  }

  pub fn bucket_coords(&self, x: f32, y: f32) -> (i32, i32) {
    ((x / self.granularity).floor() as i32, (y / self.granularity).floor() as i32)
  }
}
