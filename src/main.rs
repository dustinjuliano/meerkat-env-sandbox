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

use env::env::{Context, EntryId, RegionId, Symbol, Cursor};

#[allow(dead_code)]
#[derive(Debug)]
struct Entry {
  value: String,
  ty: String,
}

struct Program {
  ctx: Context,
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

  ctx.bind(cursor, Symbol(1), EntryId(0));
  ctx.bind(cursor, Symbol(2), EntryId(1));

  ctx.push_block(&mut cursor)?;
  ctx.bind(cursor, Symbol(2), EntryId(2));

  ctx.push_block(&mut cursor)?;
  ctx.bind(cursor, Symbol(3), EntryId(3));

  ctx.up(&mut cursor)?;
  ctx.push_block(&mut cursor)?;
  ctx.bind(cursor, Symbol(2), EntryId(4));
  ctx.bind(cursor, Symbol(3), EntryId(5));

  ctx.up(&mut cursor)?;
  ctx.bind(cursor, Symbol(3), EntryId(6));

  ctx.up(&mut cursor)?;
  ctx.bind(cursor, Symbol(4), EntryId(7));

  Some(Program {
    ctx,
    mem,
    root_region,
  })
}

fn find<'a>(p: &'a Program, cursor: Cursor, sym: Symbol) -> Option<&'a Entry> {
  let eid = p.ctx.find(cursor, sym)?;
  Some(&p.mem[eid.0 as usize])
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

fn main() -> Result<(), &'static str> {
  let p = simulate_lexical_scoping().ok_or("build error")?;
  simulate_eval(&p).ok_or("find error")?;
  Ok(())
}
