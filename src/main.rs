/*
Sample Program Structure:
{
  let a = "a";
  let b = "b1";
  {
    let b = "b2";
    {
      let c = "c";
    }
  }
  let d = "d";
}

Symbol Association:
- "a" => Symbol(1)
- "b" => Symbol(2)
- "c" => Symbol(3)
- "d" => Symbol(4)
*/

use env::env::{Context, EntryId, RegionId, Symbol};

#[allow(dead_code)]
#[derive(Debug)]
struct Entry {
  value: String,
  ty: String,
}

#[allow(dead_code)]
struct Program {
  ctx: Context,
  mem: Vec<Entry>,
  root_region: RegionId,
}

// Simulates static lexical scoping
fn build_program() -> Option<Program> {
  let mem = vec![
    Entry { value: "a".to_string(), ty: "String".to_string() },
    Entry { value: "b1".to_string(), ty: "String".to_string() },
    Entry { value: "b2".to_string(), ty: "String".to_string() },
    Entry { value: "c".to_string(), ty: "String".to_string() },
    Entry { value: "d".to_string(), ty: "String".to_string() },
  ];

  let mut ctx = Context::new();
  let root_region = ctx.region_alloc(5);
  let mut i = ctx.iter_mut(root_region)?;

  i.bind(Symbol(1), EntryId(0));
  i.bind(Symbol(2), EntryId(1));

  i.push();
  i.bind(Symbol(2), EntryId(2));

  i.push();
  i.bind(Symbol(3), EntryId(3));

  i = i.up()?;
  i = i.up()?;

  i.bind(Symbol(4), EntryId(4));

  Some(Program {
    ctx,
    mem,
    root_region,
  })
}

/// Simulates evaulator using pre-computed lexical scopes
fn walk_and_lookup(p: &Program) -> Option<()> {
  let mut cursor = p.ctx.iter(p.root_region)?;

  cursor.find(Symbol(2));
  cursor.find(Symbol(3));

  cursor = cursor.down()?;

  cursor.find(Symbol(2));
  cursor.find(Symbol(3));

  cursor = cursor.down()?;

  cursor.find(Symbol(3));
  cursor.find(Symbol(2));
  cursor.find(Symbol(1));
  cursor.find(Symbol(4));

  cursor = cursor.up()?;
  let _ = cursor.up()?;

  Some(())
}

fn main() -> Result<(), &'static str> {
  let pg = build_program().ok_or("build error")?;
  walk_and_lookup(&pg).ok_or("find error")?;
  Ok(())
}
