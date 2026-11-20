use bevy::prelude::*;

use super::InteractionId;

/// Siatka przestrzenna z zawijaniem (torus) do szukania sąsiadów cząstek.
/// Świat obejmuje `[-half_size, half_size]` w obu osiach.
///
/// Komórka `(ix, iy)` zawiera pozycje, dla których `round(x / granularity) == ix` (tak samo
/// dla y). Skrajne komórki (`left`/`right`, `bottom`/`top`) leżą w połowie poza światem i
/// razem tworzą jedną komórkę na szwie, dlatego zapytanie o komórkę przy krawędzi zagląda też
/// do jej odpowiednika po drugiej stronie. Komórki są w płaskim `Vec`, wierszami.
///
/// Oprócz encji komórka trzyma pozycję i typ cząstki ([`Neighbour`]), żeby liczenie sił nie
/// musiało pytać ECS o każdą parę. Pozycja musi być aktualna: `move_entity` co krok dla każdej
/// cząstki, także gdy nie zmienia komórki. Żeby to nie wymagało szukania w komórce, `slots`
/// mówi, gdzie leży każda encja (indeksowane `Entity::index`).
#[derive(Resource)]
pub struct SpatialGrid {
  half_size: Vec2,
  granularity: f32,
  left: i32,
  right: i32,
  bottom: i32,
  top: i32,
  columns: usize,
  cells: Vec<Vec<Neighbour>>,
  slots: Vec<Slot>,
  /// Sąsiedztwo każdej komórki policzone raz w `new`: indeksy komórek do przejrzenia (z
  /// zawijaniem) dla komórki `c` to `neighbour_cells[neighbour_starts[c]..neighbour_starts[c + 1]]`.
  neighbour_starts: Vec<u32>,
  neighbour_cells: Vec<u32>,
  /// To samo sąsiedztwo jako posortowane przebiegi kolejnych komórek `[od, do)`: komórki
  /// leżą wierszami, więc trzy komórki jednego wiersza to zwykle jeden przebieg. Dla komórki
  /// `c`: `neighbour_runs[run_starts[c]..run_starts[c + 1]]`.
  run_starts: Vec<u32>,
  neighbour_runs: Vec<(u32, u32)>,
}

/// Miejsce encji w siatce: komórka i pozycja w niej. `Slot::EMPTY`, jeśli encji nie ma.
#[derive(Clone, Copy, PartialEq)]
struct Slot {
  cell: u32,
  index: u32,
}

impl Slot {
  const EMPTY: Slot = Slot { cell: u32::MAX, index: u32::MAX };
}

/// Cząstka w komórce siatki.
#[derive(Clone, Copy)]
pub struct Neighbour {
  pub entity: Entity,
  pub position: Vec2,
  pub interaction: InteractionId,
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
    let mut grid = SpatialGrid {
      half_size,
      granularity,
      left,
      right,
      bottom,
      top,
      columns,
      cells: vec![Vec::new(); columns * rows],
      slots: Vec::new(),
      neighbour_starts: Vec::new(),
      neighbour_cells: Vec::new(),
      run_starts: Vec::new(),
      neighbour_runs: Vec::new(),
    };
    let mut starts = Vec::with_capacity(columns * rows + 1);
    let mut cells = Vec::with_capacity(columns * rows * 9 + 64);
    starts.push(0);
    for iy in bottom..=top {
      for ix in left..=right {
        cells.extend(grid.neighbour_cells_of((ix, iy)).map(|c| c as u32));
        starts.push(cells.len() as u32);
      }
    }
    grid.neighbour_starts = starts;
    grid.neighbour_cells = cells;

    let mut run_starts = Vec::with_capacity(columns * rows + 1);
    let mut runs = Vec::with_capacity(columns * rows * 3 + 64);
    let mut sorted = Vec::with_capacity(36);
    run_starts.push(0);
    for c in 0..columns * rows {
      sorted.clear();
      sorted.extend_from_slice(grid.neighbour_cells_at(c));
      sorted.sort_unstable();
      let first = runs.len();
      for &cell in &sorted {
        match runs[first..].last_mut() {
          Some((_, end)) if *end == cell => *end += 1,
          _ => runs.push((cell, cell + 1)),
        }
      }
      run_starts.push(runs.len() as u32);
    }
    grid.run_starts = run_starts;
    grid.neighbour_runs = runs;
    grid
  }

  /// Indeksy komórek sąsiedztwa `(ix, iy)` z zawijaniem, w stałej kolejności (`OFFSETS`).
  fn neighbour_cells_of(&self, (ix, iy): (i32, i32)) -> impl Iterator<Item = usize> + Clone + '_ {
    Self::OFFSETS
      .iter()
      .flat_map(move |&(xoff, yoff)| {
        let wrapped = self.get_wrapped_buckets(ix + xoff, iy + yoff);
        wrapped.buckets.into_iter().take(wrapped.len)
      })
      .filter_map(|bucket| self.cell_index(bucket))
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

  fn neighbour_cells_at(&self, c: usize) -> &[u32] {
    &self.neighbour_cells[self.neighbour_starts[c] as usize..self.neighbour_starts[c + 1] as usize]
  }

  pub fn half_size(&self) -> Vec2 {
    self.half_size
  }

  /// Komórki w kolejności indeksów (wierszami).
  pub fn cells(&self) -> &[Vec<Neighbour>] {
    &self.cells
  }

  /// Indeks komórki zawierającej pozycję ze świata.
  pub fn cell_at(&self, x: f32, y: f32) -> usize {
    self
      .cell_index(self.bucket_coords(x, y))
      .unwrap_or_else(|| panic!("position ({x}, {y}) outside the world"))
  }

  /// Sąsiedztwo komórki `c` jako przebiegi indeksów komórek `[od, do)`, rosnąco. Te same
  /// komórki co w `get_neighbour_cells`, w innej kolejności.
  pub fn neighbour_runs(&self, c: usize) -> &[(u32, u32)] {
    &self.neighbour_runs[self.run_starts[c] as usize..self.run_starts[c + 1] as usize]
  }

  /// Indeks komórki w `cells`, albo `None` poza siatką (taka komórka jest zawsze pusta).
  fn cell_index(&self, (ix, iy): (i32, i32)) -> Option<usize> {
    if ix < self.left || ix > self.right || iy < self.bottom || iy > self.top {
      return None;
    }
    Some((iy - self.bottom) as usize * self.columns + (ix - self.left) as usize)
  }


  fn slot_key(entity: Entity) -> usize {
    entity.index_u32() as usize
  }

  /// Miejsce encji; panika, jeśli jej nie ma w siatce.
  fn slot(&self, entity: Entity) -> Slot {
    let slot = self.slots.get(Self::slot_key(entity)).copied().unwrap_or(Slot::EMPTY);
    let found = slot != Slot::EMPTY
      && self.cells[slot.cell as usize].get(slot.index as usize).map(|n| n.entity) == Some(entity);
    assert!(found, "entity not in bucket");
    slot
  }

  fn push(&mut self, cell: usize, neighbour: Neighbour) {
    let key = Self::slot_key(neighbour.entity);
    if self.slots.len() <= key {
      self.slots.resize(key + 1, Slot::EMPTY);
    }
    self.slots[key] = Slot { cell: cell as u32, index: self.cells[cell].len() as u32 };
    self.cells[cell].push(neighbour);
  }

  pub fn insert_entity(&mut self, entity: Entity, x: f32, y: f32, interaction: InteractionId) {
    let cell = self.cell_at(x, y);
    self.push(cell, Neighbour { entity, position: Vec2::new(x, y), interaction });
  }

  /// Usuwa encję z siatki. `(x_old, y_old)` to jej zapisana pozycja.
  pub fn remove_entity(&mut self, entity: Entity, x_old: f32, y_old: f32) -> Neighbour {
    let slot = self.slot(entity);
    debug_assert_eq!(slot.cell as usize, self.cell_at(x_old, y_old));
    let cell = &mut self.cells[slot.cell as usize];
    let removed = cell.swap_remove(slot.index as usize);
    if let Some(moved) = cell.get(slot.index as usize) {
      self.slots[Self::slot_key(moved.entity)] = slot;
    }
    self.slots[Self::slot_key(entity)] = Slot::EMPTY;
    removed
  }

  /// Przenosi cząstkę z `(x_old, y_old)` do `(x_new, y_new)`; zapisana pozycja zmienia się
  /// zawsze, komórka tylko gdy trzeba.
  pub fn move_entity(&mut self, entity: Entity, x_old: f32, y_old: f32, x_new: f32, y_new: f32) {
    let position = Vec2::new(x_new, y_new);
    let new_cell = self.cell_at(x_new, y_new);
    let slot = self.slot(entity);
    if slot.cell as usize == new_cell {
      self.cells[new_cell][slot.index as usize].position = position;
      return;
    }
    let mut neighbour = self.remove_entity(entity, x_old, y_old);
    neighbour.position = position;
    self.push(new_cell, neighbour);
  }

  /// Cząstki z komórki `(ix, iy)` i ośmiu sąsiednich, z zawijaniem. Zawiera też cząstkę, o
  /// którą pytamy, jeśli leży w tej komórce.
  ///
  /// Dla komórki w siatce sąsiedztwo jest z tablicy z `new`; poza siatką liczone na miejscu.
  pub fn get_neighbours(&self, coords: (i32, i32)) -> impl Iterator<Item = &Neighbour> + Clone + '_ {
    self.get_neighbour_cells(coords).flat_map(|cell| cell.iter())
  }

  /// Jak `get_neighbours`, ale po komórkach: każda jako wycinek, w tej samej kolejności.
  pub fn get_neighbour_cells(&self, coords: (i32, i32)) -> impl Iterator<Item = &[Neighbour]> + Clone + '_ {
    let (precomputed, computed) = match self.cell_index(coords) {
      Some(c) => {
        let range = self.neighbour_starts[c] as usize..self.neighbour_starts[c + 1] as usize;
        (Some(self.neighbour_cells[range].iter().map(|&i| i as usize)), None)
      }
      None => (None, Some(self.neighbour_cells_of(coords))),
    };
    precomputed
      .into_iter()
      .flatten()
      .chain(computed.into_iter().flatten())
      .map(|index| self.cells[index].as_slice())
  }

  pub fn get_neighbours_by_position(&self, x: f32, y: f32) -> impl Iterator<Item = &Neighbour> + '_ {
    self.get_neighbours(self.bucket_coords(x, y))
  }

  pub fn get_entities(&self, coords: (i32, i32)) -> impl Iterator<Item = Entity> + Clone + '_ {
    self.get_neighbours(coords).map(|n| n.entity)
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
