# Bounds and Allocation Safeguards

## Open Issues

- None

## Note: Arena Indexing

Internal arena accesses are safe by structural invariant: every `BlockId`
produced by the allocator is within `block_arena` bounds by construction, and
all external entry points (`iter`, `iter_mut`, `region_free`, `region_size`)
check `idx < arena.len()` before indexing. Iterator traversal guards on
`block.0 != 0` before any dereference. Cross-context stale handle safety is
a separate ownership concern covered by the Instance ID Tagging suggestion.
