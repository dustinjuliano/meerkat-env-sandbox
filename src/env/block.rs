//! Block structural node and parent/child linkage implementations


/// Unique identifier for blocks
///
/// Lightweight type-safe wrapper around a `u32` value
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// Range of blocks in the freelist
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Interval {
  pub(super) begin: BlockId,
  pub(super) end: BlockId,
}

/// Node representing hierarchical scope relationships
///
/// Holds parent, child, and sibling references along with `RegionId`
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Block {
  pub(super) up: BlockId,
  pub(super) down: BlockId,
  pub(super) next: BlockId,
  pub(super) region: super::RegionId,
}



#[cfg(test)]
mod tests {
  use super::*;
  use crate::env::Context;

  /// Verifies default initialization and derives for block structures
  #[test]
  fn test_block_defaults_and_derives() {
    let bid = BlockId::default();
    assert_eq!(bid.0, 0);

    let iv = Interval::default();
    assert_eq!(iv.begin.0, 0);
    assert_eq!(iv.end.0, 0);

    let block = Block::default();
    assert_eq!(block.up.0, 0);
    assert_eq!(block.down.0, 0);
    assert_eq!(block.next.0, 0);
    assert_eq!(block.region.0, 0);
  }

  /// Verifies parent linkage updates for blocks
  #[test]
  fn test_context_link_up() {
    let mut context = Context::new();
    let r = context.region_alloc(2);
    
    // Parent link block 1 to block 2
    context.link_up(BlockId(1), BlockId(2));
    let iter = context.iter(r).unwrap();
    let parent = iter.up();
    assert_eq!(parent.unwrap().block_id().0, 2);
  }

  /// Verifies child linkage updates for blocks
  #[test]
  fn test_context_link_down() {
    let mut context = Context::new();
    let r = context.region_alloc(2);
    
    // Link parent 1 to child 2
    context.link_down(BlockId(1), BlockId(2));
    let iter = context.iter(r).unwrap();
    let child = iter.down();
    assert_eq!(child.unwrap().block_id().0, 2);
  }

  /// Verifies sibling linkage updates for blocks
  #[test]
  fn test_context_link_next() {
    let mut context = Context::new();
    let r = context.region_alloc(3);
    
    // Link sibling 1 to 2
    context.link_next(BlockId(1), BlockId(2));
    let mut iter = context.iter(r).unwrap();
    let _ = iter.next(); // Consume block 1
    assert_eq!(iter.block_id().0, 2);
  }
}

