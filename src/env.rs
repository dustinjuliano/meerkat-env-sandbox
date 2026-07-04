//! Environment system

use std::rc::Rc;

pub mod region;
pub use self::region::Region;

/// The maximum block identifier allowed due to half-open ranges
pub const MAX_BLOCK_ID: u32 = u32::MAX - 1;

/// A unique identifier for blocks
///
/// This is a lightweight type-safe wrapper around a `u32` value
#[derive(Clone, Copy, Debug, Default)]
struct BlockId(u32);

/// A node in the graph representing structure relationships
///
/// Each block stores references to parent, child, and sibling
/// block identifiers
#[derive(Default)]
struct Block {
  up: BlockId,
  down: BlockId,
  next: BlockId,
}

/// The execution context managing block allocations and tracking
///
/// This holds the backing blocks list, the next block identifier
/// generator, and the list of freed block regions available for
/// reuse
pub struct Context {
  blocks: Vec<Block>,
  block_next_id: BlockId,
  block_freelist: Vec<Region>,
}

impl Default for Context {
  /// Creates the default environment context
  ///
  /// Returns:
  ///     `Context`: The default context instance
  fn default() -> Self {
    Context {
      blocks: Vec::new(),
      // Note the use of `1` and not `0` because of the sentinel value which
      // must always be upheld
      block_next_id: BlockId(1),
      block_freelist: Vec::new(),
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

  /// Allocates a contiguous run of blocks representing a region
  ///
  /// This method first checks the freelist for a suitable free region.
  /// If one is found, it is reused (and split if it was larger than the
  /// requested size). If not, new blocks are allocated at the end of
  /// the backing block list
  ///
  /// Args:
  ///     size (`u32`): The number of blocks requested for the region
  ///
  /// Returns:
  ///     `Rc<Region>`: A shared pointer to the allocated `Region` handle
  pub fn region_alloc(&mut self, size: u32) -> Rc<Region> {
    // Check the freelist first
    for i in 0..self.block_freelist.len() {
      let r = self.block_freelist[i];
      let r_size = r.end.0 - r.begin.0;
      if r_size >= size {
        self.block_freelist.swap_remove(i);
        if r_size > size {
          self.block_freelist.push(Region {
            begin: BlockId(r.begin.0 + size),
            end: r.end,
          });
        }
        return Rc::new(Region {
          begin: r.begin,
          end: BlockId(r.begin.0 + size),
        });
      }
    }

    // No suitable free regions; allocate new blocks
    let begin = self.block_next_id;
    let end = BlockId(begin.0 + size);
    self.block_next_id = end;

    while self.blocks.len() < (end.0 as usize).saturating_sub(1) {
      self.blocks.push(Block::default());
    }

    Rc::new(Region { begin, end })
  }

  /// Releases a region handle and returns its blocks to the freelist
  ///
  /// The freed block range is registered back into the internal
  /// freelist so it can be reused by future allocation calls
  ///
  /// Args:
  ///     region (`Rc<Region>`): The region handle to release
  pub fn region_free(&mut self, region: Rc<Region>) {
    self.block_freelist.push(*region);
  }

  /// Returns the current total capacity of the backing block array
  ///
  /// This reports the high-water mark of allocated blocks and is
  /// used to verify memory monotonicity
  ///
  /// Returns:
  ///     `usize`: The total number of active/allocated backing blocks
  pub fn blocks_capacity(&self) -> usize {
    self.blocks.len()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verifies the `-1` space complexity bounds optimization on the backing array
  #[test]
  fn test_backing_space_complexity_monotone_frontiers() {
    let mut context = Context::new();

    // Verify space complexity: allocating size `S` resizes `blocks`
    // to exactly `S` (eliminating the previous `+ 1` overhead)
    let r1 = context.region_alloc(10);
    assert_eq!(context.blocks.len(), 10, "Backing blocks length must be exactly equal to the allocated size");
    context.region_free(r1);
  }

  /// Verifies the `O(1)` time complexity of freelist deletion using `swap_remove`
  #[test]
  fn test_freelist_swap_remove_o1_complexity() {
    let mut context = Context::new();

    // Populate the freelist with three regions of different sizes
    let r_a = context.region_alloc(5);
    let sep1 = context.region_alloc(1);
    let r_b = context.region_alloc(15);
    let sep2 = context.region_alloc(1);
    let r_c = context.region_alloc(25);

    context.region_free(r_a);
    context.region_free(r_b);
    context.region_free(r_c);

    // Assert freelist initial state is sorted by free order: [r_a, r_b, r_c]
    assert_eq!(context.block_freelist.len(), 3);
    assert_eq!(context.block_freelist[0].begin.0, 1);
    assert_eq!(context.block_freelist[1].begin.0, 7);
    assert_eq!(context.block_freelist[2].begin.0, 23);

    // Request size 12. This should match and split `r_b` (index 1).
    // The `swap_remove(1)` should swap `r_b` with the last element `r_c`
    // (index 2), then pop `r_b`.
    // The freelist should become: [r_a, r_c, split_remainder]
    let r_alloc = context.region_alloc(12);

    assert_eq!(context.block_freelist.len(), 3);
    // Index 1 must now contain `r_c` (proving swap_remove took place in O(1) time)
    assert_eq!(context.block_freelist[1].begin.0, 23, "Last element must swap into the deleted index");

    // Clean up
    context.region_free(sep1);
    context.region_free(sep2);
    context.region_free(r_alloc);
  }
}
