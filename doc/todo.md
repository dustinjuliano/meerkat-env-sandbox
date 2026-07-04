This comment is missing these features needed to realize the full BSS vision:

### 1. APIs to Mutate the Tree (LCRS Links)

Currently, all `Block` instances are pushed with `BlockId(0)` sentinel links on all structural fields (`up`, `down`, `next`). The `Context` lacks any APIs to build the hierarchy:
* **What is missing**: You need methods like `pub fn add_child_block(&mut self, parent: BlockId) -> BlockId` or `pub fn link_sibling(&mut self, target: BlockId, sibling: BlockId)` to construct the tree layout during static parsing.

### 2. Lexical Binding (Environment Maps)

A BSS must map symbols (variable names) to their types and values within each block:
* **What is missing**: You need an environment storage system per block. As proposed in the Structure of Arrays section, this is best done by having a parallel array mapping `BlockId` -> `HashMap<SymbolId, Payload>` or sorted arrays of `(SymbolId, Payload)` to keep traversals cache-dense.
* **What is missing**: A resolution method: `pub fn lookup(&self, start: BlockId, name: SymbolId) -> Option<&Payload>` which walks up the `up` chain from the target `BlockId` and queries the binding tables.

### 3. Parameterization (`Context<T>`)

The `doc/requirements.md` file notes that BSS entries should hold both `Value` and `Type` to support mixed-phase VM compilation and execution:
* **What is missing**: Parameterizing the `Context` (or the parallel payload storage) to allow it to be configured for either compile-time type arrays, runtime value arrays, or a unified type-value map.

### 4. Checked arithmetic safeguards

* **What is missing**: Implementing the checked additions (`checked_add`) detailed in `bounds.md` so that the primitive allocation stage is robust against overflows before any compiler layers are built on top.
