# Future Considerations - Block Scope System for Bytecode and TSLC

This report analyzes how the current `Context`, `Region`, and Left-Child Right-Sibling (LCRS) `Block` structures support the compilation phase, VM execution, and Type-Safe Live Code Updates (TSLCU) in a bytecode virtual machine.

## 1. Bytecode Compilation and Name Resolution

During the compilation phase from source code to bytecode, the static LCRS tree structure (`up`, `down`, `next`) acts as the primary layout map:

- **Upvalue Resolution**: The compiler walks the `up` pointers to compute the scope distance between a variable definition and its usage in a nested closure
- **Lexical Addressing**: This distance is mapped directly to a stack offset or environment slot, allowing the compiler to emit direct lexical load/store instructions like `LOAD_UPVALUE`
- **Closure Flattening**: The tree structures define which variables escape each block, enabling the compiler to pack runtime environments efficiently into flat arrays

## 2. Type-Safe Live Code Updates (TSLCU)

In a live-running virtual machine executing bytecode, active stacks can be patched dynamically:

- **Active Frame Mapping**: By mapping instruction pointer ranges to active `Region` boundaries, the VM knows which execution frames correspond to which static scopes
- **Index-Based Migration**: When a `Region` is updated, the VM can map the old `BlockId` indices to the new `BlockId` indices to rewrite active execution stacks or closures without state corruption
- **Hot Swap Isolation**: Reclaiming old regions via the `block_freelist` ensures that unused bytecode frames are recycled in `O(1)` time without memory leaks or pointer fragmentation

## 3. Unified Static and Dynamic representation

Representing scope relationships inside a flat `Vec<Block>` indexed by `BlockId`s provides:

- **Cheap Cloning**: The VM can clone the environment layout at linear copy speed for speculative executions, testing, or multi-threading
- **Cache-Friendly Traversal**: Following indices in a flat array is highly cache-friendly compared to heap-allocated pointer chasing

## 4. Performance Implications of Efficiency and Growth

As the codebase grows and handles larger compilation units, several efficiency invariants apply:

- **Linear Free Search Overhead**: The `region_alloc` call performs a linear scan over `block_freelist` which is `O(F)` where `F` is the number of free slots. Under high frequency allocations, this scan can become a bottleneck
- **Swap-Remove Efficiency**: The transition to `swap_remove` guarantees `O(1)` complexity for reclaiming slots, ensuring that freelist cleanup times remain constant regardless of size
- **Backing Array Growth Monotonicity**: The backing vector `blocks` grows monotonically. Since we only resize upwards, the memory footprint represents the high-water mark of block storage, ensuring predictable allocator memory usage

## 5. Limitations of the Current Region-Based Design

While efficient, the current design has structural limitations that must be addressed for large-scale production use:

- **No Contiguous Coalescing**: When regions are freed, adjacent free intervals are not merged. If `[10, 20)` and `[20, 30)` are freed, they remain as separate slots, preventing a subsequent request for size 20 from using them and forcing the backing vector to grow
- **Monotonic Memory Footprint**: The `Context` never shrinks the `blocks` vector. Once memory is allocated to the high-water mark, it remains consumed, which can lead to memory bloat under sparse long-lived allocations
- **Relocation Complexity**: Since active scopes hold raw `BlockId`s, we cannot easily compact the backing vector. Moving a region to reclaim space would require rewriting all active stack and compiler references, which introduces garbage collection overhead
- **First-Fit Fragmentation**: The First-Fit strategy can lead to fragmenting large free blocks into smaller remainders at the front of the list, increasing the search depth for subsequent larger allocation requests

## 6. Asynchronous Concurrency and Event Loops

The destination system is strictly single-threaded but runs highly concurrent async tasks on event loops (e.g. using `tokio`):

- **Borrows Across Await Points**: A future cannot hold a mutable borrow `&mut Context` across an `.await` boundary, as this would block the event loop and prevent other concurrent tasks from accessing the context
- **Local Event-Loop Sharing**: To share `Context` among concurrent async tasks safely on a single thread, the context can be wrapped in `Rc<RefCell<Context>>`, enabling multiple tasks to borrow it briefly and yield before `.await` points
- **Actor-Based Scoping**: Alternatively, `Context` can be owned exclusively by a dedicated coordinator task, with other async tasks requesting allocation or deallocation via channels (message-passing), keeping the context borrow-free

## 7. Closure Flattening and Escape Analysis

The static LCRS tree structure directly assists in the compilation of escaping variables:

- **Escape Invariant Mapping**: During static analysis, variables defined in a parent block that are referenced inside a child block that outlives its parent are marked as escaping
- **Granular Allocation**: The VM can choose to heap-allocate scopes only for blocks containing escaping variables. Non-escaping scopes can remain entirely stack-allocated, maximizing VM speed
- **Direct Stack Reference**: The `up` chain allows the compiler to construct a precise stack-relative link for non-escaping captures, avoiding pointer indirection

## 8. Service Dependency Analysis and Topological Sorting

Integrating the block scope layout directly into the system's static dependency analyzer facilitates rapid compilation and granular live updates:

- **Boundary-Based Dependency Mapping**: Each service corresponds to an allocated `Region`. During static variable name resolution, if a resolved `BlockId` falls within another service's `Region` boundary (`begin.0 <= resolved_id.0 < end.0`), a dependency edge is registered
- **Ultra-Fast Topological Sorting**: Instead of parsing the entire AST to locate references, variable lookups are mapped to their target defining `BlockId`s. Since each service's lexical scope maps to a contiguous `Region` range, determining whether service `A` depends on service `B` simplifies to checking if a resolved `BlockId` falls within `B`'s boundaries (`begin.0 <= resolved_id.0 < end.0`). Once edges are built, sorting the dependency graph is an `O(V + E)` linear-time operation, allowing the compiler to instantly determine the bootstrap order of services
- **Granular Live Update Propagation**: When a service `Region` is updated, the dependency graph determines which downstream services are affected. The compiler re-runs type checks and name resolution only on the `Region` of those downstream services, avoiding a full project rebuild
- **Circular Reference Safeguards**: To detect cyclic dependencies between services, the system traverses the resolved `BlockId` references between service regions. In the current system, dependency checking is performed via a recursive Depth-First Search (DFS) over a nested object/scope tree, which incurs poor worst-case time complexity ($O(V^2)$ or $O(2^V)$ in deeply nested graphs) and carries a high risk of stack overflow. Under this proposed design, because variables are bound to flat `BlockId` structures, the compiler can run Tarjan's strongly connected components (SCC) algorithm in `O(V + E)` linear time to identify dependency loops and fail the build safely before any runtime code is loaded

## 9. REPL and Global Scope Mutation

This is a proposal for an architectural concept to support interactive statement evaluation in a REPL session. The global scope in a REPL is open-ended and dynamic, which typically requires environment rebuilding or copying. Our region-based allocation model is proposed to facilitate dynamic mutation of the global scope as the user enters new statements:

- **Chained Region Allocation**: Each statement typed in the REPL allocates a small statement-specific `Region`. The root block of the new statement has its `up` pointer linked to the leaf block of the previous statement
- **Frontier Shadowing**: When a user redefines a variable, the new binding is placed in the latest statement's `Region`. Because the lookup starts at the newest statement and walks `up`, older global bindings are shadowed naturally
- **Dynamic Scoping Erasure**: If the user overrides or removes a statement, the system reclaims its specific `Region` through `block_freelist`. The `up` pointer of the subsequent statement is patched to point to the preceding statement, bypassing the deleted block in constant time

## 10. Cache-Optimized Payload Separation (Structure of Arrays)

This is a proposal for a structural layout optimization designed to improve execution performance by separating structural topology from data payloads. This layout is designed exclusively to support the type-value pairing proposed in Section 11.

To achieve maximum cache efficiency, the physical representation of blocks must avoid carrying heavy payload weight. In a standard node-based tree structure, every node contains the structural links alongside its type and value pointers. When the compiler or VM traverses this tree (for example, walking up parents to resolve a variable name), it must pull the entire node payload into the L1 cache, polluting it with metadata. 

By separating the structural relations from the payloads (using a Structure of Arrays layout), we isolate the 12-byte LCRS `Block` relationships into a dense contiguous vector `blocks`. The type and value payloads are stored in separate parallel vectors (`types` and `values`) that are indexed by the same `BlockId` offset. When name resolution is executed, the traversal runs entirely inside the cache-dense `blocks` vector. Only when the correct `BlockId` is found does the VM query the parallel vectors to load the type or value payload:

- **Relational Integrity Isolation**: Traversing the lexical scope tree during name resolution accesses only the `blocks: Vec<Block>` array. Because `Block` is so small, multiple records fit into a single CPU L1 cache line, preventing cache misses
- **Parallel Array Allocation**: Value and type payloads are stored in separate vectors (`types: Vec<Option<Type>>` and `values: Vec<Option<Value>>`) that are allocated in parallel to the `blocks` vector and share the same `BlockId` index
- **Deferred Payload Loading**: The compiler and VM fetch payloads from the `types` and `values` vectors only when a traversal successfully locates the target `BlockId`
- **Payload Cache Protection**: Walking the `up`/`down`/`next` links to check scope nesting does not load or touch large runtime data, protecting the CPU caches from pollution

## 11. Type-Value Pairing for Mixed-Phase Validation

This is a proposal to store both compile-time `Type` and runtime `Value` slots within the parallel arrays described in Section 10, enabling safe dynamic updates.

The Type-Value Pairing proposal builds directly upon the separated Structure of Arrays layout. By keeping parallel `types` and `values` vectors under the same `BlockId` index, the block scope system bridges the gap between the static compilation phase and the dynamic execution phase. During program load time, the compiler's static analysis passes populate the `types` vector and verify correctness. During runtime execution, the interpreter/VM reads and updates the `values` vector:

- **Unified Phase Mapping**: Maintaining both slots inside the environment allows the same scope system to serve static typing checks during loading and dynamic evaluation during execution
- **Dynamic Type Safety Checks**: When a live update swaps a service implementation at runtime, the compiler compiles the new version, writes its signatures to a temporary `Region`, and checks if the new `Type` definitions are compatible with the active `Value` representations
- **Atomic Rollback Invariant**: If compatibility checks fail, the VM aborts the hot swap and leaves the active `Region` untouched, preventing runtime state corruption

## 12. Comparison and Trade-Off Analysis of Proposals 10 and 11

Proposals 10 and 11 are highly integrated but serve distinct design goals: Section 10 proposes a physical memory layout optimization (Structure of Arrays), whereas Section 11 proposes a semantic phase-binding model (Type-Value Pairing). 

### Proposal 10 (Structure of Arrays)
- **Pros**:
  - Dramatically improves cache density: the LCRS tree traversal uses only 12 bytes per block, maximizing L1 cache utility
  - Prevents memory traversal from pulling large, unneeded metadata into CPU registers
- **Cons**:
  - Increases code complexity: the system must ensure that `blocks`, `types`, and `values` vectors are kept strictly in sync during resizing and region recycling
  - Double indirection: reading a variable requires searching the tree first, then performing a second index lookup in the parallel array

### Proposal 11 (Type-Value Pairing)
- **Pros**:
  - Unifies static and dynamic environments: compile-time validation and runtime execution share a single BSS, simplifying VM design
  - Safe hot swapping: type checks are performed against active values before committing a live update, preventing state corruption
- **Cons**:
  - Memory overhead: every allocated block index has slots for both `Type` and `Value`, which means half of the slots are empty during specific phases (e.g. types are empty for dynamic values, and values are empty during static analysis)
