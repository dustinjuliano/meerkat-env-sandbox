//! Submodule defining region structures
//!
//! A region is used to track a contiguous sequence of blocks, which
//! is exploited for allocation and deallocation purposes by observing
//! that the block ids are allocated contiguously due to the nature
//! of static lexical scoping

use super::BlockId;

/// A range of contiguous blocks represented by `[begin, end)`
///
/// This struct is a lightweight copyable handle that defines the
/// boundaries of the allocated blocks
#[derive(Clone, Copy, Debug, Default)]
pub struct Region {
  pub(super) begin: BlockId,
  pub(super) end: BlockId,
}
