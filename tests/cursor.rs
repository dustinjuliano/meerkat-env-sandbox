//! Integration tests for the Context-centric Cursor API and scope tree traversal

use env::env::{Context, Symbol, EntryId, RegionId, Cursor};

/// Verifies cursor creation and error handling for invalid region handles
#[test]
fn test_cursor_creation() {
  let context = Context::new();
  assert!(context.cursor(RegionId(999)).is_none());
}

/// Verifies parent, child, and sibling scope traversal using up, down, and next
#[test]
fn test_cursor_traversal() {
  let mut context = Context::new();
  let r = context.region_alloc(5).unwrap();
  
  let mut cursor = context.cursor(r).unwrap();
  context.bind(cursor, Symbol(1), EntryId(10));
  
  // No parent, child, or sibling at start
  assert!(context.up(&mut cursor).is_none());
  assert!(context.down(&mut cursor).is_none());
  assert!(context.next(&mut cursor).is_none());

  // Push block 2 (child of 1)
  context.push_block(&mut cursor).unwrap();
  context.bind(cursor, Symbol(2), EntryId(20));

  // Return to 1, then push block 3 (second child of 1, sibling to 2)
  context.up(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(2)), None);
  
  context.push_block(&mut cursor).unwrap();
  context.bind(cursor, Symbol(3), EntryId(30));

  // Move back up to parent (1)
  context.up(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(3)), None);

  // Move down to first child (2)
  context.down(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(2)), Some(EntryId(20)));
  assert_eq!(context.find(cursor, Symbol(3)), None);

  // Move next to sibling (3)
  context.next(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(3)), Some(EntryId(30)));
  assert_eq!(context.find(cursor, Symbol(2)), None);

  // No more siblings
  assert!(context.next(&mut cursor).is_none());
}

/// Verifies lexical lookup resolution climbing parent scopes to find bindings
#[test]
fn test_cursor_lexical_find_and_bind() {
  let mut context = Context::new();
  let r = context.region_alloc(3).unwrap();
  let mut cursor = context.cursor(r).unwrap(); // Block 1

  context.bind(cursor, Symbol(42), EntryId(100));
  context.push_block(&mut cursor).unwrap(); // Block 2

  // Query from child block (2) — should climb and find binding in block 1
  assert_eq!(context.find(cursor, Symbol(42)), Some(EntryId(100)));
  assert_eq!(context.find(cursor, Symbol(99)), None);
}

/// Verifies region nesting, dynamic sizing, and cross-region boundary resolution
#[test]
fn test_push_region_and_lexical_continuity() {
  let mut context = Context::new();
  
  // Allocate parent region
  let r_parent = context.region_alloc(2).unwrap();
  let mut cursor = context.cursor(r_parent).unwrap(); // Block 1 in region 0
  context.bind(cursor, Symbol(10), EntryId(1000));
  
  context.push_block(&mut cursor).unwrap(); // Block 2 in region 0
  context.bind(cursor, Symbol(20), EntryId(2000));

  // Nest a new region (region 1) under Block 2
  let r_child = context.push_region(&mut cursor).unwrap(); // Block 3 in region 1
  assert_eq!(r_child.0, 1);
  context.bind(cursor, Symbol(30), EntryId(3000));

  // Symbol resolution from nested region should climb boundaries to parent region
  assert_eq!(context.find(cursor, Symbol(30)), Some(EntryId(3000)));
  assert_eq!(context.find(cursor, Symbol(20)), Some(EntryId(2000)));
  assert_eq!(context.find(cursor, Symbol(10)), Some(EntryId(1000)));
  assert_eq!(context.find(cursor, Symbol(99)), None);

  // Navigating up should cross the region boundary seamlessly
  context.up(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(30)), None);
  assert_eq!(context.find(cursor, Symbol(20)), Some(EntryId(2000)));
}

/// Verifies that traversal and push operations return None or handle invalid cursors gracefully
#[test]
fn test_invalid_cursor_handling() {
  let context = Context::new();
  let mut invalid_cursor = Cursor::default();

  assert!(context.up(&mut invalid_cursor).is_none());
  assert!(context.down(&mut invalid_cursor).is_none());
  assert!(context.next(&mut invalid_cursor).is_none());
  assert_eq!(context.find(invalid_cursor, Symbol(1)), None);

  let mut context_mut = Context::new();
  // Attempting to push on an invalid cursor should return None
  assert!(context_mut.push_block(&mut invalid_cursor).is_none());
  assert!(context_mut.push_region(&mut invalid_cursor).is_none());
  
  // bind on invalid cursor does nothing
  context_mut.bind(invalid_cursor, Symbol(1), EntryId(1));
}

/// Verifies that multiple blocks can be dynamically pushed in a region,
/// and that the internally coalesced region works seamlessly.
#[test]
fn test_region_dynamic_growth_and_coalescing_integration() {
  let mut context = Context::new();
  let r = context.region_alloc(2).unwrap();
  let mut cursor = context.cursor(r).unwrap();

  context.bind(cursor, Symbol(1), EntryId(10));

  // Push beyond initial capacity (2) to trigger allocation growth
  for i in 2..=10 {
    context.push_block(&mut cursor).unwrap();
    context.bind(cursor, Symbol(i), EntryId(i * 10));
  }

  // Verify that all sequential scopes can resolve their respective bindings
  for i in 1..=10 {
    assert_eq!(context.find(cursor, Symbol(i)), Some(EntryId(i * 10)));
  }

  // Verify that coalescing successfully kept the number of intervals minimal (1)
  assert_eq!(context.region_size(r), Some(10));
}

/// Verifies name resolution climbs recursively across three levels of nested regions
#[test]
fn test_multi_level_region_resolution() {
  let mut context = Context::new();
  let r_a = context.region_alloc(1).unwrap();
  let mut cursor = context.cursor(r_a).unwrap();
  context.bind(cursor, Symbol(1), EntryId(10));

  let _r_b = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(2), EntryId(20));

  let _r_c = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(3), EntryId(30));

  // From the deepest region, we should resolve all symbols up the chain
  assert_eq!(context.find(cursor, Symbol(3)), Some(EntryId(30)));
  assert_eq!(context.find(cursor, Symbol(2)), Some(EntryId(20)));
  assert_eq!(context.find(cursor, Symbol(1)), Some(EntryId(10)));
}

/// Verifies that variables in inner regions shadow variables in outer regions,
/// and that navigating up restores the outer binding visibility.
#[test]
fn test_shadowing_across_region_boundaries() {
  let mut context = Context::new();
  let r_outer = context.region_alloc(1).unwrap();
  let mut cursor = context.cursor(r_outer).unwrap();
  context.bind(cursor, Symbol(42), EntryId(100));

  // Push inner region with shadowing binding
  let _r_inner = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(42), EntryId(200));

  // Shadowed value visible in inner region
  assert_eq!(context.find(cursor, Symbol(42)), Some(EntryId(200)));

  // Return to outer region — outer value restored
  context.up(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(42)), Some(EntryId(100)));
}

/// Verifies that two sibling regions nested under the same block are isolated from each other
#[test]
fn test_sibling_regions_isolation() {
  let mut context = Context::new();
  let r_parent = context.region_alloc(1).unwrap();
  let mut cursor = context.cursor(r_parent).unwrap(); // Parent block

  // Push first sibling region
  let _r_sib1 = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(1), EntryId(10));

  // Go back to parent block, push second sibling region
  context.up(&mut cursor).unwrap();
  let _r_sib2 = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(2), EntryId(20));

  // Cursor is currently at sibling 2. Should resolve Symbol(2) but NOT Symbol(1)
  assert_eq!(context.find(cursor, Symbol(2)), Some(EntryId(20)));
  assert_eq!(context.find(cursor, Symbol(1)), None);

  // Navigate to sibling 1. Should resolve Symbol(1) but NOT Symbol(2)
  context.up(&mut cursor).unwrap(); // Back to parent block
  context.down(&mut cursor).unwrap(); // Goes to sibling 1 (first child)
  assert_eq!(context.find(cursor, Symbol(1)), Some(EntryId(10)));
  assert_eq!(context.find(cursor, Symbol(2)), None);
}

/// Verifies stale cursor safety: after freeing a region, any existing cursor
/// pointing to its blocks can no longer resolve bindings, and new cursors cannot be created.
#[test]
fn test_stale_cursor_behavior_after_region_free() {
  let mut context = Context::new();
  let r = context.region_alloc(2).unwrap();
  let cursor = context.cursor(r).unwrap();
  context.bind(cursor, Symbol(42), EntryId(100));

  // Free the region
  context.region_free(r);

  // Future cursor requests must return None
  assert!(context.cursor(r).is_none());

  // Existing cursor can no longer resolve the binding (its region bindings were cleared)
  assert_eq!(context.find(cursor, Symbol(42)), None);
}

/// Verifies that lookups climb the correct branch of a scope tree and do not leak into sibling branches.
#[test]
fn test_lexical_tree_climbing_branch_isolation() {
  let mut context = Context::new();
  let r = context.region_alloc(5).unwrap();
  let mut cursor = context.cursor(r).unwrap(); // Block 1
  context.bind(cursor, Symbol(10), EntryId(100));

  // Branch A: Push child block 2
  context.push_block(&mut cursor).unwrap(); // Block 2
  context.bind(cursor, Symbol(20), EntryId(200));

  // Back to parent
  context.up(&mut cursor).unwrap(); // Block 1

  // Branch B: Push child block 3
  context.push_block(&mut cursor).unwrap(); // Block 3
  context.bind(cursor, Symbol(30), EntryId(300));

  // Query from Block 3: should resolve parent Symbol(10) and local Symbol(30), but NOT sibling Symbol(20)
  assert_eq!(context.find(cursor, Symbol(30)), Some(EntryId(300)));
  assert_eq!(context.find(cursor, Symbol(10)), Some(EntryId(100)));
  assert_eq!(context.find(cursor, Symbol(20)), None);
}

/// Verifies name resolution climbs past intermediate blocks that have no symbol bindings.
#[test]
fn test_climbing_past_empty_scopes() {
  let mut context = Context::new();
  let r = context.region_alloc(5).unwrap();
  let mut cursor = context.cursor(r).unwrap(); // Block 1
  context.bind(cursor, Symbol(10), EntryId(100));

  context.push_block(&mut cursor).unwrap(); // Block 2 (empty)
  context.push_block(&mut cursor).unwrap(); // Block 3 (empty)
  context.push_block(&mut cursor).unwrap(); // Block 4
  context.bind(cursor, Symbol(40), EntryId(400));

  // Query from Block 4: should find Symbol(10) in Block 1 and Symbol(40) in Block 4
  assert_eq!(context.find(cursor, Symbol(40)), Some(EntryId(400)));
  assert_eq!(context.find(cursor, Symbol(10)), Some(EntryId(100)));
}

/// Verifies region recycling and resource isolation: newly recycled regions must not inherit old bindings.
#[test]
fn test_region_id_recycling_isolation() {
  let mut context = Context::new();
  
  // Allocate and populate region A
  let r_a = context.region_alloc(2).unwrap();
  let cursor_a = context.cursor(r_a).unwrap();
  context.bind(cursor_a, Symbol(42), EntryId(100));
  context.region_free(r_a);

  // Allocate region B (which recycles region A's ID)
  let r_b = context.region_alloc(2).unwrap();
  assert_eq!(r_a, r_b);

  // Cursor on recycled ID should resolve nothing initially
  let cursor_b = context.cursor(r_b).unwrap();
  assert_eq!(context.find(cursor_b, Symbol(42)), None);
}

/// Verifies that multiple cursors can traverse and read the same context independently.
#[test]
fn test_multiple_coexisting_cursors() {
  let mut context = Context::new();
  let r = context.region_alloc(5).unwrap();
  
  let mut cursor_a = context.cursor(r).unwrap(); // Root position
  let cursor_b = cursor_a; // Copy of root position

  // Mutate via cursor_a (push and bind)
  context.push_block(&mut cursor_a).unwrap();
  context.bind(cursor_a, Symbol(1), EntryId(10));

  // Cursor B (at root) cannot see the child's local binding
  assert_eq!(context.find(cursor_b, Symbol(1)), None);

  // But if Cursor B moves down, it can now resolve the binding
  let mut cursor_b_moved = cursor_b;
  context.down(&mut cursor_b_moved).unwrap();
  assert_eq!(context.find(cursor_b_moved, Symbol(1)), Some(EntryId(10)));
}

/// Verifies that sibling blocks cannot resolve symbols horizontally from each other.
#[test]
fn test_sibling_scope_horizontal_isolation() {
  let mut context = Context::new();
  let r = context.region_alloc(5).unwrap();
  let mut cursor = context.cursor(r).unwrap(); // Parent Block 1

  context.push_block(&mut cursor).unwrap(); // Block 2 (first child)
  context.bind(cursor, Symbol(2), EntryId(20));

  context.up(&mut cursor).unwrap(); // Back to parent
  
  context.push_block(&mut cursor).unwrap(); // Block 3 (second child, sibling to 2)
  context.bind(cursor, Symbol(3), EntryId(30));

  // Sibling 2 cannot see Sibling 3's bindings, and vice versa
  assert_eq!(context.find(cursor, Symbol(3)), Some(EntryId(30)));
  assert_eq!(context.find(cursor, Symbol(2)), None);
}

/// Verifies that down navigation works seamlessly across nested region boundaries.
#[test]
fn test_down_navigation_across_boundaries() {
  let mut context = Context::new();
  let r_parent = context.region_alloc(2).unwrap();
  let mut cursor = context.cursor(r_parent).unwrap(); // Block 1
  context.bind(cursor, Symbol(1), EntryId(10));

  // Nest a new region (Block 2)
  let _r_child = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(2), EntryId(20));

  // Go back up to Block 1
  context.up(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(1)), Some(EntryId(10)));
  assert_eq!(context.find(cursor, Symbol(2)), None);

  // Traverse back down across the region boundary using down()
  context.down(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(2)), Some(EntryId(20)));
}

/// Verifies that when a region expands and coalesces internally,
/// freeing it returns the entire coalesced block to the freelist as a single block range.
#[test]
fn test_coalesced_memory_reclamation() {
  let mut context = Context::new();
  
  // Allocate region of size 2
  let r = context.region_alloc(2).unwrap();
  let mut cursor = context.cursor(r).unwrap();

  // Grow region by pushing 3 blocks (triggers allocation and coalescing)
  context.push_block(&mut cursor).unwrap();
  context.push_block(&mut cursor).unwrap();
  context.push_block(&mut cursor).unwrap(); // Size is now 4

  // Free the region
  context.region_free(r);

  // The block freelist should contain the single coalesced interval of size 4
  assert_eq!(context.block_freelist_len(), 1);

  // Allocating a new region of size 4 should consume the coalesced interval from the freelist
  let r2 = context.region_alloc(4).unwrap();
  assert_eq!(context.block_freelist_len(), 0);
  context.region_free(r2);
}

/// Verifies that two sibling scopes nested under the same parent block
/// can shadow the same parent symbol independently.
#[test]
fn test_shadowing_independence_in_sibling_scopes() {
  let mut context = Context::new();
  let r = context.region_alloc(5).unwrap();
  let mut cursor = context.cursor(r).unwrap(); // Parent Block 1
  context.bind(cursor, Symbol(42), EntryId(10));

  // Sibling A: Pushes child, shadows Symbol(42) to 20
  context.push_block(&mut cursor).unwrap(); // Block 2
  context.bind(cursor, Symbol(42), EntryId(20));

  context.up(&mut cursor).unwrap(); // Back to parent

  // Sibling B: Pushes child, shadows Symbol(42) to 30
  context.push_block(&mut cursor).unwrap(); // Block 3
  context.bind(cursor, Symbol(42), EntryId(30));

  // Cursor at Block 3 resolves Symbol(42) to 30
  assert_eq!(context.find(cursor, Symbol(42)), Some(EntryId(30)));

  // Cursor at Block 2 resolves Symbol(42) to 20
  context.up(&mut cursor).unwrap();
  context.down(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(42)), Some(EntryId(20)));
}

/// Verifies that zero-sized region allocation is rejected (returns None).
#[test]
fn test_cursor_spawning_on_zero_sized_region() {
  let mut context = Context::new();
  let r = context.region_alloc(0);
  assert!(r.is_none());
}

/// Verifies that re-binding a symbol inside the same block updates the entry identifier correctly.
#[test]
fn test_symbol_re_binding_in_same_block() {
  let mut context = Context::new();
  let r = context.region_alloc(1).unwrap();
  let cursor = context.cursor(r).unwrap();

  // Initial binding
  context.bind(cursor, Symbol(1), EntryId(10));
  assert_eq!(context.find(cursor, Symbol(1)), Some(EntryId(10)));

  // Re-bind to new entry
  context.bind(cursor, Symbol(1), EntryId(20));
  assert_eq!(context.find(cursor, Symbol(1)), Some(EntryId(20)));
}

/// Verifies that next() horizontally traverses sibling blocks even when they belong to different regions.
#[test]
fn test_next_sibling_navigation_across_regions() {
  let mut context = Context::new();
  let r_parent = context.region_alloc(1).unwrap();
  let mut cursor = context.cursor(r_parent).unwrap(); // Parent

  // Sibling A: nested Region A
  let r_a = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(1), EntryId(10));

  // Go back to parent, push sibling B (nested Region B)
  context.up(&mut cursor).unwrap();
  let _r_b = context.push_region(&mut cursor).unwrap();
  context.bind(cursor, Symbol(2), EntryId(20));

  // Navigate back to Sibling A
  let mut cursor_a = context.cursor(r_a).unwrap();
  assert_eq!(context.find(cursor_a, Symbol(1)), Some(EntryId(10)));

  // Call next() from Sibling A — should cross region boundary to Sibling B
  context.next(&mut cursor_a).unwrap();
  assert_eq!(context.find(cursor_a, Symbol(2)), Some(EntryId(20)));
}

/// Verifies that growing a region non-contiguously (due to separator allocations)
/// correctly splits intervals, and freeing it returns all separate intervals to the freelist.
#[test]
fn test_non_contiguous_region_growth_and_reclamation() {
  let mut context = Context::new();

  // 1. Allocate Region A (size 1)
  let r_a = context.region_alloc(1).unwrap();
  let mut cursor_a = context.cursor(r_a).unwrap();

  // 2. Allocate Region B (size 1) to act as a separator in the block arena
  let _r_b = context.region_alloc(1).unwrap();

  // 3. Grow Region A (forces a non-contiguous block allocation)
  context.push_block(&mut cursor_a).unwrap(); // Block A2 in Region A

  // 4. Free Region A
  context.region_free(r_a);

  // The block freelist must now contain the two separate intervals
  assert_eq!(context.block_freelist_len(), 2);

  // Allocating two independent regions of size 1 should consume both intervals from the freelist
  let r3 = context.region_alloc(1).unwrap();
  let r4 = context.region_alloc(1).unwrap();
  assert_eq!(context.block_freelist_len(), 0);

  context.region_free(r3);
  context.region_free(r4);
}

/// Verifies that symbol lookups successfully resolve bindings stored in older,
/// non-contiguous intervals of the same region.
#[test]
fn test_symbol_lookup_in_non_contiguous_region() {
  let mut context = Context::new();
  
  // Allocate Region A (size 1) and bind Symbol(1)
  let r_a = context.region_alloc(1).unwrap();
  let mut cursor_a = context.cursor(r_a).unwrap();
  context.bind(cursor_a, Symbol(1), EntryId(10));

  // Separator region
  let _r_b = context.region_alloc(1).unwrap();

  // Grow Region A non-contiguously and bind Symbol(2)
  context.push_block(&mut cursor_a).unwrap();
  context.bind(cursor_a, Symbol(2), EntryId(20));

  // From the new non-contiguous block, lookup should resolve both Symbol(2) and Symbol(1)
  assert_eq!(context.find(cursor_a, Symbol(2)), Some(EntryId(20)));
  assert_eq!(context.find(cursor_a, Symbol(1)), Some(EntryId(10)));
}

/// Verifies that climbing down navigates successfully to children even if
/// they were allocated in non-contiguous intervals.
#[test]
fn test_down_navigation_to_non_contiguous_child() {
  let mut context = Context::new();
  let r_a = context.region_alloc(1).unwrap();
  let mut cursor = context.cursor(r_a).unwrap(); // Parent in Interval 1

  // Separator region
  let _r_b = context.region_alloc(1).unwrap();

  // Push child block (allocated in non-contiguous Interval 2)
  context.push_block(&mut cursor).unwrap(); // Child in Interval 2
  context.bind(cursor, Symbol(2), EntryId(20));

  // Go back up to Parent
  context.up(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(2)), None);

  // Navigate down to child in the non-contiguous interval
  context.down(&mut cursor).unwrap();
  assert_eq!(context.find(cursor, Symbol(2)), Some(EntryId(20)));
}

/// Verifies that a cursor pointing to a freed region can still navigate the old links,
/// but can no longer resolve any bindings.
#[test]
fn test_stale_cursor_navigation_vs_lookup() {
  let mut context = Context::new();
  let r = context.region_alloc(2).unwrap();
  let mut cursor = context.cursor(r).unwrap(); // Block 1
  context.bind(cursor, Symbol(1), EntryId(10));

  context.push_block(&mut cursor).unwrap(); // Block 2
  context.bind(cursor, Symbol(2), EntryId(20));

  // Free the region
  context.region_free(r);

  // Traversals should still succeed because block linkages in the arena are preserved
  context.up(&mut cursor).unwrap(); // Go back to Block 1
  
  // But lookup on Block 1 must fail because region bindings were cleared
  assert_eq!(context.find(cursor, Symbol(1)), None);
}
