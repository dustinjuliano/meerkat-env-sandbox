//! Environment cursor representing a position in the block scope tree
//!
//! Holds a reference-free identifier to avoid borrow locking the context.

use super::block::BlockId;

/// Lightweight, copyable position handle in the environment context
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
  pub(super) i: BlockId,
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verifies default constructor produces a Cursor pointing to BlockId(0)
  #[test]
  fn test_cursor_default() {
    let cursor = Cursor::default();
    assert_eq!(cursor.i.0, 0);
  }

  /// Verifies equality, cloning, and copying properties of Cursor
  #[test]
  fn test_cursor_equality_and_copy() {
    let c1 = Cursor { i: BlockId(5) };
    let c2 = c1; // Test Copy
    let c3 = c1.clone(); // Test Clone
    
    assert_eq!(c1, c2);
    assert_eq!(c1, c3);

    let c4 = Cursor { i: BlockId(10) };
    assert_ne!(c1, c4);
  }

  /// Verifies Debug representation format
  #[test]
  fn test_cursor_debug_format() {
    let cursor = Cursor { i: BlockId(42) };
    let debug_str = format!("{:?}", cursor);
    assert!(debug_str.contains("Cursor"));
    assert!(debug_str.contains("i: BlockId(42)"));
  }
}
