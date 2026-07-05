//! Environment context managing block allocations and tracking
//!
//! # Sentinel conventions
//!
//! - [`BlockId(0)`](BlockId) is the null/sentinel value. Every arena access first
//!   checks `id.0 != 0` before computing an index.
//! - [`RegionId(0)`](RegionId) is **not** a sentinel; slot 0 is a valid live region.
//!   Do not treat `RegionId(0)` as "no region".

mod alloc;
mod block;
pub mod iter;
mod region;
use self::block::BlockId;
use self::block::Interval;
use std::collections::HashMap;
pub use self::iter::{Iter, IterMut};
use alloc::BlockAllocator;

/// The maximum block identifier allowed due to half-open ranges
pub const MAX_BLOCK_ID: u32 = u32::MAX - 1;

/// A unique identifier for regions
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegionId(pub u32);

/// A type-safe wrapper around symbol identifiers
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);

/// An opaque handle mapped to an external Entry storage
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EntryId(pub u32);

/// Context managing block arena, regions, and range allocations
pub struct Context {
  block_arena: Vec<block::Block>,
  region_arena: Vec<region::Region>,
  region_freelist: Vec<u32>,
  allocator: BlockAllocator,
}

impl Default for Context {
  /// Creates the default environment context
  ///
  /// Returns:
  ///     `Context`: The default context instance
  fn default() -> Self {
    Context {
      block_arena: Vec::new(),
      region_arena: Vec::new(),
      region_freelist: Vec::new(),
      allocator: BlockAllocator::new(),
    }
  }
}

impl Context {
  /// Creates a new empty environment context
  ///
  /// Returns:
  ///     `Context`: The newly created context instance
  pub fn new() -> Self {
    Self::default()
  }

  /// Allocates a block range of size from the freelist or arena
  fn alloc_block_range(&mut self, size: u32) -> Interval {
    self.allocator.alloc_block_range(size, &mut self.block_arena)
  }

  /// Allocates a contiguous run of blocks representing a region
  ///
  /// Args:
  ///     size (`u32`): The number of blocks requested for the region
  ///
  /// Returns:
  ///     `RegionId`: A handle to the allocated region
  pub fn region_alloc(&mut self, size: u32) -> RegionId {
    let interval = self.alloc_block_range(size);
    let region_id = if let Some(idx) = self.region_freelist.pop() {
      self.region_arena[idx as usize].intervals.push(interval);
      self.region_arena[idx as usize].active_interval_used = 0;
      RegionId(idx)
    } else {
      let idx = self.region_arena.len() as u32;
      self.region_arena.push(region::Region {
        intervals: vec![interval],
        bindings: HashMap::new(),
        active_interval_used: 0,
      });
      RegionId(idx)
    };

    if size > 0 {
      let idx = region_id.0 as usize;
      self.region_arena[idx].active_interval_used = 1;
      let root_block = interval.begin;
      debug_assert!(root_block.0 != 0, "allocator must never produce BlockId(0)");
      self.block_arena[(root_block.0 as usize) - 1].region = region_id;
    }

    region_id
  }

  /// Releases a region handle and returns its blocks to the freelist
  ///
  /// Args:
  ///     region_id (`RegionId`): The region handle to release
  pub fn region_free(&mut self, region_id: RegionId) {
    let idx = region_id.0 as usize;
    if idx < (self.region_arena.len()) {
      let intervals = std::mem::take(
        &mut self.region_arena[idx].intervals,
      );
      self.region_arena[idx].clear();

      for interval in intervals {
        if (interval.begin.0 != 0)
          && (interval.end.0 != 0)
          && ((interval.end.0) > (interval.begin.0))
        {
          self.allocator.block_freelist.push(interval);
        }
      }
      self.region_freelist.push(region_id.0);
    }
  }

  /// Allocates a new block within the given region, growing if needed
  ///
  /// Args:
  ///     region_id (`RegionId`): The region identifier to grow
  ///
  /// Returns:
  ///     `BlockId`: The allocated block identifier
  fn alloc_block_in_region(
    &mut self,
    region_id: RegionId,
  ) -> BlockId {
    let idx = region_id.0 as usize;
    {
      let region = &mut self.region_arena[idx];
      let has_space = if let Some(last_interval) = region.intervals.last()
      {
        let size = (last_interval.end.0) - (last_interval.begin.0);
        region.active_interval_used < size
      } else {
        false
      };

      if has_space {
        region.active_interval_used += 1;
      } else {
        let new_interval = self.alloc_block_range(1);
        let region = &mut self.region_arena[idx];
        region.intervals.push(new_interval);
        region.active_interval_used = 1;
      }
    }

    let region = &self.region_arena[idx];
    let offset = (region.active_interval_used) - 1;
    debug_assert!(
      !region.intervals.is_empty(),
      "alloc_block_in_region: region has no intervals; implementation error"
    );
    let begin_val = region.intervals.last().unwrap().begin.0;
    let new_block_id = BlockId(begin_val + offset);

    let block_idx = (new_block_id.0 as usize) - 1;
    if block_idx >= (self.block_arena.len()) {
      while (self.block_arena.len()) <= block_idx {
        self.block_arena.push(block::Block::default());
      }
    }
    self.block_arena[block_idx].region = region_id;

    new_block_id
  }

  /// Spawns an iterator starting at the region's begin block
  ///
  /// Args:
  ///     id (`RegionId`): The region identifier to traverse
  ///
  /// Returns:
  ///     `Option<Iter<'_>>`: The iterator cursor if valid
  pub fn iter(&self, id: RegionId) -> Option<Iter<'_>> {
    let idx = id.0 as usize;
    if idx < (self.region_arena.len()) {
      let r = &self.region_arena[idx];
      if let Some(first_interval) = r.intervals.first() {
        return Some(Iter {
          context: self,
          i: first_interval.begin,
        });
      }
    }
    None
  }

  /// Spawns a mutable iterator starting at the region's begin block
  ///
  /// Args:
  ///     id (`RegionId`): The region identifier to traverse
  ///
  /// Returns:
  ///     `Option<IterMut<'_>>`: The mutable iterator cursor if valid
  pub fn iter_mut(&mut self, id: RegionId) -> Option<IterMut<'_>> {
    let idx = id.0 as usize;
    if idx < (self.region_arena.len()) {
      let r = &self.region_arena[idx];
      if let Some(first_interval) = r.intervals.first() {
        let begin = first_interval.begin;
        return Some(IterMut {
          context: self,
          i: begin,
        });
      }
    }
    None
  }

  /// Allocates a child region nested under a parent block scope
  ///
  /// Args:
  ///     size (`u32`): The number of blocks requested for the region
  ///     parent (`BlockId`): The parent block scope identifier
  ///
  /// Returns:
  ///     `RegionId`: The allocated child region identifier
  fn region_alloc_child(
    &mut self,
    size: u32,
    parent: BlockId,
  ) -> RegionId {
    assert!(size > 0, "region_alloc_child: size must be > 0");
    let region_id = self.region_alloc(size);
    debug_assert!(
      !self.region_arena[region_id.0 as usize].intervals.is_empty(),
      "region_alloc_child: region has no intervals after alloc; implementation error"
    );
    let root = self.region_arena[region_id.0 as usize]
      .intervals
      .first()
      .unwrap()
      .begin;
    self.link_up(root, parent);

    if (parent.0 != 0)
      && ((parent.0 as usize) <= (self.block_arena.len()))
    {
      let p_idx = (parent.0 as usize) - 1;
      let down = self.block_arena[p_idx].down;
      if down.0 == 0 {
        self.link_down(parent, root);
      } else {
        let mut sib = down;
        loop {
          let sib_idx = (sib.0 as usize) - 1;
          let next = self.block_arena[sib_idx].next;
          if next.0 == 0 {
            self.link_next(sib, root);
            break;
          }
          sib = next;
        }
      }
    }
    region_id
  }

  /// Returns the size of the allocated region
  ///
  /// Args:
  ///     id (`RegionId`): The region identifier to inspect
  ///
  /// Returns:
  ///     `Option<u32>`: The total size of the region if valid
  pub fn region_size(&self, id: RegionId) -> Option<u32> {
    let idx = id.0 as usize;
    if idx < (self.region_arena.len()) {
      let r = &self.region_arena[idx];
      let mut total = 0;
      for interval in &r.intervals {
        total += (interval.end.0) - (interval.begin.0);
      }
      if total > 0 {
        return Some(total);
      }
    }
    None
  }

  /// Returns the total capacity of the backing block array
  ///
  /// Returns:
  ///     `usize`: The total number of allocated backing blocks
  pub fn blocks_capacity(&self) -> usize {
    self.block_arena.len()
  }

  /// Returns the region identifier of a block
  ///
  /// Args:
  ///     block (`BlockId`): The block identifier to inspect
  ///
  /// Returns:
  ///     `Option<RegionId>`: The owning region identifier if valid
  fn get_region_id_from_block(&self, block: BlockId) -> Option<RegionId> {
    if (block.0 != 0)
      && ((block.0 as usize) <= (self.block_arena.len()))
    {
      Some(self.block_arena[(block.0 as usize) - 1].region)
    } else {
      None
    }
  }

  /// Returns the number of items in the block freelist
  ///
  /// Returns:
  ///     `usize`: The length of the block freelist
  pub fn block_freelist_len(&self) -> usize {
    self.allocator.block_freelist.len()
  }

  /// Returns boundaries of a freed block interval at the given index
  ///
  /// Args:
  ///     idx (`usize`): The index into the block freelist
  ///
  /// Returns:
  ///     `Option<(BlockId, BlockId)>`: The boundaries of the interval
  fn block_freelist_interval(
    &self,
    idx: usize,
  ) -> Option<(BlockId, BlockId)> {
    self.allocator
      .block_freelist
      .get(idx)
      .map(|r| (r.begin, r.end))
  }

  /// Links a block to its parent scope
  ///
  /// Args:
  ///     block (`BlockId`): The block identifier to link
  ///     parent (`BlockId`): The parent block identifier
  fn link_up(&mut self, block: BlockId, parent: BlockId) {
    if ((block.0) != 0)
      && ((block.0 as usize) <= (self.block_arena.len()))
    {
      self.block_arena[(block.0 as usize) - 1].up = parent;
    }
  }

  /// Links a block to its first nested child scope
  ///
  /// Args:
  ///     block (`BlockId`): The parent block identifier
  ///     child (`BlockId`): The child block identifier
  fn link_down(&mut self, block: BlockId, child: BlockId) {
    if ((block.0) != 0)
      && ((block.0 as usize) <= (self.block_arena.len()))
    {
      self.block_arena[(block.0 as usize) - 1].down = child;
    }
  }

  /// Links a block to its next sibling scope
  ///
  /// Args:
  ///     block (`BlockId`): The block identifier to link
  ///     next (`BlockId`): The sibling block identifier
  fn link_next(&mut self, block: BlockId, next: BlockId) {
    if ((block.0) != 0)
      && ((block.0 as usize) <= (self.block_arena.len()))
    {
      self.block_arena[(block.0 as usize) - 1].next = next;
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verifies backing block capacity matches the allocated size
  #[test]
  fn test_backing_space_complexity_monotone_frontiers() {
    let mut context = Context::new();
    let r1 = context.region_alloc(10);
    assert_eq!(context.block_arena.len(), 10);
    context.region_free(r1);
  }

  /// Verifies `O(1)` swap-remove logic during freelist reclamation
  #[test]
  fn test_freelist_swap_remove_o1_complexity() {
    let mut context = Context::new();
    let r_a = context.region_alloc(5);
    let sep1 = context.region_alloc(1);
    let r_b = context.region_alloc(15);
    let sep2 = context.region_alloc(1);
    let r_c = context.region_alloc(25);

    context.region_free(r_a);
    context.region_free(r_b);
    context.region_free(r_c);

    assert_eq!(context.allocator.block_freelist.len(), 3);
    assert_eq!(context.allocator.block_freelist[0].begin.0, 1);
    assert_eq!(context.allocator.block_freelist[1].begin.0, 7);
    assert_eq!(context.allocator.block_freelist[2].begin.0, 23);

    let r_alloc = context.region_alloc(12);

    assert_eq!(context.allocator.block_freelist.len(), 3);
    assert_eq!(context.allocator.block_freelist[1].begin.0, 23);

    context.region_free(sep1);
    context.region_free(sep2);
    context.region_free(r_alloc);
  }

  /// Verifies default initialization and derives for handle types
  #[test]
  fn test_struct_derives_and_defaults() {
    assert_eq!(RegionId::default(), RegionId(0));
    assert_eq!(Symbol::default(), Symbol(0));
    assert_eq!(EntryId::default(), EntryId(0));
    assert_eq!(BlockId::default(), BlockId(0));
  }

  /// Verifies exact-match block range reuse from the freelist
  #[test]
  fn test_alloc_block_range_exact_match() {
    let mut context = Context::new();
    let r1 = context.region_alloc(10);
    context.region_free(r1);
    
    let r2 = context.region_alloc(10);
    assert_eq!(r2.0, 0);
  }

  /// Verifies partial-match block range splitting and remainder tracking
  #[test]
  fn test_alloc_block_range_partial_match() {
    let mut context = Context::new();
    let r1 = context.region_alloc(10);
    context.region_free(r1);
    
    let _r2 = context.region_alloc(6);
    assert_eq!(context.block_freelist_len(), 1);
    assert_eq!(context.block_freelist_interval(0).unwrap().0.0, 7);
    assert_eq!(context.block_freelist_interval(0).unwrap().1.0, 11);
  }

  /// Verifies region identifier recycling on allocation
  #[test]
  fn test_region_freelist_reuse() {
    let mut context = Context::new();
    let r1 = context.region_alloc(5);
    let _r2 = context.region_alloc(5);
    
    context.region_free(r1);
    let r3 = context.region_alloc(5);
    assert_eq!(r3, r1);
  }

  /// Verifies error handling when freeing non-existent region identifiers
  #[test]
  fn test_region_free_invalid_id() {
    let mut context = Context::new();
    context.region_free(RegionId(999));
  }

  /// Verifies iterator creation checks on out-of-bounds region handles
  #[test]
  fn test_iterators_invalid_region() {
    let context = Context::new();
    assert!(context.iter(RegionId(999)).is_none());
    
    let mut context_mut = Context::new();
    assert!(context_mut.iter_mut(RegionId(999)).is_none());
  }

  /// Verifies safety checks on invalid block or region identifiers
  #[test]
  fn test_introspection_invalid_inputs() {
    let context = Context::new();
    assert_eq!(context.region_size(RegionId(999)), None);
    assert_eq!(context.get_region_id_from_block(BlockId(999)), None);
    assert_eq!(context.get_region_id_from_block(BlockId(0)), None);
    assert_eq!(context.block_freelist_interval(999), None);
  }

  /// Verifies nested region linking when the parent block is invalid
  #[test]
  fn test_region_alloc_child_invalid_parent() {
    let mut context = Context::new();
    let _r = context.region_alloc_child(2, BlockId(0));
    assert_eq!(context.block_arena[0].up.0, 0);
    
    let r2 = context.region_alloc_child(2, BlockId(999));
    assert_eq!(context.get_region_id_from_block(BlockId(3)), Some(r2));
  }

  /// Verifies that `region_alloc_child` panics on zero size (caller contract)
  #[test]
  #[should_panic(expected = "size must be > 0")]
  fn test_region_alloc_child_zero_size_panics() {
    let mut context = Context::new();
    context.region_alloc_child(0, BlockId(0));
  }

  /// Verifies disjoint range allocation when active interval is full
  #[test]
  fn test_alloc_block_in_region_disjoint_allocation() {
    let mut context = Context::new();
    let r = context.region_alloc(2);
    let mut iter_mut = context.iter_mut(r).unwrap();
    
    iter_mut.push();
    
    iter_mut.push();
    assert_eq!(iter_mut.i.0, 3);
    assert_eq!(context.region_size(r), Some(3));
  }

  /// Verifies size introspection behavior on empty regions
  #[test]
  fn test_zero_sized_region_size_introspection() {
    let mut context = Context::new();
    let r = context.region_alloc(0);
    assert_eq!(context.region_size(r), None);
  }
}
