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

use env::env::{Context, EntryId, RegionId, Symbol, Iter};

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
  let mut i = ctx.iter_mut(root_region)?;

  i.bind(Symbol(1), EntryId(0));
  i.bind(Symbol(2), EntryId(1));

  i.push()?;
  i.bind(Symbol(2), EntryId(2));

  i.push()?;
  i.bind(Symbol(3), EntryId(3));

  i.up()?;
  i.push()?;
  i.bind(Symbol(2), EntryId(4));
  i.bind(Symbol(3), EntryId(5));

  i.up()?;
  i.bind(Symbol(3), EntryId(6));

  i.up()?;
  i.bind(Symbol(4), EntryId(7));

  Some(Program {
    ctx,
    mem,
    root_region,
  })
}

fn find<'a>(p: &'a Program, i: Iter, sym: Symbol) -> Option<&'a Entry> {
  let eid = i.find(sym)?;
  Some(&p.mem[eid.0 as usize])
}

fn simulate_eval(p: &Program) -> Option<()> {
  let mut i = p.ctx.iter(p.root_region)?;

  // Block 1
  find(p, i, Symbol(1));
  find(p, i, Symbol(2));
  find(p, i, Symbol(3));

  // Enter Block 2
  i.down()?;
  find(p, i, Symbol(2));
  find(p, i, Symbol(3));

  // Enter Block 3
  i.down()?;
  find(p, i, Symbol(3));
  find(p, i, Symbol(2));

  // Exit Block 3
  i.up()?;

  // Enter Block 4
  i.down()?;
  i.next()?;
  find(p, i, Symbol(2));
  find(p, i, Symbol(3));

  // Exit Block 4 (returns to Block 2)
  i.up()?;
  find(p, i, Symbol(3));

  // Exit Block 2 (returns to Block 1)
  i.up()?;
  find(p, i, Symbol(4));

  Some(())
}

fn main() -> Result<(), &'static str> {
  let p = simulate_lexical_scoping().ok_or("build error")?;
  simulate_eval(&p).ok_or("find error")?;
  Ok(())
}
