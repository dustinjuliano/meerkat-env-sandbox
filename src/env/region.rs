//! Region data structure containing memory intervals and symbol bindings

use std::collections::HashMap;

use super::{BlockId, Symbol, EntryId};
use super::block::Interval;

/// Memory region containing block intervals and symbol bindings
#[derive(Default)]
pub(super) struct Region {
  pub(super) is_active: bool,
  pub(super) intervals: Vec<Interval>,
  pub(super) bindings: HashMap<(BlockId, Symbol), EntryId>,
  pub(super) active_interval_used: u32,
}

impl Region {
  /// Clears all allocated intervals and symbol bindings
  pub(super) fn clear(&mut self) {
    self.is_active = false;
    self.intervals.clear();
    self.bindings.clear();
    self.active_interval_used = 0;
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
    assert!(!r.is_active);
    assert_eq!(r.intervals.len(), 0);
    assert_eq!(r.bindings.len(), 0);
    assert_eq!(r.active_interval_used, 0);
  }

  /// Verifies that `clear` resets the region state
  #[test]
  fn test_region_clear() {
    let mut r = Region::default();
    r.is_active = true;
    r.intervals.push(Interval {
      begin: BlockId(1),
      end: BlockId(5),
    });
    r.bindings.insert((BlockId(1), Symbol(10)), EntryId(20));
    r.active_interval_used = 3;

    r.clear();
    assert!(!r.is_active);
    assert_eq!(r.intervals.len(), 0);
    assert_eq!(r.bindings.len(), 0);
    assert_eq!(r.active_interval_used, 0);
  }

  /// Verifies direct struct construction of `Region`
  #[test]
  fn test_region_struct_construction() {
    let mut bindings = HashMap::new();
    bindings.insert((BlockId(2), Symbol(5)), EntryId(12));
    
    let r = Region {
      is_active: true,
      intervals: vec![Interval {
        begin: BlockId(2),
        end: BlockId(4),
      }],
      bindings,
      active_interval_used: 1,
    };
    
    assert!(r.is_active);
    assert_eq!(r.intervals.len(), 1);
    assert_eq!(r.intervals[0].begin.0, 2);
    assert_eq!(r.intervals[0].end.0, 4);
    assert_eq!(r.bindings.get(&(BlockId(2), Symbol(5))), Some(&EntryId(12)));
    assert_eq!(r.active_interval_used, 1);
  }

  /// Verifies that `clear` is safe to call multiple times
  #[test]
  fn test_region_double_clear() {
    let mut r = Region::default();
    r.clear();
    assert!(!r.is_active);
    assert_eq!(r.intervals.len(), 0);
    assert_eq!(r.bindings.len(), 0);
    assert_eq!(r.active_interval_used, 0);
    
    r.clear();
    assert!(!r.is_active);
    assert_eq!(r.intervals.len(), 0);
    assert_eq!(r.bindings.len(), 0);
    assert_eq!(r.active_interval_used, 0);
  }
}

