# Bounds and Allocation Safeguards Analysis

This document details the boundary conditions, integer overflow issues, and resource limits of the `Context` block allocation system.

## 1. Diagnostic Analysis of Flaws and Vulnerabilities

Several structural gaps and missing checks inside the allocator and handle resolution paths present stability issues:

### Arithmetic Overflow in Region Bounds

In [alloc.rs](../src/env/alloc.rs), the allocation frontier is advanced using raw addition:
`let end = BlockId((begin.0) + size);`

- **Debug Mode Assertion Panic**: In debug compilations, standard arithmetic operators trigger panic when `begin.0 + size` exceeds `u32::MAX`
- **Release Mode Wraparound**: In release builds, overflow wraps around modulo `2^32`, potentially overlapping with active allocations or the `BlockId(0)` sentinel
- **Linkage Corruption**: A wrapped identifier can point to active memory, corrupting structural links (`up`, `down`, `next`) and causing infinite loops during tree traversal

### Monotonic ID Exhaustion

The allocator counter `block_next_id` is monotonic and advances permanently when a freelist check fails:

- **Unreclaimed Space Expansion**: The counter is not decremented when a region is freed. Re-allocating a range that cannot be satisfied by existing slots in `block_freelist` forces the counter forward
- **Artificial Exhaustion**: In long-running sessions, `block_next_id` can eventually reach the `u32::MAX` limit even if active memory footprint remains low, locking the allocator from processing further requests

### Unverified Arena Indexing

The internal data structures do not validate block indices during resolution:

- **Missing Index Guard**: Resolving structural relationships relies on raw index lookups like `self.block_arena[(block.0 as usize) - 1]` without verifying if `block.0 as usize` is within the arena bounds
- **Stale and Out-of-Bounds Handles**: Iterators or callers holding arbitrary or outdated `BlockId` and `RegionId` handles can access the arena, potentially causing out-of-bounds panics or corrupting state if a handle belongs to a different context instance

### Double-Free Vulnerability

The `Context::region_free` implementation does not verify if a region is active:

- **Freelist Duplication**: Calling `region_free` on an already reclaimed `RegionId` pushes duplicate indices onto `region_freelist`
- **Identifier Collisions**: Subsequent allocations pop duplicate IDs, resulting in multiple active regions sharing the same `RegionId` and corrupting bindings

### Allocation Failure Wraparound

If the allocator returns the `BlockId(0)` sentinel on failure:

- **Index Underflow Panic**: Functions like `IterMut::push` perform `(new_block.0 as usize) - 1`. If `new_block` is `BlockId(0)`, this calculation underflows `usize` and causes a runtime panic

## 2. Memory Capacity vs ID Limits

There is a gap between physical memory capacity and the `u32` identifier space:

- **Memory Consumption**: A `block_arena` vector containing `u32::MAX` elements of size 16 bytes requires approximately `68.7 GB` of contiguous physical RAM
- **Preemptive Heap OOM**: A user virtual machine will typically encounter an Out-Of-Memory (OOM) panic from the system heap allocator before reaching the theoretical `u32` limit
- **Lack of Compaction**: Because the backing store is a flat `Vec<Block>`, memory cannot be reclaimed or shrunk unless we implement a compaction pass that shifts active regions and updates all external references

## 3. Recommended Mitigation Strategies

To secure the allocator against crashes and undefined behavior under extreme allocations, several strategies are proposed:

### Safe Checked Arithmetic and Error Propagation

Replace raw increments with safe checked operations and bubble up allocation failures to callers:

- **Checked Calculations**: Rewrite the allocation step using `begin.0.checked_add(size)` and verify the resulting boundary does not exceed `MAX_BLOCK_ID`
- **Result-Based Signatures**: Refactor `region_alloc` and `alloc_block_in_region` to return `Result<RegionId, AllocError>` or `Option<RegionId>` instead of panicking on overflow
- **Caller Recovery**: Allow the compiler or virtual machine interpreter to catch allocation limits, flush transient resources, and report errors gracefully instead of crashing the process

### Two-Pass Copying Compaction

Introduce an active compaction phase to recycle ID gaps and shrink the backing vector:

- **Mark Phase**: Traverse active `Region` handles to identify all occupied block indexes in the backing store
- **Relocate Phase**: Copy active blocks to the front of `block_arena`, reducing the vector size and resetting `block_next_id` to the end of the compacted range
- **Pointer Rewriting**: Update `up`, `down`, and `next` indices inside the relocated blocks by applying the relocation offset, and rewrite the active `Region` boundaries held by compiler/VM clients

### Instance ID Tagging

Enforce handle security by validating ownership:

- **Generational Handlers**: Tag all `BlockId` and `RegionId` handles with context instance IDs
- **Access Verification**: Check the handle tag against the context instance ID before any lookup or mutation to prevent cross-context and stale handle lookup issues
