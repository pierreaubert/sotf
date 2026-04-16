/// Hash table test function type.
#[derive(Debug, Clone, PartialEq)]
pub enum HashTableTest {
    Eq,
    Eql,
    Equal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LispObject {
    Nil,
    T,
    Symbol(String),
    Integer(i64),
    Float(f64),
    String(String),
    Cons(Box<LispObject>, Box<LispObject>),
    Primitive(String),
    Vector(Vec<LispObject>),
    BytecodeFn(BytecodeFunction),
    HashTable(Box<LispHashTable>),
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
    Symbol(String),
    Integer(i64),
    String(String),
    /// For 'equal test: use prin1 representation as key
    Printed(String),
}

impl PartialEq for LispHashTable {
    fn eq(&self, other: &Self) -> bool {
        self.test == other.test && self.data.len() == other.data.len()
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
            HashTableTest::Eq | HashTableTest::Eql => match obj {
                LispObject::Symbol(s) => HashKey::Symbol(s.clone()),
                LispObject::Integer(i) => HashKey::Integer(*i),
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
        LispObject::Symbol(name.to_string())
    }

    pub fn cons(car: LispObject, cdr: LispObject) -> Self {
        LispObject::Cons(Box::new(car), Box::new(cdr))
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
        matches!(self, LispObject::Cons(_, _))
    }

    pub fn car(&self) -> Option<&LispObject> {
        match self {
            LispObject::Cons(car, _) => Some(car),
            _ => None,
        }
    }

    pub fn cdr(&self) -> Option<&LispObject> {
        match self {
            LispObject::Cons(_, cdr) => Some(cdr),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<&String> {
        match self {
            LispObject::Symbol(s) => Some(s),
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
            LispObject::Cons(car, cdr) => (*car, *cdr),
            _ => (LispObject::Nil, LispObject::Nil),
        }
    }

    pub fn destructure_cons(&self) -> Option<(LispObject, LispObject)> {
        match self {
            LispObject::Cons(car, cdr) => Some(((**car).clone(), (**cdr).clone())),
            _ => None,
        }
    }

    pub fn first(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(car, _) => Some((**car).clone()),
            _ => None,
        }
    }

    pub fn rest(&self) -> Option<LispObject> {
        match self {
            LispObject::Cons(_, cdr) => Some((**cdr).clone()),
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
            LispObject::Cons(car, cdr) => {
                if let LispObject::Symbol(s) = &**car {
                    if s == "quote" {
                        return cdr.first();
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
            LispObject::Symbol(s) => s.clone(),
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
            LispObject::Cons(_, _) => {
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
                let parts: Vec<String> = v.iter().map(|e| e.prin1_to_string()).collect();
                format!("[{}]", parts.join(" "))
            }
            LispObject::BytecodeFn(bc) => {
                format!("#<bytecode {:p}>", bc as *const _)
            }
            LispObject::HashTable(ht) => {
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
