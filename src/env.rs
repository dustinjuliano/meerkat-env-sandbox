use std::collections::HashMap;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct FrameId(pub u32);

pub struct Frame {
  pub id: FrameId,
  pub parent: FrameId,
}

pub struct RegionId(pub u32);

pub struct Region<T> {
  pub id: RegionId,
  pub frames: Vec<Frame>,
  pub bindings: HashMap<(FrameId, String), T>,
}

pub struct Context<T> {
  pub next_region_id: u32,
  pub regions: Vec<Region<T>>,
}

impl<T> Context<T> {
  pub fn new() -> Self {
    Context {
      next_region_id: 0,
      regions: vec![],
    }
  }

  pub fn new_region(&mut self) -> &mut Region<T> {
    let id = RegionId(self.next_region_id);
    self.next_region_id += 1;
    let region = Region::new(id);
    self.regions.push(region);
    self.regions.last_mut().unwrap()
  }
}

impl<T> Region<T> {
  pub fn new(region_id: RegionId) -> Self {
    Region {
      id: region_id,
      frames: vec![],
      bindings: HashMap::new(),
    }
  }

  pub fn insert(&mut self, frame_id: FrameId, name: String, value: T) {
    self.bindings.insert((frame_id, name), value);
  }

  pub fn find(&self, mut frame: FrameId, name: &str) -> Option<&T> {
    loop {
      if let Some(val) = self.bindings.get(&(frame, name.to_string())) {
        return Some(val);
      }

      if frame == FrameId(0) {
        return None;
      }

      frame = self.frames.get((frame.0 - 1) as usize)?.parent;
    }
  }
}
