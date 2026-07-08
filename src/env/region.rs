//! Region data structure containing memory intervals and symbol bindings.
//!
//! A `Region<T>` stores values of type `T` directly in its bindings map.
//! The region makes no claim about the validity or freshness of those
//! values; see the `env` module documentation for the full caller
//! responsibility contract governing `T`

use std::collections::HashMap;

use super::block::Interval;
use super::{BlockId, Symbol};

/// Memory region containing block intervals and symbol bindings.
///
/// `T` is the value type stored per binding. The region holds `T` by
/// value and does not impose any bound on `T` beyond what its internal
/// operations require. Callers are responsible for all validity and
/// freshness invariants on `T`; see the `env` module documentation
pub(super) struct Region<T> {
    pub(super) is_active: bool,
    pub(super) intervals: Vec<Interval>,
    pub(super) bindings: HashMap<(BlockId, Symbol), T>,
    pub(super) active_interval_used: u32,
}

/// Default implementation for `Region<T>`.
///
/// Produces an inactive region with no intervals, no bindings, and
/// zero `active_interval_used`. Does not require `T: Default`
impl<T> Default for Region<T> {
    fn default() -> Self {
        Region {
            is_active: false,
            intervals: Vec::new(),
            bindings: HashMap::new(),
            active_interval_used: 0,
        }
    }
}

impl<T> Region<T> {
    /// Clears all allocated intervals and symbol bindings.
    ///
    /// Resets the region to an inactive, empty state. Drops all stored
    /// `T` values. After this call `is_active` is `false` and all
    /// collections are empty
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

    /// Verifies default initialization of `Region`
    #[test]
    fn test_region_default() {
        let r: Region<u32> = Region::default();
        assert!(!r.is_active);
        assert_eq!(r.intervals.len(), 0);
        assert_eq!(r.bindings.len(), 0);
        assert_eq!(r.active_interval_used, 0);
    }

    /// Verifies that `clear` resets the region state
    #[test]
    fn test_region_clear() {
        let mut r: Region<u32> = Region {
            is_active: true,
            ..Region::default()
        };
        r.intervals.push(Interval {
            begin: BlockId(1),
            end: BlockId(5),
        });
        r.bindings.insert((BlockId(1), Symbol(10)), 20u32);
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
        bindings.insert((BlockId(2), Symbol(5)), 12u32);

        let r: Region<u32> = Region {
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
        assert_eq!(r.bindings.get(&(BlockId(2), Symbol(5))), Some(&12u32));
        assert_eq!(r.active_interval_used, 1);
    }

    /// Verifies that `clear` is safe to call multiple times
    #[test]
    fn test_region_double_clear() {
        let mut r: Region<u32> = Region::default();
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
