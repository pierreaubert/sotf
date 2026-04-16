use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Environment {
    bindings: HashMap<String, LispObject>,
    parent: Option<Arc<Environment>>,
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub args: LispObject,
    pub body: LispObject,
}

type MacroTable = Arc<RwLock<HashMap<String, Macro>>>;
type PlistTable = Arc<RwLock<HashMap<String, LispObject>>>;
type FeatureList = Arc<RwLock<Vec<String>>>;

const MAX_EVAL_DEPTH: usize = 1000;

thread_local! {
    static EVAL_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn inc_eval_depth() -> Result<usize, ElispError> {
    EVAL_DEPTH.with(|d| {
        let new_depth = d.get() + 1;
        if new_depth > MAX_EVAL_DEPTH {
            Err(ElispError::StackOverflow)
        } else {
            d.set(new_depth);
            Ok(new_depth)
        }
    })
}

fn dec_eval_depth() {
    EVAL_DEPTH.with(|d| {
        d.set(d.get().saturating_sub(1));
    });
}

macro_rules! eval_next {
    ($expr:expr, $env:expr, $editor:expr, $macros:expr, $state:expr) => {{
        inc_eval_depth()?;
        let result = eval($expr, $env, $editor, $macros, $state);
        dec_eval_depth();
        result
    }};
}

/// Returns true when `obj` is something that can appear in function position.
fn is_callable_value(obj: &LispObject) -> bool {
    match obj {
        LispObject::Primitive(_) | LispObject::BytecodeFn(_) => true,
        LispObject::Cons(cell) => {
            let b = cell.lock();
            if let LispObject::Symbol(id) = &b.0 {
                crate::obarray::symbol_name(*id) == "lambda"
            } else {
                false
            }
        }
        _ => false,
    }
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            bindings: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(parent: Arc<Environment>) -> Self {
        Environment {
            bindings: HashMap::new(),
            parent: Some(parent),
        }
    }

    pub fn get(&self, name: &str) -> Option<LispObject> {
        self.bindings
            .get(name)
            .cloned()
            .or_else(|| self.parent.as_ref().and_then(|p| p.get(name)))
    }

    /// Lisp-2 function lookup: find a callable value for `name`, skipping
    /// non-callable local shadows (e.g. a parameter named `list` that
    /// hides the built-in `list` function).  Returns the first callable
    /// binding found, or the first binding of any kind if no callable
    /// binding exists.
    pub fn get_function(&self, name: &str) -> Option<LispObject> {
        let mut first_found: Option<LispObject> = None;
        // Check this level
        if let Some(val) = self.bindings.get(name).cloned() {
            if is_callable_value(&val) {
                return Some(val);
            }
            if first_found.is_none() {
                first_found = Some(val);
            }
        }
        // Walk parent chain
        let mut parent = self.parent.as_ref();
        while let Some(p) = parent {
            if let Some(val) = p.bindings.get(name).cloned() {
                if is_callable_value(&val) {
                    return Some(val);
                }
                if first_found.is_none() {
                    first_found = Some(val);
                }
            }
            parent = p.parent.as_ref();
        }
        first_found
    }

    pub fn set(&mut self, name: &str, value: LispObject) {
        self.bindings.insert(name.to_string(), value);
    }

    pub fn define(&mut self, name: &str, value: LispObject) {
        self.bindings.insert(name.to_string(), value);
    }
}

/// Dynamic binding stack entry: (variable name, previous value or None if unbound).
type Specpdl = Arc<RwLock<Vec<(String, Option<LispObject>)>>>;
/// Set of variable names declared special via `defvar`/`defconst`.
type SpecialVars = Arc<RwLock<HashSet<String>>>;

/// Shared interpreter state accessible during evaluation.
#[derive(Clone)]
pub struct InterpreterState {
    pub plists: PlistTable,
    pub features: FeatureList,
    pub profiler: Arc<RwLock<crate::jit::Profiler>>,
    #[cfg(feature = "jit")]
    pub jit: Arc<RwLock<crate::jit::JitCompiler>>,
    /// Variables declared special (dynamically bound) via `defvar`/`defconst`.
    pub special_vars: SpecialVars,
    /// Dynamic binding stack — saves/restores old values of special variables.
    pub specpdl: Specpdl,
    /// The root (global) environment. Special variables are always read/written here.
    pub global_env: Arc<RwLock<Environment>>,
    /// Garbage-collected heap for cons cell allocation.
    pub heap: Arc<parking_lot::Mutex<crate::gc::Heap>>,
    /// Counter for total cons cell allocations (monotonically increasing).
    pub cons_count: Arc<std::sync::atomic::AtomicU64>,
}

pub struct Interpreter {
    env: Arc<RwLock<Environment>>,
    editor: Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: MacroTable,
    pub state: InterpreterState,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Environment::new();
        env.define("nil", LispObject::nil());
        env.define("t", LispObject::t());

        // Standard special variables (always dynamically bound).
        let special_vars: HashSet<String> = [
            "load-path",
            "features",
            "standard-output",
            "standard-input",
            "print-escape-newlines",
            "print-length",
            "print-level",
            "debug-on-error",
            "inhibit-quit",
            "case-fold-search",
            "default-directory",
            "buffer-file-name",
            "last-command",
            "this-command",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let env = Arc::new(RwLock::new(env));
        Interpreter {
            env: env.clone(),
            editor: Arc::new(RwLock::new(None)),
            macros: Arc::new(RwLock::new(HashMap::new())),
            state: InterpreterState {
                plists: Arc::new(RwLock::new(HashMap::new())),
                features: Arc::new(RwLock::new(Vec::new())),
                profiler: Arc::new(RwLock::new(crate::jit::Profiler::new(1000))),
                #[cfg(feature = "jit")]
                jit: Arc::new(RwLock::new(crate::jit::JitCompiler::new())),
                special_vars: Arc::new(RwLock::new(special_vars)),
                specpdl: Arc::new(RwLock::new(Vec::new())),
                global_env: env,
                heap: Arc::new(parking_lot::Mutex::new(crate::gc::Heap::new())),
                cons_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            },
        }
    }

    /// Public API: evaluate a LispObject expression, returning a LispObject.
    /// Converts at the boundary to/from the internal Value representation.
    pub fn eval(&self, expr: LispObject) -> ElispResult<LispObject> {
        let val = obj_to_value(expr);
        let result = eval(val, &self.env, &self.editor, &self.macros, &self.state)?;
        Ok(value_to_obj(result))
    }

    pub fn define(&self, name: &str, value: LispObject) {
        let mut env = self.env.write();
        env.define(name, value);
    }

    pub fn set_editor(&self, editor: Box<dyn EditorCallbacks>) {
        let mut e = self.editor.write();
        *e = Some(editor);
    }

    /// Evaluate all forms in a source string. Returns the result of the last form,
    /// or the first error encountered (with the count of successful forms).
    pub fn eval_source(&self, source: &str) -> Result<LispObject, (usize, ElispError)> {
        let forms = crate::read_all(source).map_err(|e| (0, e))?;
        let mut result = LispObject::nil();
        for (i, form) in forms.into_iter().enumerate() {
            result = self.eval(form).map_err(|e| (i, e))?;
        }
        Ok(result)
    }

    /// Evaluate a Value expression directly (internal Value representation).
    pub fn eval_value(&self, expr: Value) -> ElispResult<Value> {
        eval(expr, &self.env, &self.editor, &self.macros, &self.state)
    }

    /// Evaluate all forms in a source string and return a Value.
    pub fn eval_source_value(&self, source: &str) -> Result<Value, (usize, ElispError)> {
        let forms = crate::read_all(source).map_err(|e| (0, e))?;
        let mut result = Value::nil();
        for (i, form) in forms.into_iter().enumerate() {
            let val = obj_to_value(form);
            result = eval(val, &self.env, &self.editor, &self.macros, &self.state)
                .map_err(|e| (i, e))?;
        }
        Ok(result)
    }

    /// Get a variable's value, or None if unbound.
    pub fn get(&self, name: &str) -> Option<LispObject> {
        self.env.read().get(name)
    }

    /// Returns `(total_calls, hot_functions_count)` from the JIT profiler.
    pub fn profiler_stats(&self) -> (u64, u64) {
        let profiler = self.state.profiler.read();
        (profiler.total_calls(), profiler.hot_function_count())
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

fn eval(
    expr: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    inc_eval_depth()?;
    let result = eval_inner(expr, env, editor, macros, state);
    dec_eval_depth();
    result
}

fn eval_inner(
    expr: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // Self-evaluating immediates
    if expr.is_fixnum() || expr.is_float() || expr.is_nil() || expr.is_t() {
        return Ok(expr);
    }

    // Symbol lookup
    if let Some(id) = expr.as_symbol_id() {
        let name = crate::obarray::symbol_name(crate::obarray::SymbolId(id));
        if name.starts_with(':') {
            return Ok(expr);
        }
        if state.special_vars.read().contains(&name) {
            let global = state.global_env.read();
            return global
                .get(&name)
                .map(obj_to_value)
                .ok_or(ElispError::VoidVariable(name));
        } else {
            let env = env.read();
            return env
                .get(&name)
                .map(obj_to_value)
                .ok_or(ElispError::VoidVariable(name));
        }
    }

    // Convert to LispObject for structural dispatch
    let expr_obj = value_to_obj(expr);
    match &expr_obj {
        // Self-evaluating heap types
        LispObject::String(_)
        | LispObject::Primitive(_)
        | LispObject::Vector(_)
        | LispObject::BytecodeFn(_)
        | LispObject::HashTable(_) => return Ok(obj_to_value(expr_obj)),
        LispObject::Cons(_) => {} // fall through to cons dispatch
        _ => return Ok(expr),     // Integer out of fixnum range, etc.
    }

    // Cons cell — dispatch on car
    let (car, cdr) = expr_obj.destructure();
    match &car {
        LispObject::Symbol(id) => {
            let sym_name = crate::obarray::symbol_name(*id);
            match sym_name.as_str() {
                "quote" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    Ok(obj_to_value(arg))
                }
                "if" => eval_if(obj_to_value(cdr), env, editor, macros, state),
                "setq" => eval_setq(obj_to_value(cdr), env, editor, macros, state),
                "defun" => eval_defun(obj_to_value(cdr), env, editor, macros, state),
                "let" => eval_let(obj_to_value(cdr), env, editor, macros, state),
                "progn" => eval_progn(obj_to_value(cdr), env, editor, macros, state),
                "lambda" => Ok(obj_to_value(LispObject::lambda_expr(
                    cdr.first().unwrap_or(LispObject::nil()),
                    cdr.rest().unwrap_or(LispObject::nil()),
                ))),
                "cond" => eval_cond(obj_to_value(cdr), env, editor, macros, state),
                "loop" => eval_loop(obj_to_value(cdr), env, editor, macros, state),
                "function" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    Ok(obj_to_value(arg))
                }
                "apply" => eval_apply(obj_to_value(cdr), env, editor, macros, state),
                "funcall" => eval_funcall_form(obj_to_value(cdr), env, editor, macros, state),
                "buffer-string" => eval_buffer_string(editor),
                "buffer-size" => eval_buffer_size(editor),
                "point" => eval_point(editor),
                "goto-char" => eval_goto_char(obj_to_value(cdr), env, editor, macros, state),
                "delete-char" => eval_delete_char(obj_to_value(cdr), env, editor, macros, state),
                "forward-char" => eval_forward_char(obj_to_value(cdr), env, editor, macros, state),
                "find-file" => eval_find_file(obj_to_value(cdr), env, editor, macros, state),
                "save-buffer" => eval_save_buffer(editor),
                "save-excursion" => {
                    eval_save_excursion(obj_to_value(cdr), env, editor, macros, state)
                }
                "save-current-buffer" => {
                    eval_save_current_buffer(obj_to_value(cdr), env, editor, macros, state)
                }
                "save-restriction" => {
                    // No narrowing support yet — treat as progn
                    builtins::eval_progn_value(obj_to_value(cdr), env, editor, macros, state)
                }
                "insert" => eval_insert(obj_to_value(cdr), env, editor, macros, state),
                "prog1" => eval_prog1(obj_to_value(cdr), env, editor, macros, state),
                "prog2" => eval_prog2(obj_to_value(cdr), env, editor, macros, state),
                "and" => eval_and(obj_to_value(cdr), env, editor, macros, state),
                "or" => eval_or(obj_to_value(cdr), env, editor, macros, state),
                "when" => eval_when(obj_to_value(cdr), env, editor, macros, state),
                "unless" => eval_unless(obj_to_value(cdr), env, editor, macros, state),
                "while" => eval_while(obj_to_value(cdr), env, editor, macros, state),
                "let*" => eval_let_star(obj_to_value(cdr), env, editor, macros, state),
                "defvar" => eval_defvar(obj_to_value(cdr), env, editor, macros, state),
                "defcustom" => eval_defvar(obj_to_value(cdr), env, editor, macros, state),
                "defgroup" | "defface" => Ok(Value::nil()),
                "define-minor-mode" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some(n) = name.as_symbol() {
                        env.write().define(&n, LispObject::nil());
                    }
                    Ok(obj_to_value(name))
                }
                "define-derived-mode" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some(n) = name.as_symbol() {
                        env.write().define(&n, LispObject::nil());
                    }
                    Ok(obj_to_value(name))
                }
                "defvar-keymap" => {
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    if let Some(n) = name.as_symbol() {
                        env.write().define(&n, LispObject::nil());
                    }
                    Ok(obj_to_value(name))
                }
                "defconst" => eval_defconst(obj_to_value(cdr), env, editor, macros, state),
                "defalias" => eval_defalias(obj_to_value(cdr), env, editor, macros, state),
                "catch" => eval_catch(obj_to_value(cdr), env, editor, macros, state),
                "throw" => eval_throw(obj_to_value(cdr), env, editor, macros, state),
                "condition-case" => {
                    eval_condition_case(obj_to_value(cdr), env, editor, macros, state)
                }
                "signal" => eval_signal(obj_to_value(cdr), env, editor, macros, state),
                "unwind-protect" => {
                    eval_unwind_protect(obj_to_value(cdr), env, editor, macros, state)
                }
                "error" => eval_error_fn(obj_to_value(cdr), env, editor, macros, state),
                "user-error" => eval_user_error_fn(obj_to_value(cdr), env, editor, macros, state),
                "put" => eval_put(obj_to_value(cdr), env, editor, macros, state),
                "get" => eval_get(obj_to_value(cdr), env, editor, macros, state),
                "provide" => eval_provide(obj_to_value(cdr), env, editor, macros, state),
                "featurep" => eval_featurep(obj_to_value(cdr), env, editor, macros, state),
                "require" => eval_require(obj_to_value(cdr), env, editor, macros, state),
                "load" => builtins::eval_load(obj_to_value(cdr), env, editor, macros, state),
                "mapcar" => eval_mapcar(obj_to_value(cdr), env, editor, macros, state),
                "mapc" => eval_mapc(obj_to_value(cdr), env, editor, macros, state),
                "dolist" => eval_dolist(obj_to_value(cdr), env, editor, macros, state),
                "declare" => Ok(Value::nil()),
                "garbage-collect" => {
                    let mut heap = state.heap.lock();
                    heap.collect();
                    let allocated = heap.bytes_allocated() as i64;
                    let gc_count = heap.gc_count() as i64;
                    let cons_total = crate::object::global_cons_count() as i64;
                    drop(heap);
                    Ok(obj_to_value(LispObject::cons(
                        LispObject::cons(
                            LispObject::symbol("conses"),
                            LispObject::integer(allocated),
                        ),
                        LispObject::cons(
                            LispObject::cons(
                                LispObject::symbol("gc-count"),
                                LispObject::integer(gc_count),
                            ),
                            LispObject::cons(
                                LispObject::cons(
                                    LispObject::symbol("cons-total"),
                                    LispObject::integer(cons_total),
                                ),
                                LispObject::nil(),
                            ),
                        ),
                    )))
                }
                "eval" => {
                    let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let form = eval(obj_to_value(form), env, editor, macros, state)?;
                    eval(form, env, editor, macros, state)
                }
                "format" => eval_format(obj_to_value(cdr), env, editor, macros, state),
                "message" => eval_format(obj_to_value(cdr), env, editor, macros, state),
                "1+" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let val = eval(obj_to_value(arg), env, editor, macros, state)?;
                    let val_obj = value_to_obj(val);
                    match val_obj {
                        LispObject::Integer(n) => Ok(obj_to_value(LispObject::integer(n + 1))),
                        LispObject::Float(f) => Ok(obj_to_value(LispObject::float(f + 1.0))),
                        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
                    }
                }
                "1-" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let val = eval(obj_to_value(arg), env, editor, macros, state)?;
                    let val_obj = value_to_obj(val);
                    match val_obj {
                        LispObject::Integer(n) => Ok(obj_to_value(LispObject::integer(n - 1))),
                        LispObject::Float(f) => Ok(obj_to_value(LispObject::float(f - 1.0))),
                        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
                    }
                }
                "defsubst" => eval_defun(obj_to_value(cdr), env, editor, macros, state),
                "define-error" => Ok(Value::nil()),
                "make-variable-buffer-local" => Ok(Value::nil()),
                "make-hash-table" => {
                    let mut test = crate::object::HashTableTest::Eql;
                    let mut cur = cdr.clone();
                    while let Some((key, rest)) = cur.destructure_cons() {
                        let key_val =
                            value_to_obj(eval(obj_to_value(key), env, editor, macros, state)?);
                        if let Some(s) = key_val.as_symbol() {
                            if s == ":test" {
                                if let Some((val_expr, rest2)) = rest.destructure_cons() {
                                    let val = value_to_obj(eval(
                                        obj_to_value(val_expr),
                                        env,
                                        editor,
                                        macros,
                                        state,
                                    )?);
                                    if let Some(t) = val.as_symbol() {
                                        test = match t.as_str() {
                                            "eq" => crate::object::HashTableTest::Eq,
                                            "eql" => crate::object::HashTableTest::Eql,
                                            "equal" => crate::object::HashTableTest::Equal,
                                            _ => crate::object::HashTableTest::Eql,
                                        };
                                    }
                                    cur = rest2;
                                    continue;
                                }
                            }
                        }
                        cur = rest;
                    }
                    Ok(obj_to_value(LispObject::HashTable(std::sync::Arc::new(
                        parking_lot::Mutex::new(crate::object::LispHashTable::new(test)),
                    ))))
                }
                "gethash" => {
                    let key = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let table = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let default = if let Some(d) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(d), env, editor, macros, state)?)
                    } else {
                        LispObject::nil()
                    };
                    if let LispObject::HashTable(ht) = &table {
                        Ok(obj_to_value(
                            ht.lock().get(&key).cloned().unwrap_or(default),
                        ))
                    } else {
                        Ok(obj_to_value(default))
                    }
                }
                "puthash" => {
                    let key = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let value = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let table_expr = cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
                    let table =
                        value_to_obj(eval(obj_to_value(table_expr), env, editor, macros, state)?);
                    if let LispObject::HashTable(ht) = &table {
                        ht.lock().put(&key, value.clone());
                    }
                    Ok(obj_to_value(value))
                }
                "clrhash" => Ok(Value::nil()),
                "hash-table-p" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(LispObject::from(matches!(
                        arg,
                        LispObject::HashTable(_)
                    ))))
                }
                "hash-table-count" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if let LispObject::HashTable(ht) = &arg {
                        Ok(obj_to_value(LispObject::integer(
                            ht.lock().data.len() as i64
                        )))
                    } else {
                        Ok(obj_to_value(LispObject::integer(0)))
                    }
                }
                "symbol-with-pos-p" => {
                    let _arg = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "bare-symbol" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(arg), env, editor, macros, state)
                }
                "vectorp" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(LispObject::from(matches!(
                        arg,
                        LispObject::Vector(_)
                    ))))
                }
                "recordp" | "char-table-p" | "bool-vector-p" => {
                    let _arg = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "aref" => {
                    let array = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let idx = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let i = idx.as_integer().unwrap_or(0) as usize;
                    match &array {
                        LispObject::Vector(v) => {
                            let v = v.lock();
                            Ok(obj_to_value(v.get(i).cloned().unwrap_or(LispObject::nil())))
                        }
                        LispObject::String(s) => Ok(obj_to_value(LispObject::integer(
                            s.chars().nth(i).map(|c| c as i64).unwrap_or(0),
                        ))),
                        _ => Err(ElispError::WrongTypeArgument("array".to_string())),
                    }
                }
                "aset" => {
                    let _array = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let _idx = eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    eval(
                        obj_to_value(cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "with-suppressed-warnings" | "dont-compile" => {
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "defvaralias"
                | "define-obsolete-function-alias"
                | "define-obsolete-variable-alias"
                | "set-advertised-calling-convention" => {
                    let mut current = cdr.clone();
                    let mut last = Value::nil();
                    while let Some((arg, rest)) = current.destructure_cons() {
                        last = eval(obj_to_value(arg), env, editor, macros, state)?;
                        current = rest;
                    }
                    Ok(last)
                }
                "push" => {
                    let val_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let place = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let val =
                        value_to_obj(eval(obj_to_value(val_expr), env, editor, macros, state)?);
                    let place_name = place
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let old = env.read().get(&place_name).unwrap_or(LispObject::nil());
                    let new = LispObject::cons(val, old);
                    env.write().set(&place_name, new.clone());
                    Ok(obj_to_value(new))
                }
                "pop" => {
                    let place = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let place_name = place
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    let list = env.read().get(&place_name).unwrap_or(LispObject::nil());
                    let car_val = list.first().unwrap_or(LispObject::nil());
                    let cdr_val = list.rest().unwrap_or(LispObject::nil());
                    env.write().set(&place_name, cdr_val);
                    Ok(obj_to_value(car_val))
                }
                "symbol-value" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = arg
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    Ok(obj_to_value(
                        env.read().get(&name).unwrap_or(LispObject::nil()),
                    ))
                }
                "default-value" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = arg
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    Ok(obj_to_value(
                        env.read().get(&name).unwrap_or(LispObject::nil()),
                    ))
                }
                "default-boundp" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = arg
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    Ok(obj_to_value(LispObject::from(
                        env.read().get(&name).is_some(),
                    )))
                }
                "set-default" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let val = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = sym
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    env.write().set(&name, val.clone());
                    Ok(obj_to_value(val))
                }
                "symbol-function" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = arg
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    if let Some(val) = env.read().get(&name) {
                        Ok(obj_to_value(val))
                    } else if let Some(m) = macros.read().get(&name).cloned() {
                        let lambda_form = LispObject::cons(
                            LispObject::symbol("lambda"),
                            LispObject::cons(m.args, m.body),
                        );
                        Ok(obj_to_value(LispObject::cons(
                            LispObject::symbol("macro"),
                            lambda_form,
                        )))
                    } else {
                        Ok(Value::nil())
                    }
                }
                "sort" => {
                    let list = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let pred = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let mut items = Vec::new();
                    let mut cur = list;
                    while let Some((car_val, cdr_val)) = cur.destructure_cons() {
                        items.push(car_val);
                        cur = cdr_val;
                    }
                    items.sort_by(|a, b| {
                        let call_args = LispObject::cons(
                            a.clone(),
                            LispObject::cons(b.clone(), LispObject::nil()),
                        );
                        let result = call_function(
                            obj_to_value(pred.clone()),
                            obj_to_value(call_args),
                            env,
                            editor,
                            macros,
                            state,
                        );
                        match result {
                            Ok(val) if !val.is_nil() => std::cmp::Ordering::Less,
                            _ => std::cmp::Ordering::Greater,
                        }
                    });
                    let mut result = LispObject::nil();
                    for item in items.into_iter().rev() {
                        result = LispObject::cons(item, result);
                    }
                    Ok(obj_to_value(result))
                }
                "nconc" => {
                    let mut lists = Vec::new();
                    let mut current = cdr.clone();
                    while let Some((arg_expr, rest)) = current.destructure_cons() {
                        lists.push(value_to_obj(eval(
                            obj_to_value(arg_expr),
                            env,
                            editor,
                            macros,
                            state,
                        )?));
                        current = rest;
                    }
                    if lists.is_empty() {
                        return Ok(Value::nil());
                    }
                    let mut result_idx = None;
                    for (i, l) in lists.iter().enumerate() {
                        if !l.is_nil() {
                            result_idx = Some(i);
                            break;
                        }
                    }
                    let result_idx = match result_idx {
                        Some(i) => i,
                        None => {
                            return Ok(obj_to_value(
                                lists.last().cloned().unwrap_or(LispObject::nil()),
                            ))
                        }
                    };
                    let result = lists[result_idx].clone();
                    let mut prev = lists[result_idx].clone();
                    for next in &lists[result_idx + 1..] {
                        let mut tail = prev.clone();
                        loop {
                            let cdr_val = tail.cdr().unwrap_or(LispObject::nil());
                            if !cdr_val.is_cons() {
                                break;
                            }
                            tail = cdr_val;
                        }
                        tail.set_cdr(next.clone());
                        prev = next.clone();
                    }
                    Ok(obj_to_value(result))
                }
                "nreverse" | "copy-sequence" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if sym_name == "nreverse" {
                        let mut items = Vec::new();
                        let mut cur = arg;
                        while let Some((car_val, cdr_val)) = cur.destructure_cons() {
                            items.push(car_val);
                            cur = cdr_val;
                        }
                        let mut result = LispObject::nil();
                        for item in items.into_iter() {
                            result = LispObject::cons(item, result);
                        }
                        Ok(obj_to_value(result))
                    } else {
                        Ok(obj_to_value(arg))
                    }
                }
                "autoload" => {
                    let func = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(func)
                }
                "vector" => {
                    let mut items = Vec::new();
                    let mut current = cdr.clone();
                    while let Some((arg, rest)) = current.destructure_cons() {
                        items.push(value_to_obj(eval(
                            obj_to_value(arg),
                            env,
                            editor,
                            macros,
                            state,
                        )?));
                        current = rest;
                    }
                    Ok(obj_to_value(LispObject::Vector(std::sync::Arc::new(
                        parking_lot::Mutex::new(items),
                    ))))
                }
                "make-symbol" => {
                    let name_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let s = name_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    Ok(obj_to_value(LispObject::symbol(s)))
                }
                "fset" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let def = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = sym
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    env.write().define(&name, def.clone());
                    Ok(obj_to_value(def))
                }
                "purecopy" => {
                    let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(arg), env, editor, macros, state)
                }
                "intern" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    match arg {
                        LispObject::String(s) => Ok(obj_to_value(LispObject::symbol(&s))),
                        LispObject::Symbol(_) => Ok(obj_to_value(arg)),
                        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
                    }
                }
                "intern-soft" => {
                    let arg = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = match &arg {
                        LispObject::String(s) => s.clone(),
                        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
                        _ => return Ok(Value::nil()),
                    };
                    if env.read().get(&name).is_some() {
                        Ok(obj_to_value(LispObject::symbol(&name)))
                    } else {
                        Ok(Value::nil())
                    }
                }
                "set" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let val = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = sym
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    env.write().set(&name, val.clone());
                    Ok(obj_to_value(val))
                }
                "boundp" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = sym
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    Ok(obj_to_value(LispObject::from(
                        env.read().get(&name).is_some(),
                    )))
                }
                "fboundp" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = sym
                        .as_symbol()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    Ok(obj_to_value(LispObject::from(
                        env.read().get(&name).is_some(),
                    )))
                }
                "symbol-plist" => {
                    let _sym = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "string-match-p" | "string-match" => {
                    let re_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let str_expr = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                    let re_val =
                        value_to_obj(eval(obj_to_value(re_expr), env, editor, macros, state)?);
                    let re_str = re_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let text_val =
                        value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                    let text = text_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let start = if let Some(s) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(s), env, editor, macros, state)?)
                            .as_integer()
                            .unwrap_or(0) as usize
                    } else {
                        0
                    };
                    let rust_re = emacs_regex_to_rust(&re_str);
                    match regex::Regex::new(&rust_re) {
                        Ok(re) => {
                            if let Some(m) = re.find(&text[start..]) {
                                Ok(obj_to_value(LispObject::integer(
                                    (start + m.start()) as i64,
                                )))
                            } else {
                                Ok(Value::nil())
                            }
                        }
                        Err(_) => Ok(Value::nil()),
                    }
                }
                "match-data" | "match-beginning" | "match-end" | "match-string"
                | "replace-match" | "looking-at" | "re-search-forward" | "re-search-backward"
                | "search-forward" | "search-backward" => Ok(Value::nil()),
                "version-to-list" => {
                    let ver_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let ver =
                        value_to_obj(eval(obj_to_value(ver_expr), env, editor, macros, state)?);
                    let ver_str = ver
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let mut result = LispObject::nil();
                    let parts: Vec<&str> = ver_str.split('.').collect();
                    for part in parts.into_iter().rev() {
                        let n = part.parse::<i64>().unwrap_or(0);
                        result = LispObject::cons(LispObject::integer(n), result);
                    }
                    Ok(obj_to_value(result))
                }
                "read-from-string" => {
                    let str_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let s = value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                    let text = s
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let start = if let Some(start_expr) = cdr.nth(1) {
                        value_to_obj(eval(obj_to_value(start_expr), env, editor, macros, state)?)
                            .as_integer()
                            .unwrap_or(0) as usize
                    } else {
                        0
                    };
                    let sub = &text[start..];
                    let mut reader = crate::reader::Reader::new(sub);
                    match reader.read() {
                        Ok(obj) => {
                            let end_pos = start + reader.position();
                            Ok(obj_to_value(LispObject::cons(
                                obj,
                                LispObject::Integer(end_pos as i64),
                            )))
                        }
                        Err(e) => Err(ElispError::Signal(Box::new(crate::error::SignalData {
                            symbol: LispObject::symbol("invalid-read-syntax"),
                            data: LispObject::cons(
                                LispObject::string(&e.to_string()),
                                LispObject::nil(),
                            ),
                        }))),
                    }
                }
                "split-string" => {
                    let str_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let s = value_to_obj(eval(obj_to_value(str_expr), env, editor, macros, state)?);
                    let text = s
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();

                    let separator = if let Some(sep_expr) = cdr.nth(1) {
                        let sep_val =
                            value_to_obj(eval(obj_to_value(sep_expr), env, editor, macros, state)?);
                        if sep_val.is_nil() {
                            None
                        } else {
                            sep_val.as_string().map(|s| s.to_string())
                        }
                    } else {
                        None
                    };

                    let omit_nulls = if let Some(omit_expr) = cdr.nth(2) {
                        let omit_val = eval(obj_to_value(omit_expr), env, editor, macros, state)?;
                        !omit_val.is_nil()
                    } else {
                        separator.is_none()
                    };

                    let parts: Vec<String> = match &separator {
                        None => text.split_whitespace().map(|s| s.to_string()).collect(),
                        Some(sep) => {
                            let rust_re = emacs_regex_to_rust(sep);
                            match regex::Regex::new(&rust_re) {
                                Ok(re) => re.split(&text).map(|s| s.to_string()).collect(),
                                Err(_) => text.split(sep.as_str()).map(|s| s.to_string()).collect(),
                            }
                        }
                    };

                    let parts: Vec<String> = if omit_nulls {
                        parts.into_iter().filter(|s| !s.is_empty()).collect()
                    } else {
                        parts
                    };

                    let mut result = LispObject::nil();
                    for p in parts.iter().rev() {
                        result = LispObject::cons(LispObject::string(p), result);
                    }
                    Ok(obj_to_value(result))
                }
                "mapconcat" => {
                    let func = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let seq = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sep = if let Some(s) = cdr.nth(2) {
                        value_to_obj(eval(obj_to_value(s), env, editor, macros, state)?)
                            .princ_to_string()
                    } else {
                        String::new()
                    };
                    let mut parts = Vec::new();
                    let mut cur = seq;
                    while let Some((car_val, rest)) = cur.destructure_cons() {
                        let call_args = LispObject::cons(car_val, LispObject::nil());
                        let r = call_function(
                            obj_to_value(func.clone()),
                            obj_to_value(call_args),
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        parts.push(value_to_obj(r).princ_to_string());
                        cur = rest;
                    }
                    Ok(obj_to_value(LispObject::string(&parts.join(&sep))))
                }
                "defmacro" => eval_defmacro(obj_to_value(cdr), macros),
                "macroexpand" => eval_macroexpand(obj_to_value(cdr), env, editor, macros, state),
                _ => {
                    if let Some(s) = car.as_symbol() {
                        let macro_table = macros.read();
                        if let Some(macro_) = macro_table.get(s.as_str()) {
                            let macro_ = macro_.clone();
                            drop(macro_table);
                            let expanded = expand_macro(&macro_, cdr, env, editor, macros, state)?;
                            return eval_next!(obj_to_value(expanded), env, editor, macros, state);
                        }
                    }
                    eval_funcall(
                        obj_to_value(car),
                        obj_to_value(cdr),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
            }
        }
        _ => eval_funcall(
            obj_to_value(car),
            obj_to_value(cdr),
            env,
            editor,
            macros,
            state,
        ),
    }
}

// Sub-modules for different evaluation contexts
mod builtins;
mod dynamic;
mod editor;
mod error_forms;
mod functions;
mod special_forms;

// Re-export functions used internally and externally
use builtins::{
    emacs_regex_to_rust, eval_dolist, eval_featurep, eval_format, eval_get, eval_mapc, eval_mapcar,
    eval_provide, eval_put, eval_require,
};
use editor::{
    eval_buffer_size, eval_buffer_string, eval_delete_char, eval_find_file, eval_forward_char,
    eval_goto_char, eval_insert, eval_point, eval_save_buffer, eval_save_current_buffer,
    eval_save_excursion,
};
use error_forms::{
    eval_catch, eval_condition_case, eval_error_fn, eval_signal, eval_throw, eval_unwind_protect,
    eval_user_error_fn,
};
use functions::{eval_apply, eval_funcall, eval_funcall_form};
use special_forms::{
    eval_and, eval_cond, eval_defalias, eval_defconst, eval_defmacro, eval_defun, eval_defvar,
    eval_if, eval_let, eval_let_star, eval_loop, eval_macroexpand, eval_or, eval_prog1, eval_prog2,
    eval_progn, eval_setq, eval_unless, eval_when, eval_while, expand_macro,
};

// Re-export pub(crate) functions that vm.rs needs
pub(crate) use functions::call_function;

#[cfg(test)]
mod tests;
