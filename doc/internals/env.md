# Meerkat Internals: Environment System (`env`)

## Part 1: Overview and Tutorial

### Motivation

Meerkat's architecture blurs the line between static and dynamic execution phases. Live code updates requires the modification of data structures, schemas, and implementations at granular levels (such as specific services or their fields) without restarting the Meerkat node.

Prior to this system, Meerkat's handling of scope analysis, free-variable detection, name binding, dependency tracking, and runtime lookups spanned several decoupled subsystems. This architectural fragmentation presented several challenges:

- **Redundant AST Traversals:** Subsystems for free-variable analysis, alpha renaming, and dependency analysis performed separate, repeated walks of the AST, often duplicating effort.
- **Allocation Churn:** Analysis phases frequently allocated and merged temporary sets during recursive descents, creating substantial heap allocation pressure.
- **Dynamic Resolution Overhead:** Variable binding and evaluation relied linear reverse scans of stack-based environments and dynamic runtime hash map queries locked behind transaction mechanisms.
- **Jump Discontinuities:** Services were tracked completely separately from standard lexical block scoping, preventing unified top-level scope management.

The Environment System (`env`) is designed to support the unification of these cross-cutting concerns. By presenting a single data structure generic over a type `T`, the system allows the compiler and runtime to bind types, AST nodes, or dynamic values into a unified static lexical scope that stays resident.

### Invariants and Conventions

- **Sentinel conventions**: `BlockId(0)` is the null/sentinel value. Every arena access first checks `id.0 != 0` before computing an index.

- `RegionId(0)` is not a sentinel; slot 0 is a valid live region. Do not treat `RegionId(0)` as "no region".

- **Caller Responsibility**: The environment context `Context<T>` is generic over the value type `T` stored in the region bindings. The caller must assume responsibility for `T` and its reference, copy, and clone semantics.

- Storing values in the internal hashmap does not guarantee they remain fresh or in sync with external state. A retrieved reference or copy may represent state that has since become obsolete or invalid relative to the caller's domain.

### Mini-Tutorial: Setup, Navigation, and Binding

This example demonstrates how to set up the context, allocate regions, navigate the lexical block tree, bind values, and clean up resources.

```rust
use meerkat_env::{Context, Symbol};

// 1. Setup the context
let mut ctx = Context::<String>::new();
let region = ctx.region_alloc(10).unwrap();

// 2. Spawn a cursor at the root block of the region
let mut cursor = ctx.cursor(region).unwrap();

// 3. Bind a value to the root scope
ctx.bind(cursor, Symbol(1), String::from("root_service"));

// 4. Tree Navigation: Allocate a child block and move into it
ctx.push_block(&mut cursor).unwrap();
ctx.bind(cursor, Symbol(2), String::from("child_node"));

// 5. Tree Navigation: Move back up to the parent block
ctx.up(&mut cursor).unwrap();

// 6. Tree Navigation: Move back down to the first child
ctx.down(&mut cursor).unwrap();

// 7. Resolve a symbol lexically from the current cursor position
let val = ctx.find(cursor, Symbol(1)).unwrap();
assert_eq!(val, "root_service");

// 8. Cleanup resources
ctx.region_free(region);
```

## Part 2: Public API Reference

The public API is designed to orchestrate block allocations, tree navigation, and symbol binding while hiding internal linkage and freelist management.

### Constants

```rust
pub const MAX_BLOCK_ID: u32 = u32::MAX - 1;
```

The maximum block identifier that may appear as the begin of an interval. Block IDs participate in half-open interval arithmetic: every allocation produces `end = begin + size`, and `end` must fit in `u32`. Therefore the largest usable `begin` value is `u32::MAX - 1`; a range starting there with `size = 1` yields `end = u32::MAX`, which still fits. Allowing `begin = u32::MAX` would make even a single-block allocation overflow.

```rust
pub const MAX_REGION_ID: u32 = u32::MAX;
```

The maximum number of live regions (i.e. the maximum valid `RegionId`). Region IDs are plain array indices; they are never used in range arithmetic and `RegionId(0)` is a valid live region (no sentinel). The full `u32` space `[0, u32::MAX]` is therefore usable, so the limit is `u32::MAX` rather than `u32::MAX - 1`. The `-1` pattern from `MAX_BLOCK_ID` does not apply here.

### Public Types

```rust
pub struct RegionId(pub u32);
```

A unique identifier for regions.

```rust
pub struct Symbol(pub u32);
```

A type-safe wrapper around symbol identifiers.

```rust
pub struct Cursor { 
  pub(super) i: BlockId 
}
```

Lightweight, copyable position handle in the environment context. Holds a reference-free identifier to avoid borrow locking the context.

### The Context Core (`Context<T>`)

`Context<T>` manages the block arena, regions, and range allocations. It holds active blocks, region mapping, and freelists.

-*Public Methods:**

- `pub fn default() -> Self`
	- Creates the default environment context.

- `pub fn new() -> Self`
	- Creates a new empty environment context.

- `pub fn region_alloc(&mut self, size: u32) -> Option<RegionId>`
	- Allocates a contiguous run of blocks representing a region.

- `pub fn region_free(&mut self, region_id: RegionId)`
	- Releases a region handle and returns its blocks to the freelist.

- `pub fn cursor(&self, id: RegionId) -> Option<Cursor>`
	- Spawns a cursor starting at the region's begin block.

- `pub fn up(&self, cursor: &mut Cursor) -> Option<()>`
	- Navigates to the parent block scope in-place.

- `pub fn down(&self, cursor: &mut Cursor) -> Option<()>`
	- Navigates to the first child block scope in-place.

- `pub fn next(&self, cursor: &mut Cursor) -> Option<()>`
	- Navigates to the next sibling block scope in-place.

- `pub fn find(&self, cursor: Cursor, symbol: Symbol) -> Option<&T>`
	- Resolves a symbol lexically by climbing parent scopes from the cursor. Returns a reference to the bound value of type `&T`.

- `pub fn bind(&mut self, cursor: Cursor, symbol: Symbol, entry: T)`
	- Binds a symbol to the cursor's current block with the given value.

- `pub fn push_block(&mut self, cursor: &mut Cursor) -> Option<()>`
	- Allocates a new child block in the current block's region and moves down to it.

- `pub fn push_region(&mut self, cursor: &mut Cursor) -> Option<RegionId>`
	- Allocates a new child region nested under the parent cursor's current block and moves the cursor into the root block of that region.

- `pub fn region_size(&self, id: RegionId) -> Option<u32>`
	- Returns the size of the allocated region.

- `pub fn blocks_capacity(&self) -> usize`
	- Returns the total capacity of the backing block array.

- `pub fn block_freelist_len(&self) -> usize`
	- Returns the number of items in the block freelist.

---

## Part 3: Internals Reference

This section details the private data structures, internal fields, and internal linkage logic that powers the environment system.

### LCRS and In-Memory Resident Graph

To store the hierarchical lexical block scopes efficiently within a flat memory arena, the environment system utilizes a Left-Child Right-Sibling (LCRS) data structure. By representing tree relationships through `down` (first child) and `next` (sibling) references, every block node maintains a fixed memory footprint. This contiguous layout maximizes CPU cache locality and avoids unpredictable heap allocations during scope resolution.

Furthermore, to support incremental live code updates, this lexical graph remains resident in memory. An in-memory graph replaces ephemeral name-based resolution. When a granular update is dispatched, such as modifying a service implementation, the localized LCRS linkages and bindings can be surgically patched. This enables safe, incremental modifications to the live structural representation without necessitating a full reboot or recompilation of the surrounding environment.

### Internal Context State (`Context<T>`)

The internal state of the `Context` maintains the flat arrays backing the memory arenas.

```rust
pub struct Context<T = u32> {
  block_arena: Vec<block::Block>,
  region_arena: Vec<region::Region<T>>,
  region_freelist: Vec<u32>,
  allocator: BlockAllocator,
}
```

#### Internal Fields

- `block_arena`: A `Vec<block::Block>` that stores the allocated structures representing the tree hierarchy.

- `region_arena`: A `Vec<region::Region<T>>` that stores the allocated region structures representing distinct scopes.

- `region_freelist`: A `Vec<u32>` of indices tracking freed region slots for reuse.

- `allocator`: The `BlockAllocator` instance responsible for managing underlying block range distributions.

#### Internal Methods

- `fn alloc_block_range(&mut self, size: u32) -> Option<Interval>`
	- Allocates a block range of size from the freelist or arena.

- `fn alloc_block_in_region(&mut self, region_id: RegionId) -> Option<BlockId>`
	- Allocates a new block within the given region, growing if needed.

- `fn region_alloc_child(&mut self, size: u32, parent: BlockId) -> Option<RegionId>`
	- Allocates a child region nested under a parent block scope.

- `fn block_is_live_in_region(&self, block: BlockId) -> bool`
	- Returns whether a block is currently live within its owning region. Validates that the block is non-null, in bounds, its recorded region is active, and falls within the region's active interval range (bounded by active_interval_used on the last interval).

- `fn get_region_id_from_block(&self, block: BlockId) -> Option<RegionId>`
	- Returns the region identifier of a block. Delegates to `block_is_live_in_region` for the membership check and then returns the owning `RegionId`.

- `fn block_freelist_interval(&self, idx: usize) -> Option<(BlockId, BlockId)>`
	- Returns boundaries of a freed block interval at the given index.

- `fn link_up(&mut self, block: BlockId, parent: BlockId)`
	- Links a block to its parent scope.

- `fn link_down(&mut self, block: BlockId, child: BlockId)`
	- Links a block to its first nested child scope.

- `fn link_last_child(&mut self, block: BlockId, last_child: BlockId)`
	- Links a block to its last nested child scope.

- `fn link_next(&mut self, block: BlockId, next: BlockId)`
	- Links a block to its next sibling scope.

### Memory Regions (`region::Region<T>`)

```rust
pub(super) struct Region<T> {
  is_active: bool,
  intervals: Vec<Interval>,
  bindings: HashMap<(BlockId, Symbol), T>,
  active_interval_used: u32,
}
```

Memory region containing block intervals and symbol bindings. `T` is the value type stored per binding. The region holds `T` by value and does not impose any bound on `T` beyond what its internal operations require.

#### Internal Fields

- `is_active`: A `bool` tracking whether the region is actively utilized or cleared.

- `intervals`: A `Vec<Interval>` representing the memory ranges owned by this region.

- `bindings`: A `HashMap` holding the bound variables of type `T`, keyed by a tuple of `BlockId` and `Symbol`.

- `active_interval_used`: A `u32` counter indicating how many blocks are consumed within the currently active interval.

#### Internal Methods

- `pub fn default() -> Self`
- Produces an inactive region with no intervals, no bindings, and zero `active_interval_used`.

- `pub(super) fn clear(&mut self)`
- Clears all allocated intervals and symbol bindings. Resets the region to an inactive, empty state. Drops all stored `T` values. After this call `is_active` is false and all collections are empty.

### Lexical Blocks (`block`)

The `block` module separates the concept of a block's structural data from its addressing mechanism. This separation enables the environment's flat-memory Left-Child Right-Sibling (LCRS) graph.

#### Block Addressing and Intervals

```rust
pub(super) struct BlockId(pub(super) u32);

```

**Purpose:** A unique identifier for blocks. It acts as a lightweight, type-safe wrapper around a `u32` value. Within the system, `BlockId` replaces standard Rust references, allowing the environment context to track node locations within the memory arena without borrow-checker conflicts.

```rust
pub(super) struct Interval {
  pub(super) begin: BlockId,
  pub(super) end: BlockId
}
```

**Purpose:** Represents a range of blocks, primarily used by the allocator and freelist to track contiguous chunks of available or allocated memory.

##### Internal Fields

- `begin`: The starting `BlockId` of the contiguous range.
- `end`: The ending `BlockId` of the contiguous range.

#### Block Nodes

```rust
pub(super) struct Block {
  pub(super) up: BlockId,
  pub(super) down: BlockId,
  pub(super) last_child: BlockId,
  pub(super) next: BlockId,
  pub(super) region: super::RegionId
}
```

**Purpose:** The node representing hierarchical scope relationships. While `BlockId` is the address, `Block` is the actual data structure residing at that address. It holds parent, child, and sibling references along with its `RegionId` to maintain the LCRS tree geometry.

##### Internal Fields

- `up`: The `BlockId` pointing to this block's parent.
- `down`: The `BlockId` pointing to this block's first nested child.
- `last_child`: The `BlockId` pointing to this block's most recently added child.
- `next`: The `BlockId` pointing to this block's sequential sibling.
- `region`: The `RegionId` associating this block with its owning region.


### Range Allocator (`alloc::BlockAllocator`)

```rust
pub(super) struct BlockAllocator {
  block_freelist: Vec<Interval>,
  block_next_id: BlockId,
}
```

Allocator managing block range reuse and freelist tracking.

#### Internal Fields

- `block_freelist`: A `Vec` of `Interval` structures holding previously freed memory ranges.

- `block_next_id`: The `BlockId` indicating the next unallocated boundary in the backing arena.

#### Internal Methods

- `pub(super) fn new() -> Self`
	- Creates a new empty block allocator.

- `pub fn default() -> Self`
	- Creates a default block allocator starting at `BlockId(1)`.

- `pub(super) fn alloc_block_range(&mut self, size: u32, block_arena: &mut Vec<super::block::Block>) -> Option<Interval>`
	- Allocates a range of block identifiers. Searches the freelist first, splitting any larger blocks found to satisfy the requested size, and falls back to growing the block arena if no suitable block is found on the freelist.

## Part 4: Remarks and Discussion

- **Stale References:** The `Context<T>` makes no active checks to ensure that a value `T` stored in a block remains semantically valid in the caller's domain over time. Storing `Clone` or `Copy` values means you are taking a snapshot; if external state updates, the environment will silently serve stale data.

- **Memory Leaks via Inactivity:** The arena heavily relies on the caller invoking `region_free`. Dropping the `Context` entirely will safely free all memory, but continuously allocating regions without freeing them in a long-lived context will perpetually grow the backing `Vec` structures.

- **Dangling Cursors:** Passing a `Cursor` across a `region_free` boundary renders it invalid. The `env` system enforces debug assertions to catch operations on blocks that are no longer live within an active region interval, but release builds do not guarantee bounds safety for stale cursors accessing recycled identifiers.

- **Region Zero is Live:** Do not mistake `RegionId(0)` for a sentinel. Only `BlockId(0)` is used as the null pointer.

- **Half-Open Interval Arithmetic:** Memory blocks are calculated as `end = begin + size`. Be aware that the `end` value is exclusive. Allocations at the extreme boundary cap `MAX_BLOCK_ID` at `u32::MAX - 1` to ensure `begin + size` does not overflow the 32-bit boundary.

## Part 5: Future Considerations

The current Abstract Syntax Tree (AST) implementation relies on individually heap-allocated objects connected via standard Rust references. While idiomatic for static compilation, this may pose challenges for live code updates; the same challenges that affected the environment system.

Replacing or modifying AST sub-trees during a live update requires individual deallocation and reallocation of nodes, leading to memory fragmentation and high allocator churn. Furthermore, relying on standard Rust references complicates surgical patching, as the borrow checker enforces strict ownership and lifetime rules.

The Environment System could be used as a blueprint for addressing these future challenges, either as they are encountered, or to prevent them from arising. The flat, arena-based memory model and reference-free index tracking (`BlockId`, `RegionId`) used in the `env` system could be adapted for the AST.
