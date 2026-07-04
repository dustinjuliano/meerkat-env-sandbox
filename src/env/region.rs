//! Region data structure containing memory intervals and symbol bindings

use std::collections::HashMap;

use super::{BlockId, Symbol, EntryId};
use super::block::Interval;

/// Memory region containing block intervals and symbol bindings
pub(super) struct Region {
  pub(super) intervals: Vec<Interval>,
  pub(super) bindings: HashMap<(BlockId, Symbol), EntryId>,
  pub(super) active_interval_used: u32,
}

impl Region {
  /// Clears all allocated intervals and symbol bindings
  pub(super) fn clear(&mut self) {
    self.intervals.clear();
    self.bindings.clear();
    self.active_interval_used = 0;
  }
}

impl Default for Region {
  /// Creates a default empty region
  fn default() -> Self {
    Region {
      intervals: Vec::new(),
      bindings: HashMap::new(),
      active_interval_used: 0,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::env::{Symbol, EntryId};

  /// Verifies default initialization of `Region`
  #[test]
  fn test_region_default() {
    let r = Region::default();
    assert_eq!(r.intervals.len(), 0);
    assert_eq!(r.bindings.len(), 0);
    assert_eq!(r.active_interval_used, 0);
  }

  /// Verifies that `clear` resets the region state
  #[test]
  fn test_region_clear() {
    let mut r = Region::default();
    r.intervals.push(Interval {
      begin: BlockId(1),
      end: BlockId(5),
    });
    r.bindings.insert((BlockId(1), Symbol(10)), EntryId(20));
    r.active_interval_used = 3;

    r.clear();
    assert_eq!(r.intervals.len(), 0);
    assert_eq!(r.bindings.len(), 0);
    assert_eq!(r.active_interval_used, 0);
  }
}

