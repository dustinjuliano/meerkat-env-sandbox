# Bounds and Allocation Safeguards — Remaining Concerns

## Open Issues

### Double-Free on `region_free`
- `region_free` does not check whether the target `RegionId` is currently active
- Calling it on an already-freed ID pushes a duplicate onto `region_freelist`
- Subsequent allocations pop duplicate IDs, causing two live regions to share
  one `RegionId` and corrupting bindings

## Note: Arena Indexing

Internal arena accesses are safe by structural invariant: every `BlockId`
produced by the allocator is within `block_arena` bounds by construction, and
all external entry points (`iter`, `iter_mut`, `region_free`, `region_size`)
check `idx < arena.len()` before indexing. Iterator traversal guards on
`block.0 != 0` before any dereference. Cross-context stale handle safety is
a separate ownership concern covered by the Instance ID Tagging suggestion.
