use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
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
            matches!(&b.0, LispObject::Symbol(s) if s == "lambda")
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
            },
        }
    }

    pub fn eval(&self, expr: LispObject) -> ElispResult<LispObject> {
        eval(expr, &self.env, &self.editor, &self.macros, &self.state)
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
    expr: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    inc_eval_depth()?;
    let result = eval_inner(expr, env, editor, macros, state);
    dec_eval_depth();
    result
}

fn eval_inner(
    expr: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    match expr {
        LispObject::Nil
        | LispObject::T
        | LispObject::Integer(_)
        | LispObject::Float(_)
        | LispObject::String(_)
        | LispObject::Primitive(_)
        | LispObject::Vector(_)
        | LispObject::BytecodeFn(_)
        | LispObject::HashTable(_) => Ok(expr),
        LispObject::Symbol(ref name) if name.starts_with(':') => {
            // Keyword symbols are self-evaluating
            Ok(expr)
        }
        LispObject::Symbol(name) => {
            // Special (dynamically bound) variables are always looked up in the global env.
            if state.special_vars.read().contains(&name) {
                let global = state.global_env.read();
                global.get(&name).ok_or(ElispError::VoidVariable(name))
            } else {
                let env = env.read();
                env.get(&name).ok_or(ElispError::VoidVariable(name))
            }
        }
        LispObject::Cons(_) => {
            let (car, cdr) = expr.destructure();
            match &car {
                LispObject::Symbol(s) => match s.as_str() {
                    "quote" => {
                        let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        Ok(arg)
                    }
                    "if" => eval_if(&cdr, env, editor, macros, state),
                    "setq" => eval_setq(&cdr, env, editor, macros, state),
                    "defun" => eval_defun(&cdr, env, editor, macros, state),
                    "let" => eval_let(&cdr, env, editor, macros, state),
                    "progn" => eval_progn(&cdr, env, editor, macros, state),
                    "lambda" => Ok(LispObject::lambda_expr(
                        cdr.first().unwrap_or(LispObject::nil()),
                        cdr.rest().unwrap_or(LispObject::nil()),
                    )),
                    "cond" => eval_cond(&cdr, env, editor, macros, state),
                    "loop" => eval_loop(&cdr, env, editor, macros, state),
                    "function" => {
                        let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        Ok(arg)
                    }
                    "apply" => eval_apply(&cdr, env, editor, macros, state),
                    "funcall" => eval_funcall_form(&cdr, env, editor, macros, state),
                    "buffer-string" => eval_buffer_string(editor),
                    "buffer-size" => eval_buffer_size(editor),
                    "point" => eval_point(editor),
                    "goto-char" => eval_goto_char(&cdr, env, editor, macros, state),
                    "delete-char" => eval_delete_char(&cdr, env, editor, macros, state),
                    "forward-char" => eval_forward_char(&cdr, env, editor, macros, state),
                    "find-file" => eval_find_file(&cdr, env, editor, macros, state),
                    "save-buffer" => eval_save_buffer(editor),
                    "insert" => eval_insert(&cdr, env, editor, macros, state),
                    "prog1" => eval_prog1(&cdr, env, editor, macros, state),
                    "prog2" => eval_prog2(&cdr, env, editor, macros, state),
                    "and" => eval_and(&cdr, env, editor, macros, state),
                    "or" => eval_or(&cdr, env, editor, macros, state),
                    "when" => eval_when(&cdr, env, editor, macros, state),
                    "unless" => eval_unless(&cdr, env, editor, macros, state),
                    "while" => eval_while(&cdr, env, editor, macros, state),
                    "let*" => eval_let_star(&cdr, env, editor, macros, state),
                    "defvar" => eval_defvar(&cdr, env, editor, macros, state),
                    "defconst" => eval_defconst(&cdr, env, editor, macros, state),
                    "defalias" => eval_defalias(&cdr, env, editor, macros, state),
                    "catch" => eval_catch(&cdr, env, editor, macros, state),
                    "throw" => eval_throw(&cdr, env, editor, macros, state),
                    "condition-case" => eval_condition_case(&cdr, env, editor, macros, state),
                    "signal" => eval_signal(&cdr, env, editor, macros, state),
                    "unwind-protect" => eval_unwind_protect(&cdr, env, editor, macros, state),
                    "error" => eval_error_fn(&cdr, env, editor, macros, state),
                    "put" => eval_put(&cdr, env, editor, macros, state),
                    "get" => eval_get(&cdr, env, editor, macros, state),
                    "provide" => eval_provide(&cdr, env, editor, macros, state),
                    "featurep" => eval_featurep(&cdr, env, editor, macros, state),
                    "require" => eval_require(&cdr, env, editor, macros, state),
                    "mapcar" => eval_mapcar(&cdr, env, editor, macros, state),
                    "mapc" => eval_mapc(&cdr, env, editor, macros, state),
                    "dolist" => eval_dolist(&cdr, env, editor, macros, state),
                    "declare" => Ok(LispObject::nil()), // no-op
                    "eval" => {
                        let form = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let form = eval(form, env, editor, macros, state)?;
                        eval(form, env, editor, macros, state)
                    }
                    "format" => eval_format(&cdr, env, editor, macros, state),
                    "message" => eval_format(&cdr, env, editor, macros, state),
                    "1+" => {
                        let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let val = eval(arg, env, editor, macros, state)?;
                        match val {
                            LispObject::Integer(n) => Ok(LispObject::integer(n + 1)),
                            LispObject::Float(f) => Ok(LispObject::float(f + 1.0)),
                            _ => Err(ElispError::WrongTypeArgument("number".to_string())),
                        }
                    }
                    "1-" => {
                        let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let val = eval(arg, env, editor, macros, state)?;
                        match val {
                            LispObject::Integer(n) => Ok(LispObject::integer(n - 1)),
                            LispObject::Float(f) => Ok(LispObject::float(f - 1.0)),
                            _ => Err(ElispError::WrongTypeArgument("number".to_string())),
                        }
                    }
                    "defsubst" => eval_defun(&cdr, env, editor, macros, state), // same as defun for now
                    "define-error" => Ok(LispObject::nil()),                    // stub
                    "make-variable-buffer-local" => Ok(LispObject::nil()),      // stub
                    "make-hash-table" => {
                        // Parse :test keyword arg
                        let mut test = crate::object::HashTableTest::Eql;
                        let mut cur = cdr.clone();
                        while let Some((key, rest)) = cur.destructure_cons() {
                            let key = eval(key, env, editor, macros, state)?;
                            if let LispObject::Symbol(s) = &key {
                                if s == ":test" {
                                    if let Some((val_expr, rest2)) = rest.destructure_cons() {
                                        let val = eval(val_expr, env, editor, macros, state)?;
                                        if let LispObject::Symbol(t) = &val {
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
                        Ok(LispObject::HashTable(std::sync::Arc::new(
                            parking_lot::Mutex::new(crate::object::LispHashTable::new(test)),
                        )))
                    }
                    "gethash" => {
                        let key = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let table = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let default = if let Some(d) = cdr.nth(2) {
                            eval(d, env, editor, macros, state)?
                        } else {
                            LispObject::nil()
                        };
                        if let LispObject::HashTable(ht) = &table {
                            Ok(ht.lock().get(&key).cloned().unwrap_or(default))
                        } else {
                            Ok(default)
                        }
                    }
                    "puthash" => {
                        let key = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let value = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let table_expr = cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
                        let table = eval(table_expr, env, editor, macros, state)?;
                        if let LispObject::HashTable(ht) = &table {
                            ht.lock().put(&key, value.clone());
                        }
                        Ok(value)
                    }
                    "clrhash" => Ok(LispObject::nil()),
                    "hash-table-p" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(LispObject::from(matches!(arg, LispObject::HashTable(_))))
                    }
                    "hash-table-count" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        if let LispObject::HashTable(ht) = &arg {
                            Ok(LispObject::integer(ht.lock().data.len() as i64))
                        } else {
                            Ok(LispObject::integer(0))
                        }
                    }
                    "symbol-with-pos-p" => {
                        let _arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(LispObject::nil()) // we never have positioned symbols
                    }
                    "bare-symbol" => {
                        let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        eval(arg, env, editor, macros, state) // identity for us
                    }
                    "vectorp" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(LispObject::from(matches!(arg, LispObject::Vector(_))))
                    }
                    "recordp" | "char-table-p" | "bool-vector-p" => {
                        let _arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(LispObject::nil())
                    }
                    "aref" => {
                        let array = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let idx = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let i = idx.as_integer().unwrap_or(0) as usize;
                        match &array {
                            LispObject::Vector(v) => {
                                let v = v.lock();
                                Ok(v.get(i).cloned().unwrap_or(LispObject::nil()))
                            }
                            LispObject::String(s) => Ok(LispObject::integer(
                                s.chars().nth(i).map(|c| c as i64).unwrap_or(0),
                            )),
                            _ => Err(ElispError::WrongTypeArgument("array".to_string())),
                        }
                    }
                    "aset" => {
                        // stub: return the value (vectors are immutable in our impl)
                        let _array = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let _idx = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let val = eval(
                            cdr.nth(2).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(val)
                    }
                    "with-suppressed-warnings" | "dont-compile" => {
                        // (with-suppressed-warnings WARNINGS BODY...) — just eval body
                        let body = cdr.rest().unwrap_or(LispObject::nil());
                        eval_progn(&body, env, editor, macros, state)
                    }
                    "defvaralias"
                    | "define-obsolete-function-alias"
                    | "define-obsolete-variable-alias"
                    | "set-advertised-calling-convention" => {
                        // stubs: eval args for side effects but do nothing special
                        let mut current = cdr.clone();
                        let mut last = LispObject::nil();
                        while let Some((arg, rest)) = current.destructure_cons() {
                            last = eval(arg, env, editor, macros, state)?;
                            current = rest;
                        }
                        Ok(last)
                    }
                    "push" => {
                        // (push VALUE PLACE) — simplified: only supports symbol as PLACE
                        let val_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let place = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                        let val = eval(val_expr, env, editor, macros, state)?;
                        let place_name = place
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        let old = env.read().get(place_name).unwrap_or(LispObject::nil());
                        let new = LispObject::cons(val, old);
                        env.write().set(place_name, new.clone());
                        Ok(new)
                    }
                    "pop" => {
                        let place = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let place_name = place
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        let list = env.read().get(place_name).unwrap_or(LispObject::nil());
                        let car = list.first().unwrap_or(LispObject::nil());
                        let cdr_val = list.rest().unwrap_or(LispObject::nil());
                        env.write().set(place_name, cdr_val);
                        Ok(car)
                    }
                    "symbol-value" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = arg
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        Ok(env.read().get(name).unwrap_or(LispObject::nil()))
                    }
                    "default-value" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = arg
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        Ok(env.read().get(name).unwrap_or(LispObject::nil()))
                    }
                    "default-boundp" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = arg
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        Ok(LispObject::from(env.read().get(name).is_some()))
                    }
                    "set-default" => {
                        let sym = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let val = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = sym
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        env.write().set(name, val.clone());
                        Ok(val)
                    }
                    "symbol-function" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = arg
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        // Check env first, then fall back to macro table.
                        // Macros are returned as (macro lambda ARGS . BODY)
                        // matching real Emacs behaviour.
                        if let Some(val) = env.read().get(name) {
                            Ok(val)
                        } else if let Some(m) = macros.read().get(name).cloned() {
                            let lambda_form = LispObject::cons(
                                LispObject::symbol("lambda"),
                                LispObject::cons(m.args, m.body),
                            );
                            Ok(LispObject::cons(LispObject::symbol("macro"), lambda_form))
                        } else {
                            Ok(LispObject::nil())
                        }
                    }
                    "sort" => {
                        let list = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let pred = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let mut items = Vec::new();
                        let mut cur = list;
                        while let Some((car, cdr_val)) = cur.destructure_cons() {
                            items.push(car);
                            cur = cdr_val;
                        }
                        // Sort using the predicate: (PRED A B) returns non-nil if A < B
                        items.sort_by(|a, b| {
                            let call_args = LispObject::cons(
                                a.clone(),
                                LispObject::cons(b.clone(), LispObject::nil()),
                            );
                            let result =
                                call_function(&pred, &call_args, env, editor, macros, state);
                            match result {
                                Ok(val) if !val.is_nil() => std::cmp::Ordering::Less,
                                _ => std::cmp::Ordering::Greater,
                            }
                        });
                        let mut result = LispObject::nil();
                        for item in items.into_iter().rev() {
                            result = LispObject::cons(item, result);
                        }
                        Ok(result)
                    }
                    "nconc" => {
                        // Destructive nconc: mutate the last cdr of each list
                        // to point to the next list.
                        let mut lists = Vec::new();
                        let mut current = cdr.clone();
                        while let Some((arg_expr, rest)) = current.destructure_cons() {
                            lists.push(eval(arg_expr, env, editor, macros, state)?);
                            current = rest;
                        }
                        if lists.is_empty() {
                            return Ok(LispObject::nil());
                        }
                        // Find the first non-nil list as result
                        let mut result_idx = None;
                        for (i, l) in lists.iter().enumerate() {
                            if !l.is_nil() {
                                result_idx = Some(i);
                                break;
                            }
                        }
                        let result_idx = match result_idx {
                            Some(i) => i,
                            None => return Ok(lists.last().cloned().unwrap_or(LispObject::nil())),
                        };
                        let result = lists[result_idx].clone();
                        // Chain: for each non-nil list, find its last cons and
                        // set_cdr to the next non-nil list (or last arg).
                        let mut prev = lists[result_idx].clone();
                        for next in &lists[result_idx + 1..] {
                            // Walk prev to its last cons
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
                        Ok(result)
                    }
                    "nreverse" | "copy-sequence" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        // nreverse: reverse list; copy-sequence: clone (already cloned)
                        if s.as_str() == "nreverse" {
                            let mut items = Vec::new();
                            let mut cur = arg;
                            while let Some((car, cdr_val)) = cur.destructure_cons() {
                                items.push(car);
                                cur = cdr_val;
                            }
                            let mut result = LispObject::nil();
                            for item in items.into_iter() {
                                result = LispObject::cons(item, result);
                            }
                            Ok(result)
                        } else {
                            Ok(arg) // copy-sequence: already cloned
                        }
                    }
                    "autoload" => {
                        // (autoload FUNCTION FILE &optional DOCSTRING INTERACTIVE TYPE)
                        // Stub: just register the function name as autoloaded (no-op for now)
                        let func = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(func)
                    }
                    "vector" => {
                        // (vector &rest ARGS) — create a vector from args
                        let mut items = Vec::new();
                        let mut current = cdr.clone();
                        while let Some((arg, rest)) = current.destructure_cons() {
                            items.push(eval(arg, env, editor, macros, state)?);
                            current = rest;
                        }
                        Ok(LispObject::Vector(std::sync::Arc::new(
                            parking_lot::Mutex::new(items),
                        )))
                    }
                    "make-symbol" => {
                        let name = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let s = name
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                        Ok(LispObject::symbol(s))
                    }
                    "fset" => {
                        let sym = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let def = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = sym
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        env.write().define(name, def.clone());
                        Ok(def)
                    }
                    "purecopy" => {
                        let arg = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        eval(arg, env, editor, macros, state)
                    }
                    "intern" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        match arg {
                            LispObject::String(s) => Ok(LispObject::symbol(&s)),
                            LispObject::Symbol(_) => Ok(arg),
                            _ => Err(ElispError::WrongTypeArgument("string".to_string())),
                        }
                    }
                    "intern-soft" => {
                        let arg = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = match &arg {
                            LispObject::String(s) => s.clone(),
                            LispObject::Symbol(s) => s.clone(),
                            _ => return Ok(LispObject::nil()),
                        };
                        // Only return the symbol if it is already known
                        if env.read().get(&name).is_some() {
                            Ok(LispObject::symbol(&name))
                        } else {
                            Ok(LispObject::nil())
                        }
                    }
                    "set" => {
                        let sym = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let val = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = sym
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        env.write().set(name, val.clone());
                        Ok(val)
                    }
                    "boundp" => {
                        let sym = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = sym
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        Ok(LispObject::from(env.read().get(name).is_some()))
                    }
                    "fboundp" => {
                        let sym = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let name = sym
                            .as_symbol()
                            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
                        Ok(LispObject::from(env.read().get(name).is_some()))
                    }
                    "symbol-plist" => {
                        let _sym = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        Ok(LispObject::nil())
                    }
                    "string-match-p" | "string-match" => {
                        let re_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let str_expr = cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
                        let re_str = eval(re_expr, env, editor, macros, state)?
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                            .clone();
                        let text = eval(str_expr, env, editor, macros, state)?
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                            .clone();
                        let start = if let Some(s) = cdr.nth(2) {
                            eval(s, env, editor, macros, state)?
                                .as_integer()
                                .unwrap_or(0) as usize
                        } else {
                            0
                        };
                        // Translate basic Emacs regex to Rust regex
                        let rust_re = emacs_regex_to_rust(&re_str);
                        match regex::Regex::new(&rust_re) {
                            Ok(re) => {
                                if let Some(m) = re.find(&text[start..]) {
                                    Ok(LispObject::integer((start + m.start()) as i64))
                                } else {
                                    Ok(LispObject::nil())
                                }
                            }
                            Err(_) => Ok(LispObject::nil()),
                        }
                    }
                    "match-data" | "match-beginning" | "match-end" | "match-string"
                    | "replace-match" | "looking-at" | "re-search-forward"
                    | "re-search-backward" | "search-forward" | "search-backward" => {
                        // Stub: return nil (no match data tracking yet)
                        Ok(LispObject::nil())
                    }
                    "version-to-list" => {
                        // Built-in version-to-list since subr.el's version needs match-data
                        let ver_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let ver = eval(ver_expr, env, editor, macros, state)?;
                        let ver_str = ver
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                        let mut result = LispObject::nil();
                        let parts: Vec<&str> = ver_str.split('.').collect();
                        for part in parts.into_iter().rev() {
                            let n = part.parse::<i64>().unwrap_or(0);
                            result = LispObject::cons(LispObject::integer(n), result);
                        }
                        Ok(result)
                    }
                    "read-from-string" => {
                        let str_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let s = eval(str_expr, env, editor, macros, state)?;
                        let text = s
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
                        let start = if let Some(start_expr) = cdr.nth(1) {
                            eval(start_expr, env, editor, macros, state)?
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
                                Ok(LispObject::cons(obj, LispObject::Integer(end_pos as i64)))
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
                        let s = eval(str_expr, env, editor, macros, state)?;
                        let text = s
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                            .clone();

                        // Evaluate optional separator (2nd arg)
                        let separator = if let Some(sep_expr) = cdr.nth(1) {
                            let sep_val = eval(sep_expr, env, editor, macros, state)?;
                            if sep_val.is_nil() {
                                None // nil means split on whitespace
                            } else {
                                sep_val.as_string().map(|s| s.to_string())
                            }
                        } else {
                            None
                        };

                        // Evaluate optional OMIT-NULLS (3rd arg)
                        let omit_nulls = if let Some(omit_expr) = cdr.nth(2) {
                            let omit_val = eval(omit_expr, env, editor, macros, state)?;
                            !omit_val.is_nil()
                        } else {
                            // Emacs default: when no separator is given, omit nulls
                            separator.is_none()
                        };

                        let parts: Vec<String> = match &separator {
                            None => {
                                // Default: split on whitespace (always omits nulls)
                                text.split_whitespace().map(|s| s.to_string()).collect()
                            }
                            Some(sep) => {
                                // Try as regex first, fall back to literal split
                                let rust_re = emacs_regex_to_rust(sep);
                                match regex::Regex::new(&rust_re) {
                                    Ok(re) => re.split(&text).map(|s| s.to_string()).collect(),
                                    Err(_) => {
                                        text.split(sep.as_str()).map(|s| s.to_string()).collect()
                                    }
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
                        Ok(result)
                    }
                    "mapconcat" => {
                        let func = eval(
                            cdr.first().ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let seq = eval(
                            cdr.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
                            env,
                            editor,
                            macros,
                            state,
                        )?;
                        let sep = if let Some(s) = cdr.nth(2) {
                            eval(s, env, editor, macros, state)?.princ_to_string()
                        } else {
                            String::new()
                        };
                        let mut parts = Vec::new();
                        let mut cur = seq;
                        while let Some((car, rest)) = cur.destructure_cons() {
                            let call_args = LispObject::cons(car, LispObject::nil());
                            parts.push(
                                call_function(&func, &call_args, env, editor, macros, state)?
                                    .princ_to_string(),
                            );
                            cur = rest;
                        }
                        Ok(LispObject::string(&parts.join(&sep)))
                    }
                    "defmacro" => eval_defmacro(&cdr, macros),
                    "macroexpand" => eval_macroexpand(&cdr, env, editor, macros, state),
                    _ => {
                        if let LispObject::Symbol(s) = &car {
                            let macro_table = macros.read();
                            if let Some(macro_) = macro_table.get(s.as_str()) {
                                let macro_ = macro_.clone();
                                drop(macro_table);
                                let expanded =
                                    expand_macro(&macro_, cdr, env, editor, macros, state)?;
                                return eval_next!(expanded, env, editor, macros, state);
                            }
                        }
                        eval_funcall(car, cdr, env, editor, macros, state)
                    }
                },
                _ => eval_funcall(car, cdr, env, editor, macros, state),
            }
        }
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
    eval_goto_char, eval_insert, eval_point, eval_save_buffer,
};
use error_forms::{
    eval_catch, eval_condition_case, eval_error_fn, eval_signal, eval_throw, eval_unwind_protect,
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
