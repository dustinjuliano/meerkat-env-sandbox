# Future Considerations - Block Scope System for Bytecode and TSLC

This document outlines unresolved future considerations, architectural proposals, and limitations of the Block Scope System (BSS) for bytecode compilation and Type-Safe Live Code Updates (TSLCU).

## 1. Type-Safe Live Code Updates (TSLCU)

In a live-running virtual machine executing bytecode, active stacks can be patched dynamically:

- **Active Frame Mapping**: By mapping instruction pointer ranges to active `Region` boundaries, the VM identifies which execution frames correspond to which static scopes
- **Index-Based Migration**: When a `Region` is updated, the VM can map the old `BlockId` indices to the new `BlockId` indices to rewrite active execution stacks or closures without introducing state corruption
- **Hot Swap Isolation**: Reclaiming old regions via the `block_freelist` recycles unused bytecode frames in `O(1)` time, mitigating memory leaks or pointer fragmentation

## 2. Limitations of the Current Region-Based Design

While efficient, the current design has structural limitations that must be addressed for large-scale production use:

- **No Global Freelist Coalescing**: While active block additions within a region now coalesce contiguously in `alloc_block_in_region`, adjacent free intervals returned to the global `block_freelist` are not merged when regions are freed. This prevents reuse of adjacent slots for subsequent larger requests and requires the backing vector to grow
- **Monotonic Memory Footprint**: The `Context` never shrinks the `block_arena` vector. Once memory is allocated to the high-water mark, it remains consumed, potentially leading to elevated memory utilization under sparse, long-lived allocations
- **Relocation Complexity**: Since active scopes hold raw `BlockId`s, we cannot easily compact the backing vector. Moving a region to reclaim space makes compaction more difficult, introducing garbage collection overhead
- **First-Fit Fragmentation**: The First-Fit strategy can lead to fragmenting large free blocks into smaller remainders at the front of the list, which may increase search depth for subsequent larger allocation requests

## 3. Closure Flattening and Escape Analysis

The static LCRS tree structure directly assists in the compilation of escaping variables:

- **Escape Invariant Mapping**: During static analysis, variables defined in a parent block that are referenced inside a child block that outlives its parent are marked as escaping
- **Granular Allocation**: The VM can choose to heap-allocate scopes only for blocks containing escaping variables. Non-escaping scopes can remain entirely stack-allocated, minimizing heap overhead
- **Direct Stack Reference**: The `up` chain allows the compiler to construct a precise stack-relative link for non-escaping captures, reducing pointer indirection

## 4. Service Dependency Analysis and Topological Sorting

Integrating the block scope layout directly into the system's static dependency analyzer facilitates rapid compilation and granular live updates:

- **Boundary-Based Dependency Mapping**: Each service corresponds to an allocated `Region`. During static variable name resolution, if a resolved `BlockId` falls within another service's region, a dependency edge is registered. Since each `Block` in `block_arena` directly stores its owning `RegionId`, checking if `resolved_id` belongs to another service is resolved in `O(1)` constant time by checking `block_arena[resolved_id - 1].region`, completely bypassing the need to search the region's intervals
- **Ultra-Fast Topological Sorting**: Instead of parsing the entire AST to locate references, variable lookups are mapped to their target defining `BlockId`s. Since the block-to-region owner lookup is `O(1)` via direct array indexing, determining whether service `A` depends on service `B` is a constant-time check. Sorting the dependency graph remains a linear-time `O(V + E)` operation, unaffected by the disjoint interval list layout of the regions
- **Granular Live Update Propagation**: When a service `Region` is updated, the dependency graph determines which downstream services are affected. The compiler re-runs type checks and name resolution only on the `Region` of those downstream services, reducing the need for a full project rebuild
- **Circular Reference Safeguards**: To detect cyclic dependencies between services, the system traverses the resolved `BlockId` references between service regions. In the current system, dependency checking is performed via a recursive Depth-First Search (DFS) over a nested object/scope tree, which exhibits poor worst-case time complexity ($O(V^2)$ or $O(2^V)$ in deeply nested graphs) and increases the risk of stack overflow. Under this proposed design, because variables are bound to flat `BlockId` structures, the compiler can run Tarjan's strongly connected components (SCC) algorithm in `O(V + E)` linear time to identify dependency loops and fail the build before runtime execution

## 5. REPL and Global Scope Mutation

This section outlines an architectural concept to support interactive statement evaluation in a REPL session. The global scope in a REPL is open-ended and dynamic, which typically requires environment rebuilding or copying. Our region-based allocation model is proposed to facilitate dynamic mutation of the global scope as the user enters new statements:

- **Chained Region Allocation**: Each statement typed in the REPL allocates a small statement-specific `Region`. The root block of the new statement has its `up` pointer linked to the leaf block of the previous statement
- **Frontier Shadowing**: When a user redefines a variable, the new binding is placed in the latest statement's `Region`. Because the lookup starts at the newest statement and walks `up`, older global bindings are shadowed naturally
- **Dynamic Scoping Erasure**: If the user overrides or removes a statement, the system reclaims its specific `Region` through `block_freelist`. The `up` pointer of the subsequent statement is patched to point to the preceding statement, bypassing the deleted block in `O(1)` time

## 6. Cache-Optimized Payload Separation (Structure of Arrays)

This section outlines a proposal for a structural layout optimization designed to improve execution performance by separating structural topology from data payloads. This layout is designed exclusively to support the type-value pairing proposed in Section 7.

To optimize cache efficiency, the physical representation of blocks must avoid carrying heavy payload weight. In a standard node-based tree structure, every node contains the structural links alongside its type and value pointers. When the compiler or VM traverses this tree (for example, walking up parents to resolve a variable name), it must pull the entire node payload into the L1 cache, potentially polluting the cache lines with metadata.

By separating the structural relations from the payloads (using a Structure of Arrays layout), we isolate the 16-byte LCRS `Block` relationships into a dense contiguous vector `block_arena`. The type and value payloads are stored in separate parallel vectors (`types` and `values`) that are indexed by the same `BlockId` offset. When name resolution is executed, the traversal runs entirely inside the cache-dense `block_arena` vector. Only when the correct `BlockId` is found does the VM query the parallel vectors to load the type or value payload:

- **Relational Integrity Isolation**: Traversing the lexical scope tree during name resolution accesses only the `blocks: Vec<Block>` array. Because `Block` is so small, multiple records fit into a single CPU L1 cache line, tending to reduce cache misses
- **Parallel Array Allocation**: Value and type payloads are stored in separate vectors (`types: Vec<Option<Type>>` and `values: Vec<Option<Value>>`) that are allocated in parallel to the `blocks` vector and share the same `BlockId` index
- **Deferred Payload Loading**: The compiler and VM fetch payloads from the `types` and `values` vectors only when a traversal successfully locates the target `BlockId`
- **Payload Cache Protection**: Walking the `up`/`down`/`next` links to check scope nesting does not load or touch large runtime data, reducing cache pollution

## 7. Type-Value Pairing for Mixed-Phase Validation

This is a proposal to store both compile-time `Type` and runtime `Value` slots within the parallel arrays described in Section 6, enabling safe dynamic updates.

The Type-Value Pairing proposal builds directly upon the separated Structure of Arrays layout. By keeping parallel `types` and `values` vectors under the same `BlockId` index, the block scope system bridges the gap between the static compilation phase and the dynamic execution phase. During program load time, the compiler's static analysis passes populate the `types` vector and verify correctness. During runtime execution, the interpreter/VM reads and updates the `values` vector:

- **Unified Phase Mapping**: Maintaining both slots inside the environment allows the same scope system to serve static typing checks during loading and dynamic evaluation during execution
- **Dynamic Type Safety Checks**: When a live update swaps a service implementation at runtime, the compiler compiles the new version, writes its signatures to a temporary `Region`, and checks if the new `Type` definitions are compatible with the active `Value` representations
- **Atomic Rollback Invariant**: If compatibility checks fail, the VM aborts the hot swap, retaining the active `Region` and reducing the risk of runtime state corruption

## 8. Comparison and Trade-Off Analysis of Proposals 6 and 7

Proposals 6 and 7 are highly integrated but serve distinct design goals: Section 6 proposes a physical memory layout optimization (Structure of Arrays), whereas Section 7 proposes a semantic phase-binding model (Type-Value Pairing).

### Proposal 6 (Structure of Arrays)
- **Pros**:
  - Improves cache density: the LCRS tree traversal uses only 16 bytes per block, maximizing L1 cache utility
  - Prevents memory traversal from pulling large, unneeded metadata into CPU registers
- **Cons**:
  - Increases code complexity: the system must ensure that `blocks`, `types`, and `values` vectors are kept strictly in sync during resizing and region recycling
  - Double indirection: reading a variable requires searching the tree first, then performing a second index lookup in the parallel array

### Proposal 7 (Type-Value Pairing)
- **Pros**:
  - Unifies static and dynamic environments: compile-time validation and runtime execution share a single BSS, simplifying VM design
  - Safe hot swapping: type checks are performed against active values before committing a live update, preventing state corruption
- **Cons**:
  - Memory overhead: every allocated block index has slots for both `Type` and `Value`, which means half of the slots are empty during specific phases (e.g. types are empty for dynamic values, and values are empty during static analysis)

## 9. Summary of Core Allocator Bottlenecks

Based on the analysis of the region-based scope allocator, the following concrete performance bottlenecks represent targets for future optimization:

- **Freelist Linear Scan Latency**: Allocating new regions or disjoint block intervals requires a linear scan over `block_freelist` which scales as `O(F)`. Under high-churn workloads, this can impact environment setup times
- **Freelist Fragmentation and Bloat**: The absence of free block coalescing causes adjacent freed intervals to remain fragmented in `block_freelist`. This prevents reuse for larger allocations and forces the backing `block_arena` to grow monotonically, potentially causing memory footprint expansion
