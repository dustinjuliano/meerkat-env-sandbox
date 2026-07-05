//! Block scope tree iterators and lexical resolution cursors

use super::{Context, BlockId, Symbol, EntryId};

/// Immutable iterator and cursor over the block scope tree
#[derive(Clone, Copy)]
pub struct Iter<'a> {
  pub(super) context: &'a Context,
  pub(super) i: BlockId,
}

impl<'a> Iter<'a> {
  /// Navigates to the parent block scope in-place
  ///
  /// Returns:
  ///     `Option<()>`: `Some(())` if successfully moved up, `None` if at root or invalid
  pub fn up(&mut self) -> Option<()> {
    if self.i.0 == 0 {
      return None;
    }
    let idx = (self.i.0 as usize) - 1;
    let parent = self.context.block_arena[idx].up;
    if parent.0 == 0 {
      None
    } else {
      self.i = parent;
      Some(())
    }
  }

  /// Navigates to the first child block scope in-place
  ///
  /// Returns:
  ///     `Option<()>`: `Some(())` if successfully moved down, `None` if no child exists
  pub fn down(&mut self) -> Option<()> {
    if self.i.0 == 0 {
      return None;
    }
    let idx = (self.i.0 as usize) - 1;
    let child = self.context.block_arena[idx].down;
    if child.0 == 0 {
      None
    } else {
      self.i = child;
      Some(())
    }
  }

  /// Resolves a symbol lexically by climbing parent scopes
  ///
  /// Args:
  ///     symbol (`Symbol`): The symbol identifier to find
  ///
  /// Returns:
  ///     `Option<EntryId>`: The resolved entry identifier if found
  pub fn find(&self, symbol: Symbol) -> Option<EntryId> {
    let mut curr = self.i;
    while curr.0 != 0 {
      let curr_val = curr.0;
      let block_idx = (curr_val as usize) - 1;
      let region_id = self.context.block_arena[block_idx].region;
      let region_idx = region_id.0 as usize;
      let region = &self.context.region_arena[region_idx];
      if let Some(&entry) = region.bindings.get(&(curr, symbol)) {
        return Some(entry);
      }
      curr = self.context.block_arena[block_idx].up;
    }
    None
  }

}

impl<'a> Iterator for Iter<'a> {
  type Item = Iter<'a>;

  /// Advances to the next sibling scope
  ///
  /// Returns:
  ///     `Option<Self::Item>`: The current iterator before advancing
  fn next(&mut self) -> Option<Self::Item> {
    if self.i.0 == 0 {
      return None;
    }
    let current = *self;
    let idx = (self.i.0 as usize) - 1;
    self.i = self.context.block_arena[idx].next;
    Some(current)
  }
}

/// Mutable iterator and cursor over the block scope tree
pub struct IterMut<'a> {
  pub(super) context: &'a mut Context,
  pub(super) i: BlockId,
}

impl<'a> IterMut<'a> {
  /// Navigates to the parent block scope in-place
  ///
  /// Returns:
  ///     `Option<()>`: `Some(())` if successfully moved up, `None` if at root or invalid
  pub fn up(&mut self) -> Option<()> {
    if self.i.0 == 0 {
      return None;
    }
    let idx = (self.i.0 as usize) - 1;
    let parent = self.context.block_arena[idx].up;
    if parent.0 == 0 {
      None
    } else {
      self.i = parent;
      Some(())
    }
  }

  /// Navigates to the first child block scope in-place
  ///
  /// Returns:
  ///     `Option<()>`: `Some(())` if successfully moved down, `None` if no child exists
  pub fn down(&mut self) -> Option<()> {
    if self.i.0 == 0 {
      return None;
    }
    let idx = (self.i.0 as usize) - 1;
    let child = self.context.block_arena[idx].down;
    if child.0 == 0 {
      None
    } else {
      self.i = child;
      Some(())
    }
  }

  /// Navigates to the next sibling block in-place
  ///
  /// Returns:
  ///     `bool`: `true` if advanced to next sibling, `false` otherwise
  pub fn step_sibling(&mut self) -> bool {
    if self.i.0 == 0 {
      return false;
    }
    let idx = (self.i.0 as usize) - 1;
    let next = self.context.block_arena[idx].next;
    if next.0 == 0 {
      false
    } else {
      self.i = next;
      true
    }
  }

  /// Allocates a new child block in the region and moves down to it
  pub fn push(&mut self) {
    if self.i.0 != 0 {
      let current = self.i;
      let block_idx = (current.0 as usize) - 1;
      let region_id = self.context.block_arena[block_idx].region;
      let new_block = self.context.alloc_block_in_region(region_id);
      let down = self.context.block_arena[block_idx].down;
      if down.0 == 0 {
        self.context.block_arena[block_idx].down = new_block;
      } else {
        let mut sib = down;
        loop {
          let sib_idx = (sib.0 as usize) - 1;
          let next = self.context.block_arena[sib_idx].next;
          if next.0 == 0 {
            self.context.block_arena[sib_idx].next = new_block;
            break;
          }
          sib = next;
        }
      }
      let new_idx = (new_block.0 as usize) - 1;
      self.context.block_arena[new_idx].up = current;
      self.i = new_block;
    }
  }

  /// Resolves a symbol lexically by climbing parent scopes
  ///
  /// Args:
  ///     symbol (`Symbol`): The symbol identifier to find
  ///
  /// Returns:
  ///     `Option<EntryId>`: The resolved entry identifier if found
  pub fn find(&self, symbol: Symbol) -> Option<EntryId> {
    let mut curr = self.i;
    while curr.0 != 0 {
      let curr_val = curr.0;
      let block_idx = (curr_val as usize) - 1;
      let region_id = self.context.block_arena[block_idx].region;
      let region_idx = region_id.0 as usize;
      let region = &self.context.region_arena[region_idx];
      if let Some(&entry) = region.bindings.get(&(curr, symbol)) {
        return Some(entry);
      }
      curr = self.context.block_arena[block_idx].up;
    }
    None
  }

  /// Binds a symbol to the current block with the given entry identifier
  ///
  /// Args:
  ///     symbol (`Symbol`): The symbol identifier to bind
  ///     entry (`EntryId`): The entry identifier to associate
  pub fn bind(&mut self, symbol: Symbol, entry: EntryId) {
    if self.i.0 != 0 {
      let block_idx = (self.i.0 as usize) - 1;
      let region_id = self.context.block_arena[block_idx].region;
      let region_idx = region_id.0 as usize;
      self.context.region_arena[region_idx]
        .bindings
        .insert((self.i, symbol), entry);
    }
  }

}

#[cfg(test)]
mod tests {
  use super::*;

  /// Verifies immutable navigation methods on `Iter`
  #[test]
  fn test_iter_immutable_navigation() {
    let mut context = Context::new();
    let r = context.region_alloc(3);
    
    let mut root = context.iter(r).unwrap();
    assert_eq!(root.i.0, 1);
    assert!(root.up().is_none());
    assert!(root.down().is_none());
  }

  /// Verifies lexical lookup resolution climbing parent scopes
  #[test]
  fn test_iter_lexical_find() {
    let mut context = Context::new();
    let r_a = context.region_alloc(2);
    let mut iter_mut_a = context.iter_mut(r_a).unwrap();
    iter_mut_a.push(); // creates 2
    iter_mut_a.bind(Symbol(42), EntryId(100));

    let r_b = context.region_alloc_child(2, BlockId(2));
    let iter_b = context.iter(r_b).unwrap();
    assert_eq!(iter_b.find(Symbol(42)), Some(EntryId(100)));
    assert_eq!(iter_b.find(Symbol(99)), None);
  }

  /// Verifies `Iterator` trait implementation behavior for sibling traversal
  #[test]
  fn test_iter_iterator_impl() {
    let mut context = Context::new();
    let r = context.region_alloc(3);
    
    let mut iter_mut = context.iter_mut(r).unwrap();
    iter_mut.push(); // creates 2
    iter_mut.up().unwrap();
    iter_mut.push(); // creates 3

    let root = context.iter(r).unwrap();
    let mut sibs = root;
    sibs.down().unwrap();
    let first = sibs.next().unwrap();
    assert_eq!(first.i.0, 2);
    let second = sibs.next().unwrap();
    assert_eq!(second.i.0, 3);
    assert!(sibs.next().is_none());
  }

  /// Verifies mutable parent/child navigation and child insertion
  #[test]
  fn test_iter_mut_navigation_and_push() {
    let mut context = Context::new();
    let r = context.region_alloc(3);
    
    let mut iter_mut = context.iter_mut(r).unwrap();
    iter_mut.push(); // creates 2
    assert_eq!(iter_mut.i.0, 2);

    iter_mut.up().unwrap();
    assert_eq!(iter_mut.i.0, 1);
    
    iter_mut.down().unwrap();
    assert_eq!(iter_mut.i.0, 2);
  }

  /// Verifies `step_sibling` functionality on `IterMut`
  #[test]
  fn test_iter_mut_sibling_step() {
    let mut context = Context::new();
    let r = context.region_alloc(3);
    
    let mut iter_mut = context.iter_mut(r).unwrap();
    iter_mut.push(); // creates 2
    iter_mut.up().unwrap();
    iter_mut.push(); // creates 3

    let mut sibs = context.iter_mut(r).unwrap();
    sibs.down().unwrap();
    assert_eq!(sibs.i.0, 2);
    assert!(sibs.step_sibling());
    assert_eq!(sibs.i.0, 3);
    assert!(!sibs.step_sibling());
  }

  /// Verifies `bind` method and symbol binding mutations on `IterMut`
  #[test]
  fn test_iter_mut_binding() {
    let mut context = Context::new();
    let r = context.region_alloc(1);
    
    let mut iter_mut = context.iter_mut(r).unwrap();
    iter_mut.bind(Symbol(5), EntryId(50));
    assert_eq!(iter_mut.find(Symbol(5)), Some(EntryId(50)));
  }

  /// Verifies correct sentinel handlings for iterators at block `0`
  #[test]
  fn test_iter_invalid_handling() {
    let context = Context::new();
    let mut invalid_iter = Iter {
      context: &context,
      i: BlockId(0),
    };
    assert!(invalid_iter.up().is_none());
    assert!(invalid_iter.down().is_none());

    let mut context_mut = Context::new();
    let mut invalid_iter_mut1 = IterMut {
      context: &mut context_mut,
      i: BlockId(0),
    };
    assert!(invalid_iter_mut1.up().is_none());

    let mut invalid_iter_mut2 = IterMut {
      context: &mut context_mut,
      i: BlockId(0),
    };
    assert!(invalid_iter_mut2.down().is_none());

    let mut invalid_iter_mut3 = IterMut {
      context: &mut context_mut,
      i: BlockId(0),
    };
    assert!(!invalid_iter_mut3.step_sibling());
    invalid_iter_mut3.push();
    invalid_iter_mut3.bind(Symbol(1), EntryId(1));
  }
}

