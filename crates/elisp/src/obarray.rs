use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Index into the global symbol table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

/// Flags associated with a symbol.
#[derive(Debug, Clone, Copy, Default)]
pub struct SymbolFlags {
    pub special: bool,
    pub constant: bool,
}

/// Data for an interned symbol.
pub struct SymbolData {
    pub name: String,
    pub flags: SymbolFlags,
}

/// The global symbol table (obarray).
pub struct SymbolTable {
    symbols: Vec<SymbolData>,
    name_to_id: HashMap<String, SymbolId>,
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolTable {
    pub fn new() -> Self {
        let mut table = SymbolTable {
            symbols: Vec::new(),
            name_to_id: HashMap::new(),
        };
        // Pre-intern common symbols
        table.intern("nil");
        table.intern("t");
        table
    }

    pub fn intern(&mut self, name: &str) -> SymbolId {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(SymbolData {
            name: name.to_string(),
            flags: SymbolFlags::default(),
        });
        self.name_to_id.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: SymbolId) -> &str {
        &self.symbols[id.0 as usize].name
    }

    pub fn flags(&self, id: SymbolId) -> &SymbolFlags {
        &self.symbols[id.0 as usize].flags
    }

    pub fn flags_mut(&mut self, id: SymbolId) -> &mut SymbolFlags {
        &mut self.symbols[id.0 as usize].flags
    }

    pub fn find(&self, name: &str) -> Option<SymbolId> {
        self.name_to_id.get(name).copied()
    }
}

/// The global obarray shared by all interpreter instances.
pub static GLOBAL_OBARRAY: LazyLock<RwLock<SymbolTable>> =
    LazyLock::new(|| RwLock::new(SymbolTable::new()));

/// Intern a symbol name in the global obarray, returning its ID.
pub fn intern(name: &str) -> SymbolId {
    GLOBAL_OBARRAY.write().intern(name)
}

/// Look up the name for a symbol ID.
pub fn symbol_name(id: SymbolId) -> String {
    GLOBAL_OBARRAY.read().name(id).to_string()
}
