# TODO

## Next Actions

- **Checked Arithmetic and Failure Propagation**: Replace raw increments with `checked_add` in `BlockAllocator` and propagate allocation errors via `Option` or `Result`
- **Keyspace Exhaustion Safeguards**: Implement overflow validation for `BlockId`, `RegionId`, `Symbol`, and `EntryId` counters to prevent keyspace wrap-around as detailed in [bounds.md](bounds.md)
- **Double-Free Prevention**: Reject deallocation requests for inactive regions to prevent duplicate entries in `region_freelist`
- **Lookup Bounds Verification**: Validate `BlockId` and `RegionId` handles against arena bounds prior to indexing to prevent panic/crash vulnerabilities on out-of-bounds lookups

## Suggestions

- **Instance ID Tagging**: Tag `BlockId` and `RegionId` handles with context instance IDs to validate access and prevent cross-context lookups
- **Stale Iterator Validation**: Add checks to detect and invalidate iterators pointing to blocks inside freed regions
