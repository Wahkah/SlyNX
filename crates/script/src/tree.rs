//! The script table's cells, read into a syntax tree (Sly 1: scripts.md sections 2 and 4, world-file.md 4.3;
//! Sly 2: specs/sly2/scripts.md sections 1 to 3).
//!
//! A [`Program`] holds every module of one world in one arena: [`Node`]s addressed by [`NodeId`], with the elements
//! of each list stored contiguously. Both games use the same nodes: a Sly 2 table is read through its own cell
//! numbering into the Sly 1 kinds, and its two new keywords get kinds of their own ([`kind::EVAL_ONE`],
//! [`kind::NO_VALUE`]).

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;

use formats::world::WorldLayout;

/// Index of a node in a [`Program`].
pub type NodeId = u32;

/// Which game a script table comes from: it selects the cell numbering, the built-in operations, the option map
/// layout and the custom-property range.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Game {
    /// Sly Cooper and the Thievius Raccoonus.
    #[default]
    Sly1,
    /// Sly 2: Band of Thieves.
    Sly2,
    /// Sly 3: Honor Among Thieves (specs/sly3/scripts.md): value kinds from specs/sly3/world-file.md 4.1,
    /// keywords mapped onto Sly 2's, built-ins numbered apart (`crate::sly3`).
    Sly3,
}

/// What a raw cell kind means in one game.
enum Cell {
    Nil,
    Keyword(u8),
    Int,
    Float,
    Vector,
    Matrix,
    Coeffs,
    Range,
    Smoothing,
    Bool,
    Symbol,
    Builtin,
    List,
    Object,
    Other,
}

impl Game {
    /// Source-position bytes after each tag byte in a debug table.
    fn debug_bytes(self) -> usize {
        match self {
            Game::Sly1 => 10,
            Game::Sly2 | Game::Sly3 => 16,
        }
    }

    fn cell(self, k: u8) -> Cell {
        match (self, k) {
            (_, 0) => Cell::Nil,
            (Game::Sly1, 1..=kind::LAST_KEYWORD) => Cell::Keyword(k),
            (Game::Sly1, 20) => Cell::Int,
            (Game::Sly1, 21) => Cell::Float,
            (Game::Sly1, 22) => Cell::Vector,
            (Game::Sly1, 23) => Cell::Matrix,
            (Game::Sly1, 24) => Cell::Coeffs,
            (Game::Sly1, 25) => Cell::Range,
            (Game::Sly1, 26) => Cell::Smoothing,
            (Game::Sly1, 27) => Cell::Bool,
            (Game::Sly1, 28) => Cell::Symbol,
            (Game::Sly1, 29) => Cell::Builtin,
            (Game::Sly1, 30) => Cell::List,
            (Game::Sly1, 32) => Cell::Object,
            // Sly 2 (specs/sly2/scripts.md 2 and 3): keywords up to 20, values moved up by one and reordered.
            (Game::Sly2, 1..=20) => Cell::Keyword(sly2_keyword(k)),
            (Game::Sly2, 21) => Cell::Int,
            (Game::Sly2, 22) => Cell::Float,
            (Game::Sly2, 23) => Cell::Bool,
            (Game::Sly2, 24) => Cell::Symbol,
            (Game::Sly2, 25) => Cell::Builtin,
            (Game::Sly2, 26) => Cell::Vector,
            (Game::Sly2, 27) => Cell::Matrix,
            (Game::Sly2, 28) => Cell::Coeffs,
            (Game::Sly2, 29) => Cell::Range,
            (Game::Sly2, 30) => Cell::Smoothing,
            (Game::Sly2, 31) => Cell::List,
            (Game::Sly2, 33) => Cell::Object,
            // Sly 3 (specs/sly3/world-file.md 4.1, scripts.md 3): Sly 2's value kinds renumbered down by 2;
            // keywords mapped onto Sly 2's.
            (Game::Sly3, 1..=18) => Cell::Keyword(sly3_keyword(k)),
            (Game::Sly3, 19) => Cell::Int,
            (Game::Sly3, 20) => Cell::Float,
            (Game::Sly3, 21) => Cell::Bool,
            (Game::Sly3, 22) => Cell::Symbol,
            (Game::Sly3, 23) => Cell::Builtin,
            (Game::Sly3, 24) => Cell::Vector,
            (Game::Sly3, 25) => Cell::Matrix,
            (Game::Sly3, 26) => Cell::Coeffs,
            (Game::Sly3, 27) => Cell::Range,
            (Game::Sly3, 28) => Cell::Smoothing,
            (Game::Sly3, 29) => Cell::List,
            (Game::Sly3, 31) => Cell::Object,
            _ => Cell::Other,
        }
    }
}

/// A Sly 2 keyword kind as the kind the interpreter knows (specs/sly2/scripts.md 3).
fn sly2_keyword(k: u8) -> u8 {
    match k {
        6 => kind::EVAL_ONE,
        7..=16 => k - 1,
        17 => kind::NO_VALUE,
        18 => kind::QUOTE,
        19 => kind::SLY2_KIND_19,
        // The rest-parameter marker (specs/sly2/scripts.md 3).
        20 => kind::REST,
        // 1 set, 2 define, 5 assert keep their numbers; 3 and 4 are unknown in both games.
        _ => k,
    }
}

/// A Sly 3 keyword kind as the kind the interpreter knows (specs/sly3/scripts.md 3): 1 to 3 as Sly 2, 4 to 15 are
/// Sly 2's 5 to 16, 16 is quote (Sly 2's 18), 17 is Sly 2's unknown 19 and 18 marks the rest parameter.
fn sly3_keyword(k: u8) -> u8 {
    match k {
        1..=3 => sly2_keyword(k),
        4..=15 => sly2_keyword(k + 1),
        16 => kind::QUOTE,
        17 => kind::SLY2_KIND_19,
        _ => kind::REST,
    }
}

/// Cell kinds (the low seven bits of a cell's tag byte).
pub mod kind {
    /// The empty list.
    pub const NIL: u8 = 0;
    /// `(set symbol expr)`.
    pub const SET: u8 = 1;
    /// `(define symbol expr)`.
    pub const DEFINE: u8 = 2;
    /// `(assert expr)`.
    pub const ASSERT: u8 = 5;
    /// `(if test then [else])`.
    pub const IF: u8 = 6;
    /// `(or expr ...)`.
    pub const OR: u8 = 7;
    /// `(and expr ...)`.
    pub const AND: u8 = 8;
    /// `(cond (test body ...) ...)`.
    pub const COND: u8 = 9;
    /// The `else` marker of `cond` and `case`.
    pub const ELSE: u8 = 10;
    /// `(case expr ((value ...) body ...) ...)`.
    pub const CASE: u8 = 11;
    /// `(let ((symbol expr) ...) body ...)`.
    pub const LET: u8 = 12;
    /// `(while test body ...)`.
    pub const WHILE: u8 = 13;
    /// `(lambda (formals ...) body ...)`.
    pub const LAMBDA: u8 = 14;
    /// `(begin expr ...)`.
    pub const BEGIN: u8 = 15;
    /// `(quote datum)`.
    pub const QUOTE: u8 = 17;
    /// Marks the rest parameter in a `lambda` formals list (scripts.md 11, item 5; Sly 2 kind 20, Sly 3 kind 18);
    /// never on the disc.
    pub const REST: u8 = 19;
    /// Highest Sly 1 keyword kind.
    pub const LAST_KEYWORD: u8 = 19;
    /// Sly 2 keyword 6: evaluate one expression, or give no value without one (internal kind; Sly 2 only).
    pub const EVAL_ONE: u8 = 40;
    /// Sly 2 keyword 17: gives no value (internal kind; Sly 2 only).
    pub const NO_VALUE: u8 = 41;
    /// Sly 2 keyword kind 19 (Sly 3 kind 17): no special form, so a list headed by it fails (never on the disc).
    pub const SLY2_KIND_19: u8 = 42;
    /// `s32`.
    pub const INT: u8 = 20;
    /// `f32`.
    pub const FLOAT: u8 = 21;
    /// Three `f32`.
    pub const VECTOR: u8 = 22;
    /// Sixteen `f32`.
    pub const MATRIX: u8 = 23;
    /// Three quadratic coefficients.
    pub const COEFFS: u8 = 24;
    /// (min, max).
    pub const RANGE: u8 = 25;
    /// Smoothing parameters.
    pub const SMOOTHING: u8 = 26;
    /// `u32`, non-zero is true.
    pub const BOOL: u8 = 27;
    /// `u32` symbol ID.
    pub const SYMBOL: u8 = 28;
    /// `u32` built-in operation ID.
    pub const BUILTIN: u8 = 29;
    /// A nested list.
    pub const LIST: u8 = 30;
    /// An object (always the null object on the disc).
    pub const OBJECT: u8 = 32;
    /// Highest kind the format allows.
    pub const LAST: u8 = 36;
    /// The tag byte of an empty node.
    pub const EMPTY_TAG: u8 = 0xFF;
}

/// One cell of the syntax tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// The empty list (kind 0, or an `0xFF` tag among list elements).
    Nil,
    /// Kind 20.
    Int(i32),
    /// Kind 21.
    Float(f32),
    /// Kind 22.
    Vector([f32; 3]),
    /// Kind 23, a 4x4 transform in stored order.
    Matrix(Box<[f32; 16]>),
    /// Kind 24: g0 + g1 x + g2 x^2.
    Coeffs([f32; 3]),
    /// Kind 25: (min, max).
    Range([f32; 2]),
    /// Kind 26: slow speed, fast speed, fast time.
    Smoothing([f32; 3]),
    /// Kind 27.
    Bool(bool),
    /// Kind 28.
    Symbol(u32),
    /// Kind 29.
    Builtin(u32),
    /// Kind 30: elements `children[start .. start + len]` of the program.
    List {
        /// First element's position in [`Program::children`].
        start: u32,
        /// Element count.
        len: u32,
    },
    /// Kind 32 with its payload (0 is the null object).
    Object(u32),
    /// Kinds 1 to 19: language keywords.
    Keyword(u8),
    /// Any other payload-less kind up to 36 (none occurs on the disc).
    Other(u8),
}

/// A malformed script table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// File offset of the problem.
    pub offset: usize,
    /// What was wrong.
    pub what: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "script table: {} at offset {:#x}", self.what, self.offset)
    }
}

/// Every module of one world.
#[derive(Clone, Debug)]
pub struct Program {
    /// The node arena.
    pub nodes: Vec<Node>,
    /// List elements, contiguous per list.
    pub children: Vec<NodeId>,
    /// Top-level nodes of every module, contiguous per module.
    tops: Vec<NodeId>,
    /// Range of `tops` per module.
    modules: Vec<(u32, u32)>,
    /// Number of cells read per tag: index = kind (0 to 36); index 37 counts `0xFF` tags.
    pub cell_counts: [u32; 38],
    /// Extra (custom) option count from the world's script-table header (Sly 1).
    pub custom_options: u32,
    /// The game whose cell numbering the modules were read with.
    pub game: Game,
    /// Sly 2: (first module, module count) of each package, in package order. Empty for Sly 1.
    pub packages: Vec<(u32, u32)>,
}

/// Deepest list nesting [`Program::read_module`] accepts. Quoting a datum and printing a node recurse once per
/// level, so this bounds their stack use; the deepest nesting on the disc is far below it.
pub const MAX_NESTING: usize = 128;

/// File reader with absolute offsets.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn err(&self, what: &'static str) -> ParseError {
        ParseError { offset: self.pos, what }
    }

    fn bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or_else(|| self.err("truncated"))?;
        let b = &self.data[self.pos..end];
        self.pos = end;
        Ok(b)
    }

    fn u8(&mut self) -> Result<u8, ParseError> {
        Ok(self.bytes(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, ParseError> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32s<const N: usize>(&mut self) -> Result<[f32; N], ParseError> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = f32::from_bits(self.u32()?);
        }
        Ok(out)
    }

    fn string(&mut self) -> Result<(), ParseError> {
        let b = self.bytes(2)?;
        let n = u16::from_le_bytes([b[0], b[1]]) as usize;
        self.bytes(n).map(|_| ())
    }
}

/// A list being read: its node (none for a module's top level), its elements so far, and whether the enclosing
/// sequence continues after it.
struct Frame {
    node: Option<NodeId>,
    items: Vec<NodeId>,
    parent_more: bool,
    first: bool,
}

impl Default for Program {
    fn default() -> Self {
        Program {
            nodes: Vec::new(),
            children: Vec::new(),
            tops: Vec::new(),
            modules: Vec::new(),
            cell_counts: [0; 38],
            custom_options: 0,
            game: Game::Sly1,
            packages: Vec::new(),
        }
    }
}

impl Program {
    /// An empty Sly 1 program.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty program for `game`.
    pub fn for_game(game: Game) -> Self {
        Program { game, ..Self::default() }
    }

    /// Reads the script blocks of every package of a parsed Sly 2 world (specs/sly2/scripts.md 1): each package's
    /// block follows its four header words. Module `m` of package `p` is module `packages[p].0 + m` of the
    /// program ([`Program::package_module`]).
    pub fn from_sly2_world(world: &formats_sly2::world::World<'_>) -> Result<Program, ParseError> {
        let mut p = Program::for_game(Game::Sly2);
        for body in &world.bodies {
            let first = p.modules.len() as u32;
            let mut pos = body.offset + 16;
            for _ in 0..body.scripts {
                pos = p.read_module(world.data, pos, body.debug != 0)?;
            }
            p.packages.push((first, body.scripts));
        }
        Ok(p)
    }

    /// Reads the script blocks of every group of a parsed Sly 3 world (specs/sly3/world-file.md 4.1): as Sly 2,
    /// each group's block follows its four header words, and a record's script module indexes its own group's
    /// block (`packages` holds the groups).
    pub fn from_sly3_world(world: &formats_sly3::world::World<'_>) -> Result<Program, ParseError> {
        Self::from_sly3_bodies(world.data, &world.bodies)
    }

    /// As [`Program::from_sly3_world`], from the world's bytes and its group bodies (as an engine keeps them).
    pub fn from_sly3_bodies(data: &[u8], bodies: &[formats_sly3::world::GroupBody]) -> Result<Program, ParseError> {
        let mut p = Program::for_game(Game::Sly3);
        for body in bodies {
            let first = p.modules.len() as u32;
            let mut pos = body.offset + 16;
            for _ in 0..body.scripts {
                pos = p.read_module(data, pos, body.debug != 0)?;
            }
            p.packages.push((first, body.scripts));
        }
        Ok(p)
    }

    /// The program-wide index of module `script` of package (Sly 3: group) `package`, if it has it.
    pub fn package_module(&self, package: usize, script: i16) -> Option<u32> {
        let &(first, count) = self.packages.get(package)?;
        let m = u32::try_from(script).ok().filter(|&m| m < count)?;
        Some(first + m)
    }

    /// Reads the script table of a decompressed world, whose layout `formats` has parsed.
    pub fn from_world(data: &[u8], layout: &WorldLayout) -> Result<Program, ParseError> {
        let mut p = Program::new();
        let debug = layout.debug != 0;
        for &offset in &layout.scripts.scripts {
            p.read_module(data, offset, debug)?;
        }
        p.custom_options = layout.scripts.extra;
        Ok(p)
    }

    /// Reads a whole script table at `offset` (the three counts, the modules and, with `debug`, their strings).
    /// Returns the program and the offset just past the table.
    pub fn read_table(data: &[u8], offset: usize, debug: bool) -> Result<(Program, usize), ParseError> {
        let mut c = Cursor { data, pos: offset };
        let count = c.u32()?;
        let symbols = c.u32()?;
        let extra = c.u32()?;
        let mut p = Program::new();
        let mut pos = c.pos;
        for _ in 0..count {
            pos = p.read_module(data, pos, debug)?;
            if debug {
                let mut c = Cursor { data, pos };
                for _ in 0..3 {
                    c.string()?;
                }
                pos = c.pos;
            }
        }
        if debug {
            let mut c = Cursor { data, pos };
            for _ in 0..(symbols as u64 + extra as u64) {
                c.string()?;
            }
            pos = c.pos;
        }
        p.custom_options = extra;
        Ok((p, pos))
    }

    /// Reads one module (a chain of cells) at `offset` and appends it. Returns the offset just past it.
    pub fn read_module(&mut self, data: &[u8], offset: usize, debug: bool) -> Result<usize, ParseError> {
        let mut c = Cursor { data, pos: offset };
        let mut stack: Vec<Frame> = Vec::new();
        stack.push(Frame { node: None, items: Vec::new(), parent_more: false, first: true });
        loop {
            let tag = c.u8()?;
            let frame = stack.last_mut().expect("frame");
            let first = core::mem::replace(&mut frame.first, false);
            let more;
            if tag == kind::EMPTY_TAG {
                self.cell_counts[37] += 1;
                // An empty node as the first cell of a list makes the list empty; elsewhere it is an empty element.
                if !first {
                    let id = self.push(Node::Nil);
                    stack.last_mut().expect("frame").items.push(id);
                }
                more = false;
            } else {
                let k = tag & 0x7F;
                more = tag & 0x80 != 0;
                if k > kind::LAST {
                    return Err(ParseError { offset: c.pos - 1, what: "cell kind above 36" });
                }
                self.cell_counts[k as usize] += 1;
                if debug {
                    c.bytes(self.game.debug_bytes())?;
                }
                let node = match self.game.cell(k) {
                    Cell::Nil => Node::Nil,
                    Cell::Keyword(kw) => Node::Keyword(kw),
                    Cell::Int => Node::Int(c.u32()? as i32),
                    Cell::Float => Node::Float(f32::from_bits(c.u32()?)),
                    Cell::Vector => Node::Vector(c.f32s()?),
                    Cell::Matrix => Node::Matrix(Box::new(c.f32s()?)),
                    Cell::Coeffs => Node::Coeffs(c.f32s()?),
                    Cell::Range => Node::Range(c.f32s()?),
                    Cell::Smoothing => Node::Smoothing(c.f32s()?),
                    Cell::Bool => Node::Bool(c.u32()? != 0),
                    Cell::Symbol => Node::Symbol(c.u32()?),
                    Cell::Builtin => Node::Builtin(c.u32()?),
                    Cell::Object => Node::Object(c.u32()?),
                    Cell::List => {
                        if stack.len() > MAX_NESTING {
                            return Err(ParseError { offset: c.pos - 1, what: "lists nested too deeply" });
                        }
                        let id = self.push(Node::List { start: 0, len: 0 });
                        stack.last_mut().expect("frame").items.push(id);
                        stack.push(Frame { node: Some(id), items: Vec::new(), parent_more: more, first: true });
                        continue;
                    }
                    Cell::Other => Node::Other(k),
                };
                let id = self.push(node);
                stack.last_mut().expect("frame").items.push(id);
            }
            if more {
                continue;
            }
            // The current sequence ended: close lists until one whose parent sequence continues.
            loop {
                let f = stack.pop().expect("frame");
                match f.node {
                    None => {
                        let start = self.tops.len() as u32;
                        self.tops.extend_from_slice(&f.items);
                        self.modules.push((start, f.items.len() as u32));
                        return Ok(c.pos);
                    }
                    Some(id) => {
                        let start = self.children.len() as u32;
                        self.children.extend_from_slice(&f.items);
                        self.nodes[id as usize] = Node::List { start, len: f.items.len() as u32 };
                        if f.parent_more {
                            break;
                        }
                    }
                }
            }
        }
    }

    fn push(&mut self, n: Node) -> NodeId {
        self.nodes.push(n);
        (self.nodes.len() - 1) as NodeId
    }

    /// Number of modules.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// The top-level nodes of module `i` (one `begin` list for every module on the disc).
    pub fn module(&self, i: usize) -> &[NodeId] {
        let (s, n) = self.modules[i];
        &self.tops[s as usize..(s + n) as usize]
    }

    /// The node `id`.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    /// The elements of list node `id` (empty for anything else).
    pub fn list(&self, id: NodeId) -> &[NodeId] {
        match self.nodes[id as usize] {
            Node::List { start, len } => &self.children[start as usize..(start + len) as usize],
            _ => &[],
        }
    }

    /// The keyword heading list `id`, if any.
    pub fn head_keyword(&self, id: NodeId) -> Option<u8> {
        match self.list(id).first().map(|&h| self.node(h)) {
            Some(Node::Keyword(k)) => Some(*k),
            _ => None,
        }
    }

    /// The module-level definitions of module `i`: `(symbol, value node)` for each `(define symbol expr)` directly
    /// inside the module's top-level `begin` (or at its top level).
    pub fn definitions(&self, i: usize) -> Vec<(u32, NodeId)> {
        let mut out = Vec::new();
        for &top in self.module(i) {
            let items: &[NodeId] =
                if self.head_keyword(top) == Some(kind::BEGIN) { &self.list(top)[1..] } else { &[top] };
            for &d in items {
                let l = self.list(d);
                if self.head_keyword(d) == Some(kind::DEFINE)
                    && l.len() >= 3
                    && let Node::Symbol(s) = *self.node(l[1])
                {
                    out.push((s, l[2]));
                }
            }
        }
        out
    }
}

/// Writes s-expressions in the notation of scripts.md section 10 (`sN` symbols, `opN` built-ins).
pub struct Display<'a> {
    program: &'a Program,
    node: NodeId,
}

impl Program {
    /// A printable form of node `id`.
    pub fn display(&self, id: NodeId) -> Display<'_> {
        Display { program: self, node: id }
    }
}

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let p = self.program;
        match p.node(self.node) {
            Node::Nil => f.write_str("()"),
            Node::Int(i) => write!(f, "{i}"),
            Node::Float(x) => write!(f, "{x:?}"),
            Node::Vector(v) | Node::Coeffs(v) | Node::Smoothing(v) => write!(f, "#({:?} {:?} {:?})", v[0], v[1], v[2]),
            Node::Matrix(_) => f.write_str("#matrix"),
            Node::Range(r) => write!(f, "#({:?} {:?})", r[0], r[1]),
            Node::Bool(b) => f.write_str(if *b { "#t" } else { "#f" }),
            Node::Symbol(s) => write!(f, "s{s}"),
            Node::Builtin(o) => write!(f, "op{o}"),
            Node::Object(0) => f.write_str("null-object"),
            Node::Object(o) => write!(f, "object{o}"),
            Node::Keyword(k) => f.write_str(keyword_name(*k)),
            Node::Other(k) => write!(f, "kind{k}"),
            Node::List { .. } => {
                f.write_str("(")?;
                for (i, &c) in p.list(self.node).iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{}", p.display(c))?;
                }
                f.write_str(")")
            }
        }
    }
}

/// The name used when printing keyword `k`.
pub fn keyword_name(k: u8) -> &'static str {
    match k {
        kind::SET => "set",
        kind::DEFINE => "define",
        kind::ASSERT => "assert",
        kind::IF => "if",
        kind::OR => "or",
        kind::AND => "and",
        kind::COND => "cond",
        kind::ELSE => "else",
        kind::CASE => "case",
        kind::LET => "let",
        kind::WHILE => "while",
        kind::LAMBDA => "lambda",
        kind::BEGIN => "begin",
        kind::QUOTE => "quote",
        kind::REST => "rest",
        kind::EVAL_ONE => "eval-one",
        kind::NO_VALUE => "no-value",
        _ => "keyword?",
    }
}
