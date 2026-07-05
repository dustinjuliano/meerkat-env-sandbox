# TODO

## Next Actions

- **Double-Free Prevention**: Reject `region_free` calls on inactive regions to
  prevent duplicate `region_freelist` entries and aliased `RegionId` corruption

## Suggestions

- **Instance ID Tagging**: Tag `BlockId` and `RegionId` handles with context
  instance IDs to detect and reject cross-context lookups
- **Stale Iterator Validation**: Detect iterators pointing into freed regions
