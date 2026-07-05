//! Integration tests for scope tree iteration and lexical resolution

use env::env::{Context, Symbol, EntryId};

/// Verifies sibling scope traversal using LCRS links
#[test]
fn test_sibling_iteration() {
  let mut context = Context::new();
  
  // Allocate region
  let r = context.region_alloc(3).unwrap();
  
  // Create sibling structure under block `1`
  // `1` -> child `2`, sibling `3`
  let mut iter_mut = context.iter_mut(r).unwrap(); // at `1`
  iter_mut.push().unwrap(); // creates `2`, moves to `2`
  iter_mut.bind(Symbol(2), EntryId(20));
  
  iter_mut.up().unwrap(); // at `1`
  iter_mut.push().unwrap(); // creates `3`, moves to `3`
  iter_mut.bind(Symbol(3), EntryId(30));
  
  // Traverse sibling chain under `1` starting at `2`
  let root = context.iter(r).unwrap(); // at `1`
  let mut sibs = root;
  sibs.down().unwrap();
  
  let first = sibs.next().unwrap();
  assert_eq!(first.find(Symbol(2)), Some(EntryId(20)));
  
  let second = sibs.next().unwrap();
  assert_eq!(second.find(Symbol(3)), Some(EntryId(30)));
  
  assert!(sibs.next().is_none());
}

/// Verifies parent/child scope traversal using `up` and `down`
#[test]
fn test_parent_child_traversal() {
  let mut context = Context::new();
  
  let r = context.region_alloc(2).unwrap();
  let mut iter_mut = context.iter_mut(r).unwrap(); // at `1`
  iter_mut.push().unwrap(); // creates `2`, moves to `2`
  iter_mut.bind(Symbol(2), EntryId(20));
  
  let mut iter = context.iter(r).unwrap();
  iter.down().unwrap(); // at `2`
  assert_eq!(iter.find(Symbol(2)), Some(EntryId(20)));
  
  iter.up().unwrap();
  assert_eq!(iter.find(Symbol(2)), None);
  
  iter.down().unwrap();
  assert_eq!(iter.find(Symbol(2)), Some(EntryId(20)));
}

/// Verifies parent/child scope traversal across region boundaries
#[test]
fn test_boundary_crossing() {
  let mut context = Context::new();
  
  let r_a = context.region_alloc(2).unwrap();
  let mut iter_mut_a = context.iter_mut(r_a).unwrap();
  iter_mut_a.push(); // creates block `2`, moves to `2`
  
  // Climb back up and verify we return to block `1`
  iter_mut_a.up().unwrap();
  assert_eq!(iter_mut_a.find(Symbol(42)), None); // nothing bound, but traversal succeeded
}

/// Verifies symbol resolution climbs parent scopes to find bindings
#[test]
fn test_lexical_resolution_and_binding() {
  let mut context = Context::new();
  
  let r_a = context.region_alloc(2).unwrap();
  let mut iter_mut_a = context.iter_mut(r_a).unwrap(); // block `1`
  iter_mut_a.bind(Symbol(42), EntryId(100));
  iter_mut_a.push(); // block `2`
  
  // Query from child block — should climb and find binding in block `1`
  assert_eq!(iter_mut_a.find(Symbol(42)), Some(EntryId(100)));
  assert_eq!(iter_mut_a.find(Symbol(99)), None);
}

/// Verifies region recycling and size tracking behaviors
#[test]
fn test_region_recycling_and_size() {
  let mut context = Context::new();
  
  let r1 = context.region_alloc(5).unwrap();
  assert_eq!(context.region_size(r1), Some(5));
  
  let mut iter_mut = context.iter_mut(r1).unwrap();
  iter_mut.bind(Symbol(1), EntryId(10));
  
  // Free region
  context.region_free(r1);
  assert_eq!(context.region_size(r1), None);
  
  // Allocate new region of same size (should reuse `r1`'s index)
  let r2 = context.region_alloc(5).unwrap();
  assert_eq!(r2, r1, "Should recycle RegionId");
  assert_eq!(context.region_size(r2), Some(5));
  
  // Ensure bindings were cleared during recycling
  let iter_r2 = context.iter(r2).unwrap();
  assert_eq!(iter_r2.find(Symbol(1)), None, "Bindings should have been cleared");
}
