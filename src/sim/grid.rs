use bevy::prelude::*;

/// Siatka przestrzenna z zawijaniem (torus) do szukania sąsiadów cząstek.
/// Świat obejmuje `[-half_size, half_size]` w obu osiach.
///
/// Komórka `(ix, iy)` zawiera pozycje, dla których `round(x / granularity) == ix` (tak samo
/// dla y). Skrajne komórki (`left`/`right`, `bottom`/`top`) leżą w połowie poza światem i
/// razem tworzą jedną komórkę na szwie, dlatego zapytanie o komórkę przy krawędzi zagląda też
/// do jej odpowiednika po drugiej stronie. Komórki są w płaskim `Vec`, wierszami.
#[derive(Resource)]
pub struct SpatialGrid {
  half_size: Vec2,
  granularity: f32,
  left: i32,
  right: i32,
  bottom: i32,
  top: i32,
  columns: usize,
  cells: Vec<Vec<Entity>>,
}

/// Komórki do przejrzenia dla jednej komórki sąsiedztwa: ona sama i do trzech odpowiedników
/// po drugiej stronie szwu. `len` mówi, ile pozycji tablicy jest ważnych.
#[derive(Clone, Copy)]
struct Wrapped {
  buckets: [(i32, i32); 4],
  len: usize,
}

impl SpatialGrid {
  const OFFSETS: [(i32, i32); 9] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 0),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
  ];

  pub fn new(width: f32, height: f32, granularity: f32) -> SpatialGrid {
    let half_size = Vec2::new(width / 2.0, height / 2.0);
    let right = (half_size.x / granularity).round() as i32;
    let top = (half_size.y / granularity).round() as i32;
    let left = (-half_size.x / granularity).round() as i32;
    let bottom = (-half_size.y / granularity).round() as i32;
    let columns = (right - left + 1) as usize;
    let rows = (top - bottom + 1) as usize;
    SpatialGrid {
      half_size,
      granularity,
      left,
      right,
      bottom,
      top,
      columns,
      cells: vec![Vec::new(); columns * rows],
    }
  }

  pub fn get_corrected_position_delta(&self, origin: Vec2, target: Vec2) -> Vec2 {
    let delta = target - origin;
    delta + self.get_wrap_around_adjustment(delta)
  }

  pub fn get_wrap_around_adjustment(&self, point: Vec2) -> Vec2 {
    let mut adjustment = Vec2::ZERO;
    if point.x > self.half_size.x {
      adjustment.x = -self.half_size.x;
    } else if point.x < -self.half_size.x {
      adjustment.x = self.half_size.x;
    }
    if point.y > self.half_size.y {
      adjustment.y = -self.half_size.y;
    } else if point.y < -self.half_size.y {
      adjustment.y = self.half_size.y;
    }
    2.0 * adjustment
  }

  fn get_wrapped_buckets(&self, ix: i32, iy: i32) -> Wrapped {
    let (left, right, bottom, top) = (self.left, self.right, self.bottom, self.top);
    let mut buckets = [(ix, iy); 4];
    let mut len = 1;

    if ix <= left {
      buckets[len].0 += right - left;
      len += 1;
    } else if ix >= right {
      buckets[len].0 -= right - left;
      len += 1;
    }

    if iy <= bottom {
      buckets[len].1 += top - bottom;
      len += 1;
    } else if iy >= top {
      buckets[len].1 -= top - bottom;
      len += 1;
    }

    if (ix <= left || ix >= right) && (iy <= bottom || iy >= top) {
      buckets[len].0 = buckets[1].0;
      buckets[len].1 = buckets[2].1;
      len += 1;
    }

    Wrapped { buckets, len }
  }

  /// Indeks komórki w `cells`, albo `None` poza siatką (taka komórka jest zawsze pusta).
  fn cell_index(&self, (ix, iy): (i32, i32)) -> Option<usize> {
    if ix < self.left || ix > self.right || iy < self.bottom || iy > self.top {
      return None;
    }
    Some((iy - self.bottom) as usize * self.columns + (ix - self.left) as usize)
  }

  fn cell_mut(&mut self, x: f32, y: f32) -> &mut Vec<Entity> {
    let coords = self.bucket_coords(x, y);
    let index = self
      .cell_index(coords)
      .unwrap_or_else(|| panic!("position ({x}, {y}) outside the world"));
    &mut self.cells[index]
  }

  pub fn insert_entity(&mut self, entity: Entity, x: f32, y: f32) {
    self.cell_mut(x, y).push(entity)
  }

  pub fn remove_entity(&mut self, entity: Entity, x_old: f32, y_old: f32) {
    let bucket_old = self.cell_mut(x_old, y_old);
    let idx = bucket_old
      .iter()
      .position(|&e| e == entity)
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

  pub fn get_entities(&self, (ix, iy): (i32, i32)) -> impl Iterator<Item = Entity> + Clone + '_ {
    Self::OFFSETS
      .iter()
      .flat_map(move |&(xoff, yoff)| {
        let wrapped = self.get_wrapped_buckets(ix + xoff, iy + yoff);
        wrapped.buckets.into_iter().take(wrapped.len)
      })
      .filter_map(|bucket| self.cell_index(bucket))
      .flat_map(|index| self.cells[index].iter().copied())
  }

  pub fn get_entities_by_position(&self, x: f32, y: f32) -> impl Iterator<Item = Entity> + '_ {
    self.get_entities(self.bucket_coords(x, y))
  }

  pub fn bucket_coords(&self, x: f32, y: f32) -> (i32, i32) {
    (
      (x / self.granularity).round() as i32,
      (y / self.granularity).round() as i32,
    )
  }
}

#[cfg(test)]
mod tests;
