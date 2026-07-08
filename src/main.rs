/*
Sample Program Structure:
{ // Block 1
  let a = "a";
  let b = "b1";
  { // Block 2
    let b = "b2";
    { // Block 3
      let c = "c1";
    }
    { // Block 4
      let b = "b3";
      let c = "c2";
    }
    let c = "c3";
  }
  let d = "d";
}

Symbol Association:
- "a" => Symbol(1)
- "b" => Symbol(2)
- "c" => Symbol(3)
- "d" => Symbol(4)
*/

use meerkat_lib::env::{Context, Cursor, RegionId, Symbol};

#[allow(dead_code)]
#[derive(Debug)]
struct Entry {
  value: String,
  ty: String,
}

struct Program {
  ctx: Context<usize>,
  mem: Vec<Entry>,
  root_region: RegionId,
}

fn simulate_lexical_scoping() -> Option<Program> {
  let mem = vec![
    Entry { value: "a".to_string(), ty: "String".to_string() },
    Entry { value: "b1".to_string(), ty: "String".to_string() },
    Entry { value: "b2".to_string(), ty: "String".to_string() },
    Entry { value: "c1".to_string(), ty: "String".to_string() },
    Entry { value: "b3".to_string(), ty: "String".to_string() },
    Entry { value: "c2".to_string(), ty: "String".to_string() },
    Entry { value: "c3".to_string(), ty: "String".to_string() },
    Entry { value: "d".to_string(), ty: "String".to_string() },
  ];

  let mut ctx = Context::new();
  let root_region = ctx.region_alloc(8)?;
  let mut cursor = ctx.cursor(root_region)?;

  ctx.bind(cursor, Symbol(1), 0);
  ctx.bind(cursor, Symbol(2), 1);

  ctx.push_block(&mut cursor)?;
  ctx.bind(cursor, Symbol(2), 2);

  ctx.push_block(&mut cursor)?;
  ctx.bind(cursor, Symbol(3), 3);

  ctx.up(&mut cursor)?;
  ctx.push_block(&mut cursor)?;
  ctx.bind(cursor, Symbol(2), 4);
  ctx.bind(cursor, Symbol(3), 5);

  ctx.up(&mut cursor)?;
  ctx.bind(cursor, Symbol(3), 6);

  ctx.up(&mut cursor)?;
  ctx.bind(cursor, Symbol(4), 7);

  Some(Program {
    ctx,
    mem,
    root_region,
  })
}

fn find<'a>(p: &'a Program, cursor: Cursor, sym: Symbol) -> Option<&'a Entry> {
  let idx = p.ctx.find(cursor, sym)?;
  Some(&p.mem[*idx])
}

fn simulate_eval(p: &Program) -> Option<()> {
  let mut cursor = p.ctx.cursor(p.root_region)?;

  // Block 1
  find(p, cursor, Symbol(1));
  find(p, cursor, Symbol(2));
  find(p, cursor, Symbol(3));

  // Enter Block 2
  p.ctx.down(&mut cursor)?;
  find(p, cursor, Symbol(2));
  find(p, cursor, Symbol(3));

  // Enter Block 3
  p.ctx.down(&mut cursor)?;
  find(p, cursor, Symbol(3));
  find(p, cursor, Symbol(2));

  // Exit Block 3
  p.ctx.up(&mut cursor)?;

  // Enter Block 4
  p.ctx.down(&mut cursor)?;
  p.ctx.next(&mut cursor)?;
  find(p, cursor, Symbol(2));
  find(p, cursor, Symbol(3));

  // Exit Block 4 (returns to Block 2)
  p.ctx.up(&mut cursor)?;
  find(p, cursor, Symbol(3));

  // Exit Block 2 (returns to Block 1)
  p.ctx.up(&mut cursor)?;
  find(p, cursor, Symbol(4));

  Some(())
}

fn simulate_simultaneous_cursors(p: &Program) -> Option<()> {
  // Shows two cursors traversing the same immutable Context independently.
  let mut c1 = p.ctx.cursor(p.root_region)?;
  let mut c2 = p.ctx.cursor(p.root_region)?;

  // c1 navigates down to Block 2
  p.ctx.down(&mut c1)?;

  // c2 navigates down to Block 2, then Block 3, then sibling Block 4
  p.ctx.down(&mut c2)?;
  p.ctx.down(&mut c2)?;
  p.ctx.next(&mut c2)?;

  // Both cursors can be used simultaneously for queries without borrowing issues
  find(p, c1, Symbol(2));
  find(p, c2, Symbol(3));

  Some(())
}

fn simulate_async_continuations_with_cursors() -> Option<()> {
  let mut ctx = Context::new();
  let root = ctx.region_alloc(8)?;

  // Simulating async continuations where different tasks hold different cursors
  // to the same context. Since we are single-threaded async, mutations happen
  // sequentially but execution order is interleaved.
  let mut task1_cursor = ctx.cursor(root)?;
  let mut task2_cursor = ctx.cursor(root)?;

  // Task 1 runs, mutates context
  ctx.bind(task1_cursor, Symbol(1), 1);
  ctx.push_block(&mut task1_cursor)?;
  ctx.bind(task1_cursor, Symbol(2), 2);

  // Task 2 resumes. Cursors are absolutely consistent because they only store index IDs.
  // The underlying Context is updated sequentially, so Task 2 instantly sees Task 1's
  // changes as long as it looks up symbols from its valid cursor position.
  ctx.find(task2_cursor, Symbol(1));
  
  // Task 2 moves down into the block created by Task 1
  ctx.down(&mut task2_cursor)?;
  ctx.find(task2_cursor, Symbol(2));

  // Task 1 resumes, adds another block
  ctx.push_block(&mut task1_cursor)?;
  ctx.bind(task1_cursor, Symbol(3), 3);

  // Task 2 can navigate to the new block
  ctx.down(&mut task2_cursor)?;
  ctx.find(task2_cursor, Symbol(3));

  Some(())
}

fn simulate_region_usability() -> Option<()> {
  let mut ctx = Context::new();
  
  // Allocate a root region
  let r1 = ctx.region_alloc(4)?;
  let mut c1 = ctx.cursor(r1)?;
  ctx.bind(c1, Symbol(1), 1);

  // Create a child region, cursor moves to its root block
  let r2 = ctx.push_region(&mut c1)?;
  ctx.bind(c1, Symbol(2), 2);

  // Cursor in child region can lexically find bindings from parent region
  ctx.find(c1, Symbol(1));
  ctx.find(c1, Symbol(2));

  // We can spawn a separate cursor in the parent region
  let parent_cursor = ctx.cursor(r1)?;
  ctx.find(parent_cursor, Symbol(1));

  // Free the child region when done
  ctx.region_free(r2);

  Some(())
}

fn simulate_class_hierarchy_regions() -> Option<()> {
  let mut ctx = Context::new();
  
  // Top-level scope region
  let global_region = ctx.region_alloc(10)?;
  let global_cursor = ctx.cursor(global_region)?;
  ctx.bind(global_cursor, Symbol(100), 100); // global variable

  // Class A region (lexical scope of the class)
  let mut class_a_builder = global_cursor; // copy the cursor
  let class_a_region = ctx.push_region(&mut class_a_builder)?;
  ctx.bind(class_a_builder, Symbol(200), 200); // Class A member

  // Method A.foo region (lexical scope of the method/field)
  let mut method_foo_builder = class_a_builder; // copy the cursor
  let method_foo_region = ctx.push_region(&mut method_foo_builder)?;
  
  // Inside the method, everything below is blocks
  ctx.push_block(&mut method_foo_builder)?;
  ctx.bind(method_foo_builder, Symbol(300), 300); // Local variable in block 1
  
  ctx.push_block(&mut method_foo_builder)?;
  ctx.bind(method_foo_builder, Symbol(301), 301); // Local variable in block 2

  // Method foo cursor can resolve symbols all the way up to global
  ctx.find(method_foo_builder, Symbol(301)); // Found in current block
  ctx.find(method_foo_builder, Symbol(300)); // Found in parent block
  ctx.find(method_foo_builder, Symbol(200)); // Found in Class A region
  ctx.find(method_foo_builder, Symbol(100)); // Found in Global region

  // Class B region (spawned from global_cursor so it's sibling to Class A)
  let mut class_b_builder = global_cursor; // copy global cursor
  let class_b_region = ctx.push_region(&mut class_b_builder)?;
  ctx.bind(class_b_builder, Symbol(400), 400); // Class B member

  // Method B.bar region
  let mut method_bar_builder = class_b_builder; // copy class B cursor
  let method_bar_region = ctx.push_region(&mut method_bar_builder)?;
  
  // Method bar blocks
  ctx.push_block(&mut method_bar_builder)?;
  ctx.bind(method_bar_builder, Symbol(500), 500);
  
  // Method bar can resolve Class B and Global, but NOT Class A
  ctx.find(method_bar_builder, Symbol(500)); // Found in current block
  ctx.find(method_bar_builder, Symbol(400)); // Found in Class B region
  ctx.find(method_bar_builder, Symbol(100)); // Found in Global region
  
  // Clean up
  ctx.region_free(method_bar_region);
  ctx.region_free(class_b_region);
  ctx.region_free(method_foo_region);
  ctx.region_free(class_a_region);

  Some(())
}

fn main() -> Result<(), &'static str> {
  let p = simulate_lexical_scoping().ok_or("build error")?;
  simulate_eval(&p).ok_or("find error")?;
  simulate_simultaneous_cursors(&p).ok_or("simultaneous cursor error")?;
  simulate_async_continuations_with_cursors().ok_or("async cursors error")?;
  simulate_region_usability().ok_or("region usability error")?;
  simulate_class_hierarchy_regions().ok_or("class hierarchy regions error")?;
  Ok(())
}
