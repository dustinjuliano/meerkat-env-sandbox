//! Integration tests for scope tree iteration and lexical resolution

use env::env::{Context, Symbol, EntryId, BlockId};

/// Verifies sibling scope traversal using LCRS links
#[test]
fn test_sibling_iteration() {
  let mut context = Context::new();
  
  // Allocate region
  let r = context.region_alloc(3);
  
  // Create sibling structure under block `1`
  // `1` -> child `2`, sibling `3`
  let mut iter_mut = context.iter_mut(r).unwrap(); // at `1`
  iter_mut.push(); // creates `2`, moves to `2`
  
  let mut iter_mut = iter_mut.up().unwrap(); // at `1`
  iter_mut.push(); // creates `3`, moves to `3`
  
  // Traverse sibling chain under `1` starting at `2`
  let root = context.iter(r).unwrap(); // at `1`
  let mut sibs = root.down().unwrap(); // at `2`
  
  let mut count = 0;
  let expected = [2, 3];
  
  while let Some(current) = sibs.next() {
    assert_eq!(current.block_id().0, expected[count]);
    count += 1;
  }
  assert_eq!(count, 2);
}

/// Verifies parent/child scope traversal using `up` and `down`
#[test]
fn test_parent_child_traversal() {
  let mut context = Context::new();
  
  let r = context.region_alloc(2);
  let mut iter_mut = context.iter_mut(r).unwrap(); // at `1`
  iter_mut.push(); // creates `2`, moves to `2`
  
  let iter2 = iter_mut.as_readonly();
  assert_eq!(iter2.block_id().0, 2);
  
  let iter1 = iter2.up().unwrap();
  assert_eq!(iter1.block_id().0, 1);
  
  let child = iter1.down().unwrap();
  assert_eq!(child.block_id().0, 2);
}

/// Verifies parent/child scope traversal across region boundaries
#[test]
fn test_boundary_crossing() {
  let mut context = Context::new();
  
  let r_a = context.region_alloc(2); // Region `A` has block `1`
  let mut iter_mut_a = context.iter_mut(r_a).unwrap();
  iter_mut_a.push(); // creates block `2`
  
  // Allocate Region `B` as a child of block `2`
  let r_b = context.region_alloc_child(2, BlockId(2)); // Region `B` starts at block `3`
  
  let iter_b = context.iter(r_b).unwrap(); // starts at block `3`
  assert_eq!(iter_b.block_id().0, 3);
  assert_eq!(iter_b.region_id(), r_b);
  assert_eq!(context.block_region(BlockId(3)), Some(r_b));
  
  // Climb parent link
  let iter_a = iter_b.up().unwrap();
  assert_eq!(iter_a.block_id().0, 2);
  assert_eq!(iter_a.region_id(), r_a);
}

/// Verifies symbol resolution climbs boundaries to find parent bindings
#[test]
fn test_lexical_resolution_and_binding() {
  let mut context = Context::new();
  
  let r_a = context.region_alloc(2);
  let mut iter_mut_a = context.iter_mut(r_a).unwrap();
  iter_mut_a.push(); // block `2`
  
  let r_b = context.region_alloc_child(2, BlockId(2)); // block `3`
  
  // Bind symbol to block `1` in region `A`
  let mut iter_mut_a = context.iter_mut(r_a).unwrap(); // block `1`
  iter_mut_a.bind(Symbol(42), EntryId(100));
  
  // Query lexical resolution from block `3` in region `B`
  let iter_b = context.iter(r_b).unwrap(); // block `3`
  let entry = iter_b.find(Symbol(42));
  
  assert_eq!(entry, Some(EntryId(100)));
  assert_eq!(iter_b.find(Symbol(99)), None);
}

/// Verifies region recycling and size tracking behaviors
#[test]
fn test_region_recycling_and_size() {
  let mut context = Context::new();
  
  let r1 = context.region_alloc(5);
  assert_eq!(context.region_size(r1), Some(5));
  
  let mut iter_mut = context.iter_mut(r1).unwrap();
  iter_mut.bind(Symbol(1), EntryId(10));
  
  // Free region
  context.region_free(r1);
  assert_eq!(context.region_size(r1), None);
  
  // Allocate new region of same size (should reuse `r1`'s index)
  let r2 = context.region_alloc(5);
  assert_eq!(r2, r1, "Should recycle RegionId");
  assert_eq!(context.region_size(r2), Some(5));
  
  // Ensure bindings were cleared during recycling
  let iter_r2 = context.iter(r2).unwrap();
  assert_eq!(iter_r2.find(Symbol(1)), None, "Bindings should have been cleared");
}
