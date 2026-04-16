use crate::obarray::{self, SymbolId};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Global counter for total cons cell allocations. Monotonically increasing.
static GLOBAL_CONS_COUNT: AtomicU64 = AtomicU64::new(0);

/// Read the global cons allocation counter.
pub fn global_cons_count() -> u64 {
    GLOBAL_CONS_COUNT.load(Ordering::Relaxed)
}

/// Shared mutable cell used for cons, vector, and hash table mutation semantics.
pub type ConsCell = Arc<Mutex<(LispObject, LispObject)>>;
pub type SharedVec = Arc<Mutex<Vec<LispObject>>>;
pub type SharedHashTable = Arc<Mutex<LispHashTable>>;

/// Hash table test function type.
#[derive(Debug, Clone, PartialEq)]
pub enum HashTableTest {
    Eq,
    Eql,
    Equal,
}

#[derive(Debug, Clone)]
pub enum LispObject {
    Nil,
    T,
    Symbol(SymbolId),
    Integer(i64),
    Float(f64),
    String(String),
    Cons(ConsCell),
    Primitive(String),
    Vector(SharedVec),
    BytecodeFn(BytecodeFunction),
    HashTable(SharedHashTable),
}

impl PartialEq for LispObject {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispObject::Nil, LispObject::Nil) => true,
            (LispObject::T, LispObject::T) => true,
            (LispObject::Symbol(a), LispObject::Symbol(b)) => a == b,
            (LispObject::Integer(a), LispObject::Integer(b)) => a == b,
            (LispObject::Float(a), LispObject::Float(b)) => a == b,
            (LispObject::String(a), LispObject::String(b)) => a == b,
            (LispObject::Cons(a), LispObject::Cons(b)) => *a.lock() == *b.lock(),
            (LispObject::Primitive(a), LispObject::Primitive(b)) => a == b,
            (LispObject::Vector(a), LispObject::Vector(b)) => *a.lock() == *b.lock(),
            (LispObject::BytecodeFn(a), LispObject::BytecodeFn(b)) => a == b,
            (LispObject::HashTable(a), LispObject::HashTable(b)) => a.lock().test == b.lock().test,
            _ => false,
        }
    }
}

/// An Emacs-style hash table.
#[derive(Debug, Clone)]
pub struct LispHashTable {
    pub test: HashTableTest,
    pub data: std::collections::HashMap<HashKey, LispObject>,
}

/// Wrapper for hash table keys that implements Hash + Eq.
#[derive(Debug, Clone)]
pub enum HashKey {
    Symbol(SymbolId),
    Integer(i64),
    String(String),
    /// For 'equal test: use prin1 representation as key
    Printed(String),
    /// For 'eq test on non-immediate types: pointer identity
    Identity(usize),
}

impl PartialEq for LispHashTable {
    fn eq(&self, other: &Self) -> bool {
        if self.test != other.test || self.data.len() != other.data.len() {
            return false;
        }
        for (key, val) in &self.data {
            match other.data.get(key) {
                Some(other_val) if val == other_val => {}
                _ => return false,
            }
        }
        true
    }
}

impl std::hash::Hash for HashKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            HashKey::Symbol(s) => {
                state.write_u8(0);
                s.hash(state);
            }
            HashKey::Integer(i) => {
                state.write_u8(1);
                i.hash(state);
            }
            HashKey::String(s) => {
                state.write_u8(2);
                s.hash(state);
            }
            HashKey::Printed(s) => {
                state.write_u8(3);
                s.hash(state);
            }
            HashKey::Identity(addr) => {
                state.write_u8(4);
                addr.hash(state);
            }
        }
    }
}

impl PartialEq for HashKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HashKey::Symbol(a), HashKey::Symbol(b)) => a == b,
            (HashKey::Integer(a), HashKey::Integer(b)) => a == b,
            (HashKey::String(a), HashKey::String(b)) => a == b,
            (HashKey::Printed(a), HashKey::Printed(b)) => a == b,
            (HashKey::Identity(a), HashKey::Identity(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for HashKey {}

impl LispHashTable {
    pub fn new(test: HashTableTest) -> Self {
        LispHashTable {
            test,
            data: std::collections::HashMap::new(),
        }
    }

    pub fn make_key(&self, obj: &LispObject) -> HashKey {
        match &self.test {
            HashTableTest::Eq => match obj {
                // eq: identity for all non-immediate types
                LispObject::Nil => HashKey::Symbol(obarray::intern("nil")),
                LispObject::T => HashKey::Symbol(obarray::intern("t")),
                LispObject::Symbol(id) => HashKey::Symbol(*id),
                LispObject::Integer(i) => HashKey::Integer(*i),
                // Strings, cons, vectors: use pointer identity
                LispObject::Cons(cell) => HashKey::Identity(std::sync::Arc::as_ptr(cell) as usize),
                LispObject::Vector(v) => HashKey::Identity(std::sync::Arc::as_ptr(v) as usize),
                LispObject::HashTable(ht) => HashKey::Identity(std::sync::Arc::as_ptr(ht) as usize),
                _ => HashKey::Printed(obj.prin1_to_string()),
            },
            HashTableTest::Eql => match obj {
                // eql: value equality for numbers, identity for rest
                LispObject::Symbol(id) => HashKey::Symbol(*id),
                LispObject::Integer(i) => HashKey::Integer(*i),
                LispObject::Float(f) => HashKey::Printed(format!("{:?}", f)),
                LispObject::String(s) => HashKey::String(s.clone()),
                _ => HashKey::Printed(obj.prin1_to_string()),
            },
            HashTableTest::Equal => HashKey::Printed(obj.prin1_to_string()),
        }
    }

    pub fn get(&self, key: &LispObject) -> Option<&LispObject> {
        let k = self.make_key(key);
        self.data.get(&k)
    }

    pub fn put(&mut self, key: &LispObject, value: LispObject) {
        let k = self.make_key(key);
        self.data.insert(k, value);
    }

    pub fn remove(&mut self, key: &LispObject) -> bool {
        let k = self.make_key(key);
        self.data.remove(&k).is_some()
    }
}

/// A compiled bytecode function object.
/// Corresponds to Emacs #[arglist bytecode constants maxdepth ...] literals.
#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeFunction {
    /// Packed argument descriptor: min_args + (max_args << 8), bit 7 of high byte = &rest
    pub argdesc: i64,
    /// The bytecode as raw bytes
    pub bytecode: Vec<u8>,
    /// Constants vector referenced by index
    pub constants: Vec<LispObject>,
    /// Maximum operand stack depth
    pub maxdepth: usize,
    /// Optional docstring
    pub docstring: Option<String>,
    /// Optional interactive spec
    pub interactive: Option<Box<LispObject>>,
}

impl BytecodeFunction {
    pub fn min_args(&self) -> usize {
        (self.argdesc & 0x7F) as usize
    }

    pub fn max_args(&self) -> usize {
        let max = ((self.argdesc >> 8) & 0x7F) as usize;
        if self.has_rest() {
            usize::MAX
        } else {
            max
        }
    }

    pub fn has_rest(&self) -> bool {
        (self.argdesc >> 7) & 1 == 1 || (self.argdesc >> 15) & 1 == 1
    }
}

impl LispObject {
    pub fn nil() -> Self {
        LispObject::Nil
    }

    pub fn t() -> Self {
        LispObject::T
    }

    pub fn symbol(name: &str) -> Self {
        LispObject::Symbol(obarray::intern(name))
    }

    pub fn cons(car: LispObject, cdr: LispObject) -> Self {
        GLOBAL_CONS_COUNT.fetch_add(1, Ordering::Relaxed);
        LispObject::Cons(Arc::new(Mutex::new((car, cdr))))
    }

    pub fn integer(i: i64) -> Self {
        LispObject::Integer(i)
    }

    pub fn float(f: f64) -> Self {
        LispObject::Float(f)
    }

    pub fn string(s: &str) -> Self {
        LispObject::String(s.to_string())
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, LispObject::Nil)
    }

    pub fn is_t(&self) -> bool {
        matches!(self, LispObject::T)
    }

    pub fn is_symbol(&self) -> bool {
        matches!(self, LispObject::Symbol(_))
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, LispObject::Integer(_))
    }

    pub fn is_float(&self) -> bool {
        matches!(self, LispObject::Float(_))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, LispObject::String(_))
    }

    pub fn is_cons(&self) -> bool {
        matches!(self, LispObject::Cons(_))
    }

    pub fn car(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(cell) => Some(cell.lock().0.clone()),
            _ => None,
        }
    }

    pub fn cdr(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(cell) => Some(cell.lock().1.clone()),
            _ => None,
        }
    }

    pub fn set_car(&self, val: LispObject) {
        if let LispObject::Cons(cell) = self {
            cell.lock().0 = val;
        }
    }

    pub fn set_cdr(&self, val: LispObject) {
        if let LispObject::Cons(cell) = self {
            cell.lock().1 = val;
        }
    }

    /// Returns the symbol name as an owned String, or None if not a symbol.
    pub fn as_symbol(&self) -> Option<String> {
        match self {
            LispObject::Symbol(id) => Some(obarray::symbol_name(*id)),
            _ => None,
        }
    }

    /// Returns the SymbolId if this is a Symbol variant.
    pub fn as_symbol_id(&self) -> Option<SymbolId> {
        match self {
            LispObject::Symbol(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            LispObject::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            LispObject::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            LispObject::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn destructure(self) -> (LispObject, LispObject) {
        match self {
            LispObject::Cons(cell) => {
                let b = cell.lock();
                (b.0.clone(), b.1.clone())
            }
            _ => (LispObject::Nil, LispObject::Nil),
        }
    }

    pub fn destructure_cons(&self) -> Option<(LispObject, LispObject)> {
        match self {
            LispObject::Cons(cell) => {
                let b = cell.lock();
                Some((b.0.clone(), b.1.clone()))
            }
            _ => None,
        }
    }

    pub fn first(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(cell) => Some(cell.lock().0.clone()),
            _ => None,
        }
    }

    pub fn rest(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(cell) => Some(cell.lock().1.clone()),
            _ => None,
        }
    }

    pub fn nth(&self, n: usize) -> Option<LispObject> {
        let mut current = self.clone();
        for _ in 0..n {
            current = current.rest()?;
        }
        current.first()
    }

    pub fn lambda_expr(args: LispObject, body: LispObject) -> LispObject {
        LispObject::cons(LispObject::symbol("lambda"), LispObject::cons(args, body))
    }

    pub fn primitive(name: &str) -> LispObject {
        LispObject::Primitive(name.to_string())
    }

    pub fn is_primitive(&self) -> bool {
        matches!(self, LispObject::Primitive(_))
    }

    pub fn as_primitive(&self) -> Option<&String> {
        match self {
            LispObject::Primitive(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_quote_content(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(cell) => {
                let b = cell.lock();
                if let LispObject::Symbol(id) = &b.0 {
                    if obarray::symbol_name(*id) == "quote" {
                        return b.1.first();
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Print in Lisp-readable form (like prin1).
    pub fn prin1_to_string(&self) -> String {
        match self {
            LispObject::Nil => "nil".to_string(),
            LispObject::T => "t".to_string(),
            LispObject::Symbol(id) => obarray::symbol_name(*id),
            LispObject::Integer(i) => i.to_string(),
            LispObject::Float(f) => {
                let s = f.to_string();
                if s.contains('.') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            LispObject::String(s) => {
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\t', "\\t");
                format!("\"{}\"", escaped)
            }
            LispObject::Cons(_) => {
                let mut parts = Vec::new();
                let mut current = self.clone();
                while let Some((car, cdr)) = current.destructure_cons() {
                    parts.push(car.prin1_to_string());
                    current = cdr;
                }
                if !current.is_nil() {
                    parts.push(".".to_string());
                    parts.push(current.prin1_to_string());
                }
                format!("({})", parts.join(" "))
            }
            LispObject::Primitive(name) => format!("#<subr {}>", name),
            LispObject::Vector(v) => {
                let v = v.lock();
                let parts: Vec<String> = v.iter().map(|e| e.prin1_to_string()).collect();
                format!("[{}]", parts.join(" "))
            }
            LispObject::BytecodeFn(bc) => {
                format!("#<bytecode {:p}>", bc as *const _)
            }
            LispObject::HashTable(ht) => {
                let ht = ht.lock();
                format!("#<hash-table count {} test {:?}>", ht.data.len(), ht.test)
            }
        }
    }

    /// Print in human-readable form (like princ). Strings without quotes.
    pub fn princ_to_string(&self) -> String {
        match self {
            LispObject::String(s) => s.clone(),
            other => other.prin1_to_string(),
        }
    }
}
