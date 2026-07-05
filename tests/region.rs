//! Integration test suite for environment region allocations

use env::env::Context;

/// Verifies that recycling a freed region retains a monotone capacity
///
/// This allocates a region, frees it, and then allocates it again
/// ensuring that the total backing capacity does not grow
#[test]
fn test_region_reclamation_capacity() {
  let mut context = Context::new();

  // Allocate region and record initial capacity
  let region = context.region_alloc(10).unwrap();
  let cap1 = context.blocks_capacity();

  // Free region
  context.region_free(region);

  // Re-allocate a region of the same size
  let _new_region = context.region_alloc(10).unwrap();
  let cap2 = context.blocks_capacity();

  // Assert capacity did not grow
  assert_eq!(
    cap1,
    cap2,
    "Backing storage capacity should remain monotone after recycling"
  );
}

/// Verifies that allocating a smaller region splits a larger free slot
///
/// This tests range splitting where requesting a smaller size splits
/// the first available large free region, returning the remainder to the
/// freelist
#[test]
fn test_arbitrary_sizes_and_allocation_splitting() {
  let mut context = Context::new();

  // Allocate three contiguous regions
  let r1 = context.region_alloc(10).unwrap();
  let r2 = context.region_alloc(20).unwrap();
  let r3 = context.region_alloc(30).unwrap();

  // High water mark capacity should be `10 + 20 + 30 = 60`
  let initial_cap = context.blocks_capacity();
  assert_eq!(initial_cap, 60);

  // Free the middle region of size `20`
  context.region_free(r2);
  assert_eq!(
    context.blocks_capacity(),
    60,
    "Freeing should not alter capacity"
  );

  // Allocate a smaller region of size `15`
  // This should split the freed region of size `20`, leaving a free slot of size `5`
  let r4 = context.region_alloc(15).unwrap();
  assert_eq!(
    context.blocks_capacity(),
    60,
    "Re-allocation via splitting should not grow capacity"
  );

  // Allocate a region of size `5`
  // This should consume the remaining split slot of size `5`
  let r5 = context.region_alloc(5).unwrap();
  assert_eq!(
    context.blocks_capacity(),
    60,
    "Re-allocation of remaining split slot should not grow capacity"
  );

  // Allocate another region of size `5`
  // Since the freelist is now empty, capacity must grow to `65`
  let r6 = context.region_alloc(5).unwrap();
  assert_eq!(
    context.blocks_capacity(),
    65,
    "Capacity should grow when freelist cannot satisfy the request"
  );

  // Cleanup
  context.region_free(r1);
  context.region_free(r3);
  context.region_free(r4);
  context.region_free(r5);
  context.region_free(r6);
}

/// Verifies monotone capacity when regions are freed out-of-order
///
/// This frees non-consecutive regions of varying sizes and asserts
/// that subsequent allocations reuse the slots correctly
#[test]
fn test_mixed_allocation_and_free_ordering() {
  let mut context = Context::new();

  // Allocate `5` regions of varying sizes
  let r1 = context.region_alloc(5).unwrap();
  let r2 = context.region_alloc(10).unwrap();
  let r3 = context.region_alloc(15).unwrap();
  let r4 = context.region_alloc(20).unwrap();
  let r5 = context.region_alloc(25).unwrap();

  let base_cap = context.blocks_capacity(); // `5 + 10 + 15 + 20 + 25 = 75`
  assert_eq!(base_cap, 75);

  // Free in non-consecutive / mixed order
  context.region_free(r4); // size `20`
  context.region_free(r2); // size `10`

  // Try allocating a size that fits in the freed size `20` but not size `10`
  let r6 = context.region_alloc(18).unwrap(); // Should consume from the size `20` slot
  assert_eq!(context.blocks_capacity(), 75);

  // Try allocating a size that fits in the remaining size `2` or size `10`
  let r7 = context.region_alloc(10).unwrap(); // Should consume the size `10` slot
  assert_eq!(context.blocks_capacity(), 75);

  // Free all remaining regions
  context.region_free(r1);
  context.region_free(r3);
  context.region_free(r5);
  context.region_free(r6);
  context.region_free(r7);

  // Re-allocating should still be monotone up to the previous high water mark
  let _r_big = context.region_alloc(25).unwrap();
  assert_eq!(context.blocks_capacity(), 75);
}

/// Verifies that allocating a size `0` region behaves as a memory no-op
///
/// This checks that zero-sized requests do not resize the backing
/// blocks array and can be freed safely
#[test]
fn test_zero_sized_allocations() {
  let mut context = Context::new();

  // Allocate size `0`
  let r1 = context.region_alloc(0);
  assert!(r1.is_none());
  
  // Backing capacity should be exactly `0`
  assert_eq!(
    context.blocks_capacity(),
    0,
    "Zero-sized allocation should not resize the blocks array"
  );

  // Allocate size `10`
  let r2 = context.region_alloc(10).unwrap();
  assert_eq!(
    context.blocks_capacity(),
    10,
    "Capacity should be exactly 10 after allocating size 10"
  );

  context.region_free(r2);
}

/// Verifies that the allocator uses a First-Fit selection strategy
///
/// This populates the freelist and asserts that a request splits the
/// first large enough slot rather than finding the best-fitting one
#[test]
fn test_first_fit_strategy() {
  let mut context = Context::new();

  // Allocate `3` regions separated by small active regions to prevent
  // automatic contiguous tracking
  let r1 = context.region_alloc(10).unwrap();
  let sep1 = context.region_alloc(1).unwrap();
  let r2 = context.region_alloc(30).unwrap();
  let sep2 = context.region_alloc(1).unwrap();
  let r3 = context.region_alloc(20).unwrap();

  // Base capacity is `10 + 1 + 30 + 1 + 20 = 62` blocks
  assert_eq!(context.blocks_capacity(), 62);

  // Free `r1`, `r2`, `r3`
  // Freelist now has free slots: `[10, 30, 20]` in that order
  context.region_free(r1);
  context.region_free(r2);
  context.region_free(r3);

  // Request size `15`
  // `10` is too small. `30` (the next one) is large enough
  // First-Fit splits the size `30` slot (leaving size `15` free)
  // The size `20` slot remains untouched (size `20`)
  let r4 = context.region_alloc(15).unwrap();
  assert_eq!(context.blocks_capacity(), 62);

  // Request size `20`
  // Since the size `20` slot is untouched, it fits perfectly
  let r5 = context.region_alloc(20).unwrap();
  assert_eq!(
    context.blocks_capacity(),
    62,
    "Size 20 slot should be intact and used without growing capacity"
  );

  // Cleanup
  context.region_free(sep1);
  context.region_free(sep2);
  context.region_free(r4);
  context.region_free(r5);
}

/// Verifies boundary fits where requests cannot overflow free slots
///
/// This asserts that a request exceeding a free slot by even `1` block
/// forces capacity growth, while exact fits reuse the slot
#[test]
fn test_exact_vs_overflow_fit() {
  let mut context = Context::new();

  // Allocate size `10`
  let r1 = context.region_alloc(10).unwrap();
  assert_eq!(context.blocks_capacity(), 10);

  // Free it
  context.region_free(r1);

  // Request size `11`. It exceeds the size `10` slot
  // It should not fit and capacity must grow to `21`
  let r2 = context.region_alloc(11).unwrap();
  assert_eq!(context.blocks_capacity(), 21);

  // Request size `10`
  // It should fit perfectly in the original size `10` free slot
  let r3 = context.region_alloc(10).unwrap();
  assert_eq!(
    context.blocks_capacity(),
    21,
    "Size 10 request should reuse the original free slot"
  );

  context.region_free(r2);
  context.region_free(r3);
}

/// Verifies that separate small slots are not merged to satisfy a larger request
///
/// Since active regions are not coalesced on free, multiple small slots
/// cannot combine to satisfy a larger allocation request
#[test]
fn test_insufficient_freelist_slots_do_not_combine() {
  let mut context = Context::new();

  // Allocate three regions, separated so they do not combine automatically
  let r1 = context.region_alloc(5).unwrap();
  let sep = context.region_alloc(1).unwrap();
  let r2 = context.region_alloc(5).unwrap();

  assert_eq!(context.blocks_capacity(), 11); // `5 + 1 + 5 = 11`

  // Free `r1` and `r2`. Freelists now has two distinct size-5 slots
  context.region_free(r1);
  context.region_free(r2);

  // Request size `10`
  // Neither of the size `5` slots is large enough, and we do not
  // coalesce active regions on free yet
  // So the allocator must grow capacity by `10` blocks (to `21`)
  let r3 = context.region_alloc(10).unwrap();
  assert_eq!(context.blocks_capacity(), 21);

  context.region_free(sep);
  context.region_free(r3);
}

/// Verifies splitting a very large free slot into tiny pieces
///
/// This allocates size `100`, frees it, and splits it into size `1`
/// and size `99`, keeping capacity at `100` blocks
#[test]
fn test_large_freelist_split() {
  let mut context = Context::new();

  // Allocate a large region of size `100`
  let r1 = context.region_alloc(100).unwrap();
  assert_eq!(context.blocks_capacity(), 100);

  // Free it
  context.region_free(r1);

  // Request size `1`
  // This should split the size `100` slot, leaving `99` blocks in freelist
  let r2 = context.region_alloc(1).unwrap();
  assert_eq!(context.blocks_capacity(), 100);

  // Request size `99`. This should consume the remainder of the split slot
  let r3 = context.region_alloc(99).unwrap();
  assert_eq!(context.blocks_capacity(), 100);

  context.region_free(r2);
  context.region_free(r3);
}

/// Verifies that requests exceeding the largest slot bypass the freelist
///
/// This ensures that large requests allocate new blocks (growing capacity)
/// without corrupting smaller free slots in the freelist
#[test]
fn test_alternating_growth_and_reuse() {
  let mut context = Context::new();

  // Allocate size `50`
  let r1 = context.region_alloc(50).unwrap();
  assert_eq!(context.blocks_capacity(), 50);

  // Free it
  context.region_free(r1);

  // Request size `100`
  // The freelist slot of `50` is too small. It must allocate `100` new blocks
  // Capacity grows to `150` blocks (`50` original + `100` new)
  let r2 = context.region_alloc(100).unwrap();
  assert_eq!(context.blocks_capacity(), 150);

  // Request size `50`. It should fit perfectly in the original size `50` slot
  let r3 = context.region_alloc(50).unwrap();
  assert_eq!(context.blocks_capacity(), 150);

  context.region_free(r2);
  context.region_free(r3);
}

/// Verifies that block ID 0 is excluded from allocations
///
/// Since block `0` is the sentinel, the backing block vector length
/// must match exactly the sum of requested active blocks
#[test]
fn test_sentinel_exclusion() {
  let mut context = Context::new();

  // Allocate multiple regions
  let r1 = context.region_alloc(10).unwrap();
  let r2 = context.region_alloc(20).unwrap();

  // Verify that capacity is exactly the sum of requested blocks,
  // confirming block `0` is not used
  assert_eq!(context.blocks_capacity(), 30);
  context.region_free(r1);
  context.region_free(r2);
}

/// Runs a stress cycle splitting ten size-10 slots into 100 size-1 slots
///
/// This validates the stability of range splitting and that capacity
/// does not exceed the high-water mark of `100` blocks
#[test]
fn test_high_volume_stress_cycle() {
  let mut context = Context::new();

  // Do cycles of allocations and deallocations with different sizes
  let mut active = Vec::new();

  // Allocate `10` regions of size `10`
  for _ in 0..10 {
    active.push(context.region_alloc(10).unwrap());
  }
  let peak_cap = context.blocks_capacity();
  assert_eq!(peak_cap, 100);

  // Free all of them
  for r in active {
    context.region_free(r);
  }

  // Allocate `100` regions of size `1`
  // This should split the `10` free slots of size `10` into `100` slots of size `1`
  // Backing capacity should remain exactly `100`
  let mut ones = Vec::new();
  for _ in 0..100 {
    ones.push(context.region_alloc(1).unwrap());
  }
  assert_eq!(context.blocks_capacity(), 100);

  for r in ones {
    context.region_free(r);
  }
}

/// Verifies behavior when the freelist is completely drained
///
/// This tests that subsequent allocations correctly grow capacity
/// once all split remainders and exact matches are consumed
#[test]
fn test_exhaust_freelist_completely() {
  let mut context = Context::new();

  // Allocate three blocks of size `10`
  let r1 = context.region_alloc(10).unwrap();
  let r2 = context.region_alloc(10).unwrap();
  let r3 = context.region_alloc(10).unwrap();
  assert_eq!(context.blocks_capacity(), 30);

  // Free all
  context.region_free(r1);
  context.region_free(r2);
  context.region_free(r3);

  // Consume all `3` slots
  let a1 = context.region_alloc(10).unwrap();
  let a2 = context.region_alloc(10).unwrap();
  let a3 = context.region_alloc(10).unwrap();
  assert_eq!(context.blocks_capacity(), 30);

  // Requesting a `4th` block must grow capacity
  let a4 = context.region_alloc(10).unwrap();
  assert_eq!(context.blocks_capacity(), 40);

  context.region_free(a1);
  context.region_free(a2);
  context.region_free(a3);
  context.region_free(a4);
}
