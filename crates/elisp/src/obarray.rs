use crate::object::LispObject;
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
    /// Value cell. Populated by Phase 1b; unused by eval/vm in Phase 1a.
    pub value: Option<LispObject>,
    /// Function cell. Populated by Phase 1b; unused by eval/vm in Phase 1a.
    pub function: Option<LispObject>,
    /// Property list as (prop, value) pairs. Ordered; first match wins.
    pub plist: Vec<(SymbolId, LispObject)>,
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
            value: None,
            function: None,
            plist: Vec::new(),
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

    /// Look up a property on the symbol's plist. Returns nil if absent.
    pub fn get_plist(&self, sym: SymbolId, prop: SymbolId) -> LispObject {
        let data = &self.symbols[sym.0 as usize];
        for (k, v) in &data.plist {
            if *k == prop {
                return v.clone();
            }
        }
        LispObject::Nil
    }

    /// Insert or replace a property on the symbol's plist.
    pub fn put_plist(&mut self, sym: SymbolId, prop: SymbolId, value: LispObject) {
        let data = &mut self.symbols[sym.0 as usize];
        for entry in data.plist.iter_mut() {
            if entry.0 == prop {
                entry.1 = value;
                return;
            }
        }
        data.plist.push((prop, value));
    }

    /// Return the plist as a freshly-built cons list (prop val prop val ...).
    pub fn full_plist(&self, sym: SymbolId) -> LispObject {
        let data = &self.symbols[sym.0 as usize];
        let mut result = LispObject::Nil;
        for (k, v) in data.plist.iter().rev() {
            result = LispObject::cons(v.clone(), result);
            result = LispObject::cons(LispObject::Symbol(*k), result);
        }
        result
    }

    pub fn set_value_cell(&mut self, sym: SymbolId, val: LispObject) {
        self.symbols[sym.0 as usize].value = Some(val);
    }

    pub fn get_value_cell(&self, sym: SymbolId) -> Option<LispObject> {
        self.symbols[sym.0 as usize].value.clone()
    }

    pub fn set_function_cell(&mut self, sym: SymbolId, val: LispObject) {
        self.symbols[sym.0 as usize].function = Some(val);
    }

    pub fn get_function_cell(&self, sym: SymbolId) -> Option<LispObject> {
        self.symbols[sym.0 as usize].function.clone()
    }
}

/// The global obarray shared by all interpreter instances.
pub static GLOBAL_OBARRAY: LazyLock<RwLock<SymbolTable>> =
    LazyLock::new(|| RwLock::new(SymbolTable::new()));

/// Intern a symbol name in the global obarray, returning its ID.
///
/// Read-lock fast-path: most interns hit an already-present symbol,
/// so we try a read first and only upgrade to a write-lock if needed.
pub fn intern(name: &str) -> SymbolId {
    if let Some(id) = GLOBAL_OBARRAY.read().find(name) {
        return id;
    }
    GLOBAL_OBARRAY.write().intern(name)
}

/// Look up the name for a symbol ID.
pub fn symbol_name(id: SymbolId) -> String {
    GLOBAL_OBARRAY.read().name(id).to_string()
}

/// Look up a property on a symbol's plist.
pub fn get_plist(sym: SymbolId, prop: SymbolId) -> LispObject {
    GLOBAL_OBARRAY.read().get_plist(sym, prop)
}

/// Insert or replace a property on a symbol's plist.
pub fn put_plist(sym: SymbolId, prop: SymbolId, value: LispObject) {
    GLOBAL_OBARRAY.write().put_plist(sym, prop, value);
}

/// Return the full plist as a freshly-built (prop val ...) cons list.
pub fn full_plist(sym: SymbolId) -> LispObject {
    GLOBAL_OBARRAY.read().full_plist(sym)
}

/// Read the symbol's value cell. None if unbound.
pub fn get_value_cell(sym: SymbolId) -> Option<LispObject> {
    GLOBAL_OBARRAY.read().get_value_cell(sym)
}

/// Write the symbol's value cell.
pub fn set_value_cell(sym: SymbolId, value: LispObject) {
    GLOBAL_OBARRAY.write().set_value_cell(sym, value);
}

/// Read the symbol's function cell. None if the symbol has no function definition.
pub fn get_function_cell(sym: SymbolId) -> Option<LispObject> {
    GLOBAL_OBARRAY.read().get_function_cell(sym)
}

/// Write the symbol's function cell.
pub fn set_function_cell(sym: SymbolId, value: LispObject) {
    GLOBAL_OBARRAY.write().set_function_cell(sym, value);
}

/// Look up the flags for a symbol.
pub fn get_flags(sym: SymbolId) -> SymbolFlags {
    *GLOBAL_OBARRAY.read().flags(sym)
}

/// Mark a symbol as special (dynamically bound).
pub fn mark_special(sym: SymbolId) {
    GLOBAL_OBARRAY.write().flags_mut(sym).special = true;
}

#[cfg(test)]
pub fn clear_plist_for_tests(sym: SymbolId) {
    GLOBAL_OBARRAY.write().symbols[sym.0 as usize].plist.clear();
}
