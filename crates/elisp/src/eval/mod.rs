use crate::error::{ElispError, ElispResult};
use crate::obarray::{self, SymbolId};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Environment {
    bindings: HashMap<SymbolId, LispObject>,
    parent: Option<Arc<Environment>>,
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub args: LispObject,
    pub body: LispObject,
}

type MacroTable = Arc<RwLock<HashMap<String, Macro>>>;
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

    /// Look up `name` in value position.
    ///
    /// Walks the lexical env chain first; if the name is unbound there,
    /// falls back to the symbol's value cell. This implements Lisp-2
    /// value-position semantics: global vars live in the value cell, but
    /// lexical bindings from `let`/`lambda` shadow them.
    pub fn get(&self, name: &str) -> Option<LispObject> {
        let id = obarray::intern(name);
        self.get_id(id)
    }

    pub fn get_id(&self, id: SymbolId) -> Option<LispObject> {
        if let Some(val) = self.bindings.get(&id).cloned() {
            return Some(val);
        }
        if let Some(p) = self.parent.as_ref() {
            if let Some(val) = p.get_id(id) {
                return Some(val);
            }
        }
        // Fallback: symbol's value cell (global binding).
        obarray::get_value_cell(id)
    }

    /// Env-only lookup: does NOT fall back to the value cell.
    /// Use for `boundp`-style checks and `defvar` initialization.
    pub fn get_id_local(&self, id: SymbolId) -> Option<LispObject> {
        if let Some(val) = self.bindings.get(&id).cloned() {
            return Some(val);
        }
        self.parent.as_ref().and_then(|p| p.get_id_local(id))
    }

    /// Look up `name` in function position.
    ///
    /// Walks the lexical env chain for a callable binding (so lexical
    /// shadowing of functions by lambdas works); falls back to the
    /// symbol's function cell. If a lexical binding exists but isn't
    /// callable, we still prefer it — matches prior behaviour.
    pub fn get_function(&self, name: &str) -> Option<LispObject> {
        let id = obarray::intern(name);
        self.get_function_id(id)
    }

    pub fn get_function_id(&self, id: SymbolId) -> Option<LispObject> {
        let mut first_found: Option<LispObject> = None;
        if let Some(val) = self.bindings.get(&id).cloned() {
            if is_callable_value(&val) {
                return Some(val);
            }
            first_found = Some(val);
        }
        let mut parent = self.parent.as_ref();
        while let Some(p) = parent {
            if let Some(val) = p.bindings.get(&id).cloned() {
                if is_callable_value(&val) {
                    return Some(val);
                }
                if first_found.is_none() {
                    first_found = Some(val);
                }
            }
            parent = p.parent.as_ref();
        }
        // Function-cell fallback.
        if let Some(fn_cell) = obarray::get_function_cell(id) {
            return Some(fn_cell);
        }
        first_found
    }

    pub fn set(&mut self, name: &str, value: LispObject) {
        let id = obarray::intern(name);
        self.bindings.insert(id, value);
    }

    pub fn set_id(&mut self, id: SymbolId, value: LispObject) {
        self.bindings.insert(id, value);
    }

    pub fn define(&mut self, name: &str, value: LispObject) {
        let id = obarray::intern(name);
        self.bindings.insert(id, value);
    }

    pub fn define_id(&mut self, id: SymbolId, value: LispObject) {
        self.bindings.insert(id, value);
    }
}

/// Dynamic binding stack entry: (variable, previous value or None if unbound).
type Specpdl = Arc<RwLock<Vec<(SymbolId, Option<LispObject>)>>>;
/// Set of variables declared special (dynamically bound) via `defvar`/`defconst`.
type SpecialVars = Arc<RwLock<HashSet<SymbolId>>>;

/// Autoload table: maps function names to the file that defines them.
type AutoloadTable = Arc<RwLock<HashMap<String, String>>>;

/// Shared interpreter state accessible during evaluation.
#[derive(Clone)]
pub struct InterpreterState {
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
    /// Autoload mappings: function-name -> file-to-load.
    pub autoloads: AutoloadTable,
    /// Per-eval operation counter. Incremented on every eval call.
    /// When `eval_ops_limit` is > 0 and ops exceeds it, eval returns an error.
    pub eval_ops: Arc<std::sync::atomic::AtomicU64>,
    /// Maximum number of eval operations before aborting (0 = unlimited).
    pub eval_ops_limit: Arc<std::sync::atomic::AtomicU64>,
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
        let special_vars: HashSet<SymbolId> = [
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
        .map(|s| {
            let id = obarray::intern(s);
            obarray::mark_special(id);
            id
        })
        .collect();

        let env = Arc::new(RwLock::new(env));
        Interpreter {
            env: env.clone(),
            editor: Arc::new(RwLock::new(None)),
            macros: Arc::new(RwLock::new(HashMap::new())),
            state: InterpreterState {
                features: Arc::new(RwLock::new(Vec::new())),
                profiler: Arc::new(RwLock::new(crate::jit::Profiler::new(1000))),
                #[cfg(feature = "jit")]
                jit: Arc::new(RwLock::new(crate::jit::JitCompiler::new())),
                special_vars: Arc::new(RwLock::new(special_vars)),
                specpdl: Arc::new(RwLock::new(Vec::new())),
                global_env: env,
                heap: Arc::new(parking_lot::Mutex::new(crate::gc::Heap::new())),
                cons_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                autoloads: Arc::new(RwLock::new(HashMap::new())),
                eval_ops: Arc::new(std::sync::atomic::AtomicU64::new(0)),
                eval_ops_limit: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
        let id = obarray::intern(name);
        // Route to the symbol's value or function cell depending on
        // callability. This is Lisp-2 semantics: functions and variables
        // live in separate slots on the symbol.
        //
        // Keep nil/t in the env (bootstrap) so legacy `env.get("nil")`
        // paths keep working until every call site moves to SymbolId.
        if name == "nil" || name == "t" {
            self.env.write().define_id(id, value);
            return;
        }
        if is_callable_value(&value) {
            obarray::set_function_cell(id, value);
        } else {
            obarray::set_value_cell(id, value);
        }
    }

    pub fn set_editor(&self, editor: Box<dyn EditorCallbacks>) {
        let mut e = self.editor.write();
        *e = Some(editor);
    }

    /// Set a maximum number of eval operations. 0 means unlimited.
    /// When the limit is reached, eval returns an error.
    pub fn set_eval_ops_limit(&self, limit: u64) {
        self.state
            .eval_ops_limit
            .store(limit, std::sync::atomic::Ordering::Relaxed);
    }

    /// Reset the eval operation counter to zero.
    pub fn reset_eval_ops(&self) {
        self.state
            .eval_ops
            .store(0, std::sync::atomic::Ordering::Relaxed);
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
    // Check operation limit (if set)
    let limit = state
        .eval_ops_limit
        .load(std::sync::atomic::Ordering::Relaxed);
    if limit > 0 {
        let ops = state
            .eval_ops
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if ops >= limit {
            return Err(ElispError::EvalError(
                "eval operation limit exceeded".into(),
            ));
        }
    }
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
    if let Some(raw) = expr.as_symbol_id() {
        let sym_id = SymbolId(raw);
        let name = crate::obarray::symbol_name(sym_id);
        if name.starts_with(':') {
            return Ok(expr);
        }
        if state.special_vars.read().contains(&sym_id) {
            let global = state.global_env.read();
            return global
                .get_id(sym_id)
                .map(obj_to_value)
                .ok_or(ElispError::VoidVariable(name));
        } else {
            let env = env.read();
            return env
                .get_id(sym_id)
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
                    // (quote x) -> x via first(), but also handle
                    // dotted form (quote . x) where cdr is the atom itself.
                    match cdr.first() {
                        Some(arg) => Ok(obj_to_value(arg)),
                        None if !cdr.is_nil() => Ok(obj_to_value(cdr)),
                        _ => Err(ElispError::WrongNumberOfArguments),
                    }
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
                // -- Buffer primitives (single-buffer model) --
                "current-buffer" => {
                    let e = editor.read();
                    match e.as_ref() {
                        Some(_) => Ok(obj_to_value(LispObject::string("*scratch*"))),
                        None => Ok(Value::nil()),
                    }
                }
                "set-buffer" => {
                    let _buf = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "buffer-name" => Ok(obj_to_value(LispObject::string("*scratch*"))),
                "get-buffer" => {
                    let name = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(name)
                }
                "get-buffer-create" => {
                    let name = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(name)
                }
                "buffer-list" => Ok(obj_to_value(LispObject::cons(
                    LispObject::string("*scratch*"),
                    LispObject::nil(),
                ))),
                "buffer-live-p" => {
                    let _buf = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(obj_to_value(LispObject::t()))
                }
                "with-current-buffer" => {
                    // (with-current-buffer BUFFER BODY...)
                    let _buf = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let body = cdr.rest().unwrap_or(LispObject::nil());
                    eval_progn(obj_to_value(body), env, editor, macros, state)
                }
                "generate-new-buffer-name" => {
                    let name = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(name)
                }
                // -- File-name quoting --
                "file-name-quote" => {
                    let name_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let name_obj = value_to_obj(name_val);
                    let s = name_obj
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    Ok(obj_to_value(LispObject::string(&format!("/:{}", s))))
                }
                "file-name-unquote" => {
                    let name_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    let name_obj = value_to_obj(name_val);
                    let s = name_obj
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let unquoted = s.strip_prefix("/:").unwrap_or(s);
                    Ok(obj_to_value(LispObject::string(unquoted)))
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
                "declare" | "interactive" | "eval-after-load" | "make-help-screen" => {
                    Ok(Value::nil())
                }
                "fmakunbound" => {
                    // Remove a function definition
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    if let Some(name) = sym.as_symbol() {
                        // Remove from macros table
                        macros.write().remove(&name);
                        // We don't have a way to truly remove from env,
                        // but we can set it to nil
                    }
                    Ok(obj_to_value(sym))
                }
                "makunbound" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    Ok(obj_to_value(sym))
                }
                "garbage-collect" => {
                    let mut heap = state.heap.lock();
                    heap.collect();
                    let allocated = heap.bytes_allocated() as i64;
                    let gc_count = heap.gc_count() as i64;
                    let cons_total = crate::object::global_cons_count() as i64;
                    drop(heap);
                    Ok(obj_to_value(LispObject::cons(
                        LispObject::cons(
                            LispObject::symbol("bytes-allocated"),
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
                "format" | "format-message" => {
                    eval_format(obj_to_value(cdr), env, editor, macros, state)
                }
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
                    // symbol-function: function-position lookup (env + function cell).
                    if let Some(val) = env.read().get_function(&name) {
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
                    let func_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    if let Some(file_expr) = cdr.nth(1) {
                        let file_val = eval(obj_to_value(file_expr), env, editor, macros, state)?;
                        let func_obj = value_to_obj(func_val);
                        let file_obj = value_to_obj(file_val);
                        if let (Some(func_name), Some(file_name)) =
                            (func_obj.as_symbol(), file_obj.as_string())
                        {
                            state.autoloads.write().insert(func_name, file_name.clone());
                        }
                    }
                    Ok(func_val)
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
                "make-vector" => {
                    let len_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let init_val = value_to_obj(eval(
                        obj_to_value(cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let len = len_val.as_integer().unwrap_or(0).max(0) as usize;
                    let items = vec![init_val; len];
                    Ok(obj_to_value(LispObject::Vector(std::sync::Arc::new(
                        parking_lot::Mutex::new(items),
                    ))))
                }
                "vconcat" => {
                    // Concatenate sequences into a vector
                    let mut items = Vec::new();
                    let mut current = cdr.clone();
                    while let Some((arg_expr, rest)) = current.destructure_cons() {
                        let arg =
                            value_to_obj(eval(obj_to_value(arg_expr), env, editor, macros, state)?);
                        match &arg {
                            LispObject::Vector(v) => {
                                items.extend(v.lock().iter().cloned());
                            }
                            LispObject::String(s) => {
                                for c in s.chars() {
                                    items.push(LispObject::integer(c as i64));
                                }
                            }
                            _ => {
                                let mut cur = arg;
                                while let Some((car, cdr_v)) = cur.destructure_cons() {
                                    items.push(car);
                                    cur = cdr_v;
                                }
                            }
                        }
                        current = rest;
                    }
                    Ok(obj_to_value(LispObject::Vector(std::sync::Arc::new(
                        parking_lot::Mutex::new(items),
                    ))))
                }
                "byte-code" => {
                    // Stub: we don't have a bytecode interpreter.
                    // Return nil to let files that contain byte-compiled forms continue loading.
                    Ok(Value::nil())
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
                    let sym_id = sym
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    // fset writes the function cell.
                    crate::obarray::set_function_cell(sym_id, def.clone());
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
                    let sym_id = sym
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    // set writes the value cell (Emacs `set` sets the global
                    // value). Environment is not touched so lexical shadows
                    // don't interfere with global set.
                    crate::obarray::set_value_cell(sym_id, val.clone());
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
                    // fboundp uses function-position lookup (env chain +
                    // function-cell fallback).
                    Ok(obj_to_value(LispObject::from(
                        env.read().get_function(&name).is_some(),
                    )))
                }
                "symbol-plist" => {
                    let sym = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let sym_id = sym
                        .as_symbol_id()
                        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                    Ok(obj_to_value(crate::obarray::full_plist(sym_id)))
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
                // eval-when-compile / eval-and-compile: at load time, behave like progn
                "eval-when-compile" | "eval-and-compile" => {
                    eval_progn(obj_to_value(cdr), env, editor, macros, state)
                }
                // File operation primitives
                "file-exists-p" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    Ok(obj_to_value(LispObject::from(
                        std::path::Path::new(path.as_str()).exists(),
                    )))
                }
                "expand-file-name" => {
                    let name_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name_str = name_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let expanded = if name_str.starts_with('/') || name_str.starts_with('~') {
                        name_str
                    } else {
                        let dir = cdr
                            .nth(1)
                            .and_then(|d| {
                                let v = value_to_obj(
                                    eval(obj_to_value(d), env, editor, macros, state).ok()?,
                                );
                                v.as_string().map(|s| s.to_string())
                            })
                            .unwrap_or_else(|| {
                                std::env::current_dir()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            });
                        format!("{}/{}", dir.trim_end_matches('/'), name_str)
                    };
                    Ok(obj_to_value(LispObject::string(&expanded)))
                }
                "file-name-directory" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    match std::path::Path::new(path.as_str()).parent() {
                        Some(p) if !p.as_os_str().is_empty() => Ok(obj_to_value(
                            LispObject::string(&format!("{}/", p.display())),
                        )),
                        _ => Ok(Value::nil()),
                    }
                }
                "file-name-nondirectory" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let name = std::path::Path::new(path.as_str())
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    Ok(obj_to_value(LispObject::string(&name)))
                }
                "file-readable-p" | "file-directory-p" | "file-regular-p" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let p = std::path::Path::new(path.as_str());
                    let result = match sym_name.as_str() {
                        "file-readable-p" => p.exists(),
                        "file-directory-p" => p.is_dir(),
                        "file-regular-p" => p.is_file(),
                        _ => false,
                    };
                    Ok(obj_to_value(LispObject::from(result)))
                }
                "file-truename" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let resolved = std::fs::canonicalize(path.as_str())
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| path.to_string());
                    Ok(obj_to_value(LispObject::string(&resolved)))
                }
                "temporary-file-directory" => Ok(obj_to_value(LispObject::string("/tmp/"))),
                "directory-file-name" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let trimmed = path.trim_end_matches('/');
                    let result = if trimmed.is_empty() { "/" } else { trimmed };
                    Ok(obj_to_value(LispObject::string(result)))
                }
                "file-name-as-directory" => {
                    let file = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let path = file
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    let result = if path.ends_with('/') {
                        path.to_string()
                    } else {
                        format!("{}/", path)
                    };
                    Ok(obj_to_value(LispObject::string(&result)))
                }
                // Environment / system primitives
                "getenv" => {
                    let var = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name = var
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                    match std::env::var(name.as_str()) {
                        Ok(val) => Ok(obj_to_value(LispObject::string(&val))),
                        Err(_) => Ok(Value::nil()),
                    }
                }
                "system-name" => Ok(obj_to_value(LispObject::string("localhost"))),
                "user-login-name" | "user-real-login-name" => Ok(obj_to_value(LispObject::string(
                    &std::env::var("USER").unwrap_or_default(),
                ))),
                "emacs-pid" => Ok(obj_to_value(LispObject::integer(std::process::id() as i64))),
                // -- Char-table primitives (P4 i18n stubs) --
                "make-char-table" => {
                    // (make-char-table PURPOSE &optional INIT)
                    // Evaluate args for side-effects, return nil (no real char-table type)
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "set-char-table-range" => {
                    // (set-char-table-range TABLE RANGE VALUE) → VALUE
                    let mut vals = Vec::new();
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        vals.push(eval(obj_to_value(arg), env, editor, macros, state)?);
                        cur = rest;
                    }
                    Ok(vals.into_iter().last().unwrap_or(Value::nil()))
                }
                "char-table-range" => {
                    // (char-table-range TABLE CHAR) → nil
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "set-char-table-parent" => {
                    // (set-char-table-parent TABLE PARENT) → no-op
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "char-table-parent" => {
                    // (char-table-parent TABLE) → nil
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "char-table-extra-slot" => {
                    // (char-table-extra-slot TABLE N) → nil
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "set-char-table-extra-slot" => {
                    // (set-char-table-extra-slot TABLE N VALUE) → VALUE
                    let mut vals = Vec::new();
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        vals.push(eval(obj_to_value(arg), env, editor, macros, state)?);
                        cur = rest;
                    }
                    Ok(vals.into_iter().last().unwrap_or(Value::nil()))
                }
                "map-char-table" => {
                    // (map-char-table FUNCTION TABLE) → no-op
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "standard-case-table" | "standard-syntax-table" | "syntax-table" => {
                    // No args, return nil
                    Ok(Value::nil())
                }
                "set-standard-case-table" | "set-syntax-table" => {
                    // (set-standard-case-table TABLE) → no-op
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "char-syntax" => {
                    // (char-syntax CHAR) → ?\s (space = word constituent, integer 32)
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(obj_to_value(LispObject::integer(32))) // ?\s = space
                }
                // -- Coding system stubs (P4 i18n) --
                "define-coding-system-alias" => {
                    // (define-coding-system-alias ALIAS CODING-SYSTEM) → no-op
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "coding-system-p" => {
                    // (coding-system-p OBJ) → nil
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "check-coding-system" => {
                    // (check-coding-system CODING-SYSTEM) → return arg
                    eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "coding-system-list" => {
                    // (coding-system-list) → nil
                    Ok(Value::nil())
                }
                "find-coding-systems-region" => {
                    // (find-coding-systems-region START END) → empty list
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "encode-coding-string" | "decode-coding-string" => {
                    // (encode-coding-string STRING CODING-SYSTEM) → STRING
                    // (decode-coding-string STRING CODING-SYSTEM) → STRING
                    let string_val = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    // Eval remaining args for side effects
                    if let Some(rest) = cdr.rest() {
                        let mut cur = rest;
                        while let Some((arg, r)) = cur.destructure_cons() {
                            let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                            cur = r;
                        }
                    }
                    Ok(string_val)
                }
                "set-keyboard-coding-system" | "set-terminal-coding-system" => {
                    // (set-keyboard-coding-system CODING-SYSTEM) → no-op
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "string-to-multibyte" | "string-to-unibyte" => {
                    // (string-to-multibyte STRING) → STRING (identity)
                    eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )
                }
                "unibyte-string" => {
                    // (unibyte-string &rest BYTES) → construct string from byte values
                    let mut bytes = Vec::new();
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let val =
                            value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
                        if let Some(n) = val.as_integer() {
                            bytes.push(n as u8);
                        }
                        cur = rest;
                    }
                    Ok(obj_to_value(LispObject::string(&String::from_utf8_lossy(
                        &bytes,
                    ))))
                }
                "locale-coding-system" => {
                    // Variable stub → nil
                    Ok(Value::nil())
                }
                // -- Misc internationalization stubs (P4 i18n) --
                "set-language-environment"
                | "set-default-coding-systems"
                | "prefer-coding-system" => {
                    // (set-language-environment ENV) → no-op
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                "define-charset-internal" => {
                    // (define-charset-internal &rest ARGS) → no-op, eval all args
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "put-charset-property" => {
                    // (put-charset-property CHARSET PROPNAME VALUE) → no-op
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "charset-plist" => {
                    // (charset-plist CHARSET) → nil
                    let _ = eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?;
                    Ok(Value::nil())
                }
                // -- locate-library --
                "locate-library" => {
                    let name_val = value_to_obj(eval(
                        obj_to_value(cdr.first().ok_or(ElispError::WrongNumberOfArguments)?),
                        env,
                        editor,
                        macros,
                        state,
                    )?);
                    let name_str = name_val
                        .as_string()
                        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                        .clone();
                    let load_path = state
                        .global_env
                        .read()
                        .get("load-path")
                        .unwrap_or(LispObject::nil());
                    let mut load_dirs = Vec::new();
                    let mut cur = load_path;
                    while let Some((dir, rest)) = cur.destructure_cons() {
                        if let Some(d) = dir.as_string() {
                            load_dirs.push(d.clone());
                        }
                        cur = rest;
                    }
                    let suffixes = [".elc", ".el", ""];
                    for suffix in &suffixes {
                        let full = format!("{}{}", name_str, suffix);
                        if std::path::Path::new(&full).exists() {
                            return Ok(obj_to_value(LispObject::string(&full)));
                        }
                        for d in &load_dirs {
                            let candidate = format!("{}/{}", d, full);
                            if std::path::Path::new(&candidate).exists() {
                                return Ok(obj_to_value(LispObject::string(&candidate)));
                            }
                        }
                    }
                    Ok(Value::nil())
                }
                // -- Text property primitives (P5 stubs) --
                "propertize" => {
                    // (propertize STRING &rest PROPERTIES) -> STRING
                    let s = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(s), env, editor, macros, state)
                }
                "put-text-property"
                | "set-text-properties"
                | "add-text-properties"
                | "remove-text-properties" => {
                    // No-op: eval all args for side effects
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "get-text-property"
                | "text-properties-at"
                | "next-single-property-change"
                | "previous-single-property-change"
                | "text-property-any" => {
                    // Eval args, return nil
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }

                // -- Face primitives (P5 stubs) --
                "make-face" => {
                    // (make-face FACE) -> FACE symbol
                    let face = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(face), env, editor, macros, state)
                }
                "face-list" => Ok(Value::nil()),
                "set-face-attribute" | "internal-set-lisp-face-attribute" | "face-spec-set" => {
                    // No-op: eval all args
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "face-attribute"
                | "face-attribute-relative-p"
                | "internal-lisp-face-attribute-values" => {
                    // Eval args, return nil
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "display-supports-face-attributes-p" => {
                    // Eval args, return t (we "support" everything)
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(obj_to_value(LispObject::t()))
                }

                // -- pcase stubs (P5) --
                "pcase" => {
                    // (pcase EXPR CLAUSES...)
                    let expr_form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let expr_val = eval(obj_to_value(expr_form), env, editor, macros, state)?;
                    let clauses = cdr.rest().unwrap_or(LispObject::nil());
                    let mut cur = clauses;
                    while let Some((clause, rest)) = cur.destructure_cons() {
                        let (pattern, body) = match clause.destructure_cons() {
                            Some(pair) => pair,
                            None => {
                                cur = rest;
                                continue;
                            }
                        };
                        // _ or t matches everything (wildcard)
                        if let Some(s) = pattern.as_symbol() {
                            if s == "_" || s == "t" {
                                return eval_progn(obj_to_value(body), env, editor, macros, state);
                            }
                        }
                        // Quoted pattern: (quote VAL) matches if equal
                        if let Some(quoted) = pattern.as_quote_content() {
                            if value_to_obj(expr_val) == quoted {
                                return eval_progn(obj_to_value(body), env, editor, macros, state);
                            }
                        }
                        // Literal match
                        match eval(obj_to_value(pattern), env, editor, macros, state) {
                            Ok(pattern_val) if pattern_val == expr_val => {
                                return eval_progn(obj_to_value(body), env, editor, macros, state);
                            }
                            _ => {}
                        }
                        cur = rest;
                    }
                    Ok(Value::nil())
                }
                "pcase-let" | "pcase-let*" => {
                    // Treat as let/let*
                    special_forms::eval_let(obj_to_value(cdr), env, editor, macros, state)
                }
                "pcase-dolist" => {
                    // Treat as dolist
                    builtins::eval_dolist(obj_to_value(cdr), env, editor, macros, state)
                }

                // -- Button stubs (P5) --
                "define-button-type" => {
                    // (define-button-type NAME &rest PROPERTIES) -> NAME
                    let name = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(name), env, editor, macros, state)
                }

                // -- Misc stubs (P5) --
                "make-local-variable" => {
                    // (make-local-variable VAR) -> VAR
                    let var = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    eval(obj_to_value(var), env, editor, macros, state)
                }
                "frame-list" | "window-list" => Ok(Value::nil()),
                "selected-frame" | "selected-window" => Ok(Value::nil()),
                "frame-parameter" => {
                    // Eval args, return nil
                    let mut cur = cdr.clone();
                    while let Some((arg, rest)) = cur.destructure_cons() {
                        let _ = eval(obj_to_value(arg), env, editor, macros, state)?;
                        cur = rest;
                    }
                    Ok(Value::nil())
                }

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
