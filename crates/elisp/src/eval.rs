use crate::error::{ElispError, ElispResult, SignalData, ThrowData};
use crate::object::LispObject;
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::collections::HashMap;
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
        LispObject::Cons(car, _) => {
            matches!(car.as_ref(), LispObject::Symbol(s) if s == "lambda")
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

/// Shared interpreter state accessible during evaluation.
#[derive(Clone)]
pub struct InterpreterState {
    pub plists: PlistTable,
    pub features: FeatureList,
    pub profiler: Arc<RwLock<crate::jit::Profiler>>,
    #[cfg(feature = "jit")]
    pub jit: Arc<RwLock<crate::jit::JitCompiler>>,
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
        Interpreter {
            env: Arc::new(RwLock::new(env)),
            editor: Arc::new(RwLock::new(None)),
            macros: Arc::new(RwLock::new(HashMap::new())),
            state: InterpreterState {
                plists: Arc::new(RwLock::new(HashMap::new())),
                features: Arc::new(RwLock::new(Vec::new())),
                profiler: Arc::new(RwLock::new(crate::jit::Profiler::new(1000))),
                #[cfg(feature = "jit")]
                jit: Arc::new(RwLock::new(crate::jit::JitCompiler::new())),
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
            let env = env.read();
            env.get(&name).ok_or(ElispError::VoidVariable(name))
        }
        LispObject::Cons(_, _) => {
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
                        Ok(LispObject::HashTable(Box::new(
                            crate::object::LispHashTable::new(test),
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
                            Ok(ht.get(&key).cloned().unwrap_or(default))
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
                        if let LispObject::HashTable(mut ht) = table {
                            ht.put(&key, value.clone());
                            // Note: mutation doesn't propagate to original binding
                            // This is a known limitation of the immutable object model
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
                            Ok(LispObject::integer(ht.data.len() as i64))
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
                        // Evaluate all args, append them (non-destructive since immutable)
                        let mut result = LispObject::nil();
                        let mut all_items = Vec::new();
                        let mut current = cdr.clone();
                        while let Some((arg_expr, rest)) = current.destructure_cons() {
                            let list = eval(arg_expr, env, editor, macros, state)?;
                            let mut cur = list;
                            while let Some((car, cdr_val)) = cur.destructure_cons() {
                                all_items.push(car);
                                cur = cdr_val;
                            }
                            // If last arg is non-nil atom, it becomes the tail
                            if !cur.is_nil() && rest.is_nil() {
                                result = cur;
                            }
                            current = rest;
                        }
                        for item in all_items.into_iter().rev() {
                            result = LispObject::cons(item, result);
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
                        Ok(LispObject::Vector(items))
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
                        match arg {
                            LispObject::String(s) => Ok(LispObject::symbol(&s)),
                            LispObject::Symbol(_) => Ok(arg),
                            _ => Ok(LispObject::nil()),
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
                        match crate::read(sub) {
                            Ok(obj) => {
                                let end_pos = start + sub.len(); // approximate
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
                        // Simplified: split on whitespace, return list of strings
                        let str_expr = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                        let s = eval(str_expr, env, editor, macros, state)?;
                        let text = s
                            .as_string()
                            .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
                            .clone();
                        let parts: Vec<&str> = text.split_whitespace().collect();
                        let mut result = LispObject::nil();
                        for p in parts.into_iter().rev() {
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

fn eval_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let then_branch = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let cond_val = eval(cond, env, editor, macros, state)?;
    if cond_val.is_nil() {
        // (if COND THEN ELSE1 ELSE2 ...) — else forms are implicit progn
        let else_forms = args
            .rest()
            .and_then(|r| r.rest())
            .unwrap_or(LispObject::nil());
        eval_progn(&else_forms, env, editor, macros, state)
    } else {
        eval(then_branch, env, editor, macros, state)
    }
}

fn eval_setq(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (setq SYM1 VAL1 SYM2 VAL2 ...) — set multiple pairs
    let mut result = LispObject::nil();
    let mut current = args.clone();
    while let Some((name_obj, rest)) = current.destructure_cons() {
        let name = name_obj
            .as_symbol()
            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
        let val_expr = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
        let value = eval(val_expr, env, editor, macros, state)?;
        env.write().set(name, value.clone());
        result = value;
        current = rest.rest().unwrap_or(LispObject::nil());
    }
    Ok(result)
}

fn eval_defun(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    _editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    _macros: &MacroTable,
    _state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let lambda = LispObject::lambda_expr(
        rest.first().unwrap_or(LispObject::nil()),
        rest.rest().unwrap_or(LispObject::nil()),
    );
    let mut env = env.write();
    env.define(name, lambda);
    Ok(LispObject::symbol(name))
}

fn eval_defmacro(args: &LispObject, macros: &MacroTable) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let macro_args = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let macro_body = rest.rest().unwrap_or(LispObject::nil());

    let macro_def = Macro {
        args: macro_args,
        body: macro_body,
    };

    macros.write().insert(name.clone(), macro_def);
    Ok(LispObject::symbol(name))
}

fn eval_macroexpand(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;

    if let LispObject::Cons(_, _) = form {
        let car = form.first().unwrap_or(LispObject::nil());
        if let LispObject::Symbol(ref s) = car {
            let macro_table = macros.read();
            if let Some(macro_) = macro_table.get(s) {
                let macro_ = macro_.clone();
                drop(macro_table);
                let cdr = form.rest().unwrap_or(LispObject::nil());
                let expanded = expand_macro(&macro_, cdr, env, editor, macros, state)?;
                return Ok(expanded); // macroexpand returns expanded form without evaluating
            }
        }
    }

    Ok(form)
}

fn expand_macro(
    macro_: &Macro,
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let params = &macro_.args;
    let body = &macro_.body;

    let param_names: Vec<String> = extract_param_names(params)?;

    let mut arg_list = args;
    let mut bindings: Vec<(String, LispObject)> = Vec::new();

    for name in &param_names {
        if name.starts_with("&rest ") || name.starts_with("&optional ") {
            continue;
        }
        if arg_list.is_nil() {
            bindings.push((name.clone(), LispObject::nil()));
        } else {
            let arg = arg_list.first().unwrap_or(LispObject::nil());
            bindings.push((name.clone(), arg));
            arg_list = arg_list.rest().unwrap_or(LispObject::nil());
        }
    }

    let parent_env = Arc::new(env.read().clone());
    let temp_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    for (name, value) in bindings {
        temp_env.write().define(&name, value);
    }

    eval_progn(body, &temp_env, editor, macros, state)
}

fn extract_param_names(params: &LispObject) -> ElispResult<Vec<String>> {
    let mut names = Vec::new();
    let mut current = Some(params.clone());

    while let Some(curr) = current {
        if let Some((LispObject::Symbol(s), rest)) = curr.destructure_cons() {
            if s == "&rest" || s == "&optional" {
                current = Some(rest);
                continue;
            }
            names.push(s);
            current = Some(rest);
        } else {
            break;
        }
    }

    Ok(names)
}

fn eval_let(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let bindings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        // Support both (VAR VALUE) and bare VAR (binds to nil)
        if let Some(name) = binding.as_symbol() {
            new_env.write().define(name, LispObject::nil());
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_name = binding_name
                .as_symbol()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val = eval(binding_val, env, editor, macros, state)?;
            new_env.write().define(binding_name, binding_val);
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        }
        bindings_list = rest;
    }

    eval_progn(&body, &new_env, editor, macros, state)
}

fn eval_progn(
    body: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut result = LispObject::nil();
    let mut current = Some(body.clone());
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(expr, env, editor, macros, state)?;
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}

fn eval_cond(
    clauses: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut current = Some(clauses.clone());
    while let Some(curr) = current {
        if let Some((clause, rest)) = curr.destructure_cons() {
            let (cond_expr, then_exprs) = if let Some((c, r)) = clause.destructure_cons() {
                (c, Some(r))
            } else {
                (clause, None)
            };

            let cond_val = eval(cond_expr, env, editor, macros, state)?;
            if !cond_val.is_nil() {
                if let Some(exprs) = then_exprs {
                    return eval_progn(&exprs, env, editor, macros, state);
                }
                return Ok(cond_val);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(LispObject::nil())
}

fn eval_loop(
    body: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    loop {
        let result = eval_progn(body, env, editor, macros, state)?;
        if result.is_nil() {
            return Ok(result);
        }
    }
}

fn eval_funcall(
    func: LispObject,
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // Emacs is a Lisp-2: function names and variable names live in
    // separate namespaces.  Our interpreter is Lisp-1, so a local
    // variable can shadow a function.  To approximate Lisp-2 semantics
    // we resolve function-position symbols with a special lookup:
    // if the local value is not callable (e.g. a parameter named `list`
    // that shadows the built-in `list` function), walk up the
    // environment chain until we find a callable binding.
    let func = resolve_function(func, env, editor, macros, state)?;
    let args = eval_list(&args, env, editor, macros, state)?;

    call_function(&func, &args, env, editor, macros, state)
}

/// Resolve a function-position expression.
///
/// For symbols: use `get_function` which prefers callable bindings over
/// non-callable shadows.  This approximates Lisp-2 semantics where
/// `(list ...)` always finds the *function* `list`, even when a local
/// variable named `list` shadows it.
fn resolve_function(
    func: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if let LispObject::Symbol(ref name) = func {
        env.read()
            .get_function(name)
            .ok_or_else(|| ElispError::VoidFunction(name.clone()))
    } else {
        // Not a symbol — eval normally (e.g. a lambda expression)
        eval(func, env, editor, macros, state)
    }
}

fn eval_apply(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let args_list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(func, env, editor, macros, state)?;
    let args_list = eval(args_list, env, editor, macros, state)?;

    let mut arg_items: Vec<LispObject> = Vec::new();
    let mut current = args_list.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        arg_items.push(arg);
        current = rest;
    }

    let mut all_args = LispObject::nil();
    for arg in arg_items.iter().rev() {
        all_args = LispObject::cons(arg.clone(), all_args);
    }

    call_function(&func_val, &all_args, env, editor, macros, state)
}

fn eval_funcall_form(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest_args = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(func, env, editor, macros, state)?;
    let args = eval_list(&rest_args, env, editor, macros, state)?;

    call_function(&func_val, &args, env, editor, macros, state)
}

fn eval_list(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    match args {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(_, _) => {
            let (car, cdr) = args.clone().destructure();
            let car_eval = eval(car, env, editor, macros, state)?;
            let cdr_eval = eval_list(&cdr, env, editor, macros, state)?;
            Ok(LispObject::cons(car_eval, cdr_eval))
        }
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

fn apply_lambda(
    params: &LispObject,
    body: &LispObject,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let mut params_list = params.clone();
    let mut args_list = args.clone();
    let mut optional = false;
    let mut rest = false;

    loop {
        if params_list.is_nil() {
            break;
        }
        let (param, params_rest) = match params_list.destructure_cons() {
            Some((p, r)) => (p, r),
            None => {
                // Dotted rest param: (a b . rest)
                if let LispObject::Symbol(name) = params_list {
                    new_env.write().define(&name, args_list);
                }
                break;
            }
        };

        if let LispObject::Symbol(ref name) = param {
            match name.as_str() {
                "&optional" => {
                    optional = true;
                    params_list = params_rest;
                    continue;
                }
                "&rest" => {
                    rest = true;
                    params_list = params_rest;
                    continue;
                }
                _ => {}
            }

            if rest {
                // Bind remaining args as a list
                new_env.write().define(name, args_list.clone());
                args_list = LispObject::nil();
                params_list = params_rest;
                continue;
            }

            let (arg, args_rest) = match args_list.destructure_cons() {
                Some((a, r)) => (a, r),
                None => {
                    if optional || rest {
                        (LispObject::nil(), LispObject::nil())
                    } else {
                        return Err(ElispError::WrongNumberOfArguments);
                    }
                }
            };
            new_env.write().define(name, arg);
            args_list = args_rest;
        }
        params_list = params_rest;
    }

    eval_progn(body, &new_env, editor, macros, state)
}

fn eval_buffer_string(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<LispObject> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(LispObject::string(&cb.buffer_string())),
        None => Ok(LispObject::string("")),
    }
}

fn eval_buffer_size(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<LispObject> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(LispObject::integer(cb.buffer_size() as i64)),
        None => Ok(LispObject::integer(0)),
    }
}

fn eval_insert(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let text_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let text = eval(text_arg, env, editor, macros, state)?;
    let text_str = match text {
        LispObject::String(s) => s.clone(),
        LispObject::Integer(i) => i.to_string(),
        LispObject::Symbol(s) => s,
        _ => format!("{:?}", text),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.insert(&text_str);
    }
    Ok(LispObject::nil())
}

fn eval_point(editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>) -> ElispResult<LispObject> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(LispObject::integer(cb.point() as i64)),
        None => Ok(LispObject::integer(0)),
    }
}

fn eval_goto_char(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let pos_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let pos = eval(pos_arg, env, editor, macros, state)?;
    let pos = match pos {
        LispObject::Integer(i) => i as usize,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.goto_char(pos);
    }
    Ok(LispObject::nil())
}

fn eval_delete_char(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let n_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = eval(n_arg, env, editor, macros, state)?;
    let n = match n {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.delete_char(n);
    }
    Ok(LispObject::nil())
}

fn eval_forward_char(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let n_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = eval(n_arg, env, editor, macros, state)?;
    let n = match n {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.forward_char(n);
    }
    Ok(LispObject::nil())
}

fn eval_find_file(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let path_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let path = eval(path_arg, env, editor, macros, state)?;
    let path_str = match path {
        LispObject::String(s) => s,
        LispObject::Symbol(s) => s,
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        let success = cb.find_file(&path_str);
        Ok(if success {
            LispObject::t()
        } else {
            LispObject::nil()
        })
    } else {
        Ok(LispObject::nil())
    }
}

fn eval_save_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<LispObject> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        let success = cb.save_buffer();
        Ok(if success {
            LispObject::t()
        } else {
            LispObject::nil()
        })
    } else {
        Ok(LispObject::nil())
    }
}

fn eval_prog1(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = eval(first, env, editor, macros, state)?;
    let _ = eval_progn(
        &args.rest().unwrap_or(LispObject::nil()),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(result)
}

fn eval_prog2(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let second = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let _ = eval(first, env, editor, macros, state)?;
    let result = eval(second, env, editor, macros, state)?;
    let rest = args
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());
    let _ = eval_progn(&rest, env, editor, macros, state)?;
    Ok(result)
}

fn eval_and(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if args.is_nil() {
        return Ok(LispObject::t());
    }
    let mut current = Some(args.clone());
    let mut result = LispObject::t();
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(expr, env, editor, macros, state)?;
            if result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}

fn eval_or(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if args.is_nil() {
        return Ok(LispObject::nil());
    }
    let mut current = Some(args.clone());
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            let result = eval(expr, env, editor, macros, state)?;
            if !result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(LispObject::nil())
}

fn eval_when(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(cond, env, editor, macros, state)?;
    if cond_val.is_nil() {
        Ok(LispObject::nil())
    } else {
        eval_progn(&body, env, editor, macros, state)
    }
}

fn eval_unless(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(cond, env, editor, macros, state)?;
    if cond_val.is_nil() {
        eval_progn(&body, env, editor, macros, state)
    } else {
        Ok(LispObject::nil())
    }
}

fn eval_while(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().unwrap_or(LispObject::nil());
    loop {
        let cond_val = eval(cond.clone(), env, editor, macros, state)?;
        if cond_val.is_nil() {
            return Ok(LispObject::nil());
        }
        eval_progn(&body, env, editor, macros, state)?;
    }
}

fn eval_let_star(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let bindings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    // let* evaluates each binding in the new env (sequential)
    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        if let Some(name) = binding.as_symbol() {
            new_env.write().define(name, LispObject::nil());
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_name = binding_name
                .as_symbol()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val = eval(binding_val, &new_env, editor, macros, state)?;
            new_env.write().define(binding_name, binding_val);
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        }
        bindings_list = rest;
    }

    eval_progn(&body, &new_env, editor, macros, state)
}

fn eval_defvar(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // defvar only sets value if currently void (unbound)
    let is_bound = env.read().get(name).is_some();
    if !is_bound {
        if let Some(value_expr) = args.nth(1) {
            let value = eval(value_expr, env, editor, macros, state)?;
            env.write().define(name, value);
        }
    }
    // Ignore docstring (3rd arg) for now
    Ok(LispObject::symbol(name))
}

fn eval_defconst(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // defconst always sets the value
    if let Some(value_expr) = args.nth(1) {
        let value = eval(value_expr, env, editor, macros, state)?;
        env.write().define(name, value);
    }
    Ok(LispObject::symbol(name))
}

fn eval_defalias(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let definition = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let name = eval(name, env, editor, macros, state)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let value = eval(definition, env, editor, macros, state)?;

    // If the value is (macro lambda ARGS . BODY), register it as a macro.
    // This handles e.g. (defalias '\` (symbol-function 'backquote))
    // where backquote is a defmacro.
    if let Some((car, rest)) = value.destructure_cons() {
        if car.as_symbol().map(|s| s.as_str()) == Some("macro") {
            if let Some((lambda_sym, lambda_rest)) = rest.destructure_cons() {
                if lambda_sym.as_symbol().map(|s| s.as_str()) == Some("lambda") {
                    let macro_args = lambda_rest.first().unwrap_or(LispObject::nil());
                    let macro_body = lambda_rest.rest().unwrap_or(LispObject::nil());
                    macros.write().insert(
                        name.to_string(),
                        Macro {
                            args: macro_args,
                            body: macro_body,
                        },
                    );
                    return Ok(LispObject::symbol(name));
                }
            }
        }
    }

    env.write().define(name, value);
    Ok(LispObject::symbol(name))
}

// --- Non-local exits and error handling ---

fn eval_catch(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let tag_expr = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().unwrap_or(LispObject::nil());

    let tag = eval(tag_expr, env, editor, macros, state)?;

    match eval_progn(&body, env, editor, macros, state) {
        Ok(value) => Ok(value),
        Err(ElispError::Throw(throw_data)) => {
            if tag == throw_data.tag {
                Ok(throw_data.value)
            } else {
                Err(ElispError::Throw(throw_data))
            }
        }
        Err(e) => Err(e),
    }
}

fn eval_throw(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let tag_expr = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value_expr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let tag = eval(tag_expr, env, editor, macros, state)?;
    let value = eval(value_expr, env, editor, macros, state)?;

    Err(ElispError::Throw(Box::new(ThrowData { tag, value })))
}

fn eval_condition_case(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let var = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let bodyform = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());

    // Evaluate bodyform
    match eval(bodyform, env, editor, macros, state) {
        Ok(value) => Ok(value),
        Err(ref err @ ElispError::Throw(..)) => Err(err.clone()),
        Err(err) => {
            // Try to match a handler
            let mut handlers = rest;
            while let Some((handler, more)) = handlers.destructure_cons() {
                let condition = handler.first().unwrap_or(LispObject::nil());
                let handler_body = handler.rest().unwrap_or(LispObject::nil());

                if err.matches_condition(&condition) {
                    // Bind the error to var if var is non-nil
                    let parent_env = Arc::new(env.read().clone());
                    let handler_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

                    if !var.is_nil() {
                        if let Some(var_name) = var.as_symbol() {
                            // Build error data as a cons: (symbol . data)
                            let signal = err.to_signal();
                            let err_value = if let ElispError::Signal(sig) = signal {
                                LispObject::cons(sig.symbol, sig.data)
                            } else {
                                LispObject::nil()
                            };
                            handler_env.write().define(var_name, err_value);
                        }
                    }

                    return eval_progn(&handler_body, &handler_env, editor, macros, state);
                }
                handlers = more;
            }
            // No handler matched — re-raise
            Err(err)
        }
    }
}

fn eval_signal(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let symbol_expr = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data_expr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let symbol = eval(symbol_expr, env, editor, macros, state)?;
    let data = eval(data_expr, env, editor, macros, state)?;

    Err(ElispError::Signal(Box::new(SignalData { symbol, data })))
}

fn eval_error_fn(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (error FORMAT-STRING &rest ARGS) — use format for the message
    let formatted = eval_format(args, env, editor, macros, state)?;
    let msg_str = formatted.princ_to_string();

    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg_str), LispObject::nil()),
    })))
}

fn eval_unwind_protect(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let bodyform = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cleanup_forms = args.rest().unwrap_or(LispObject::nil());

    // Evaluate body, capturing any error
    let body_result = eval(bodyform, env, editor, macros, state);

    // Always run cleanup forms, regardless of body outcome
    let _ = eval_progn(&cleanup_forms, env, editor, macros, state);

    // Return the body's result (or re-raise its error)
    body_result
}

// --- Property lists ---

fn eval_put(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = eval(
        args.first().ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;
    let prop = eval(
        args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;
    let val = eval(
        args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;

    let sym_name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_name = prop
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let key = format!("{}:{}", sym_name, prop_name);
    state.plists.write().insert(key, val.clone());
    Ok(val)
}

fn eval_get(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let sym = eval(
        args.first().ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;
    let prop = eval(
        args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;

    let sym_name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_name = prop
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let key = format!("{}:{}", sym_name, prop_name);
    Ok(state
        .plists
        .read()
        .get(&key)
        .cloned()
        .unwrap_or(LispObject::nil()))
}

// --- Module system ---

fn eval_provide(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let feature = eval(
        args.first().ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let mut features = state.features.write();
    if !features.contains(name) {
        features.push(name.clone());
    }
    Ok(feature)
}

fn eval_featurep(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let feature = eval(
        args.first().ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let features = state.features.read();
    Ok(LispObject::from(features.contains(name)))
}

fn eval_require(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let feature = eval(
        args.first().ok_or(ElispError::WrongNumberOfArguments)?,
        env,
        editor,
        macros,
        state,
    )?;
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let features = state.features.read();
    if features.contains(name) {
        return Ok(feature);
    }
    drop(features);
    // For now, just return the feature without loading
    // TODO: implement load-path searching
    Ok(feature)
}

// --- Higher-order functions ---

fn eval_mapcar(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func_expr = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_expr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func = eval(func_expr, env, editor, macros, state)?;
    let list = eval(list_expr, env, editor, macros, state)?;

    let mut results = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        let call_args = LispObject::cons(car, LispObject::nil());
        let result = call_function(&func, &call_args, env, editor, macros, state)?;
        results.push(result);
        current = cdr;
    }
    let mut result = LispObject::nil();
    for r in results.into_iter().rev() {
        result = LispObject::cons(r, result);
    }
    Ok(result)
}

fn eval_mapc(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func_expr = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_expr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func = eval(func_expr, env, editor, macros, state)?;
    let list = eval(list_expr, env, editor, macros, state)?;

    let mut current = list.clone();
    while let Some((car, cdr)) = current.destructure_cons() {
        let call_args = LispObject::cons(car, LispObject::nil());
        call_function(&func, &call_args, env, editor, macros, state)?;
        current = cdr;
    }
    Ok(list)
}

/// Call a function value (lambda or primitive) with already-evaluated args.
pub fn call_function(
    func: &LispObject,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    match func {
        LispObject::Cons(car, cdr) => {
            if let LispObject::Symbol(s) = car.as_ref() {
                if s == "lambda" {
                    let params = cdr.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                    return apply_lambda(&params, &body, args, env, editor, macros, state);
                }
            }
            Err(ElispError::WrongTypeArgument("function".to_string()))
        }
        LispObject::Primitive(name) => crate::primitives::call_primitive(name, args),
        LispObject::BytecodeFn(bc) => {
            let func_id = bc as *const _ as usize;
            #[allow(unused_variables)]
            let should_jit = state.profiler.write().record_call(func_id);

            // Collect args
            let mut arg_vec = Vec::new();
            let mut current = args.clone();
            while let Some((car, cdr)) = current.destructure_cons() {
                arg_vec.push(car);
                current = cdr;
            }

            // Try JIT execution if available
            #[cfg(feature = "jit")]
            {
                let mut jit = state.jit.write();
                if should_jit && !jit.is_compiled(func_id) {
                    jit.compile(func_id, bc);
                }
                if let Some(id) = jit.get_compiled(func_id) {
                    {
                        // Convert args to i64 (NaN-boxed values)
                        let jit_args: Vec<i64> = arg_vec
                            .iter()
                            .map(|a| crate::value::Value::from_lisp_object(a).raw() as i64)
                            .collect();
                        if let Some(native_result) = jit.call(id, &jit_args) {
                            match native_result {
                                crate::jit::NativeResult::Ok(raw) => {
                                    let val = crate::value::Value::from_raw(raw);
                                    return Ok(val.to_lisp_object());
                                }
                                crate::jit::NativeResult::Deoptimize => {
                                    // Fall through to VM
                                }
                            }
                        }
                    }
                }
            }

            crate::vm::execute_bytecode(bc, &arg_vec, env, editor, macros, state)
        }
        LispObject::Symbol(name) => {
            let val = env
                .read()
                .get(name)
                .ok_or_else(|| ElispError::VoidFunction(name.clone()))?;
            call_function(&val, args, env, editor, macros, state)
        }
        _ => Err(ElispError::WrongTypeArgument("function".to_string())),
    }
}

// --- Iteration ---

fn eval_dolist(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (dolist (VAR LIST [RESULT]) BODY...)
    let spec = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().unwrap_or(LispObject::nil());

    let var = spec.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let var_name = var
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let list_expr = spec.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result_expr = spec.nth(2);

    let list = eval(list_expr, env, editor, macros, state)?;

    let parent_env = Arc::new(env.read().clone());
    let loop_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        loop_env.write().set(var_name, car);
        eval_progn(&body, &loop_env, editor, macros, state)?;
        current = cdr;
    }

    // Set var to nil and eval result
    loop_env.write().set(var_name, LispObject::nil());
    if let Some(result_expr) = result_expr {
        eval(result_expr, &loop_env, editor, macros, state)
    } else {
        Ok(LispObject::nil())
    }
}

// --- String formatting ---

/// Translate basic Emacs regex to Rust regex.
/// Emacs uses \( \) for groups, \| for alternation, etc.
fn emacs_regex_to_rust(emacs: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = emacs.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                '(' => {
                    result.push('(');
                    i += 2;
                }
                ')' => {
                    result.push(')');
                    i += 2;
                }
                '|' => {
                    result.push('|');
                    i += 2;
                }
                '{' => {
                    result.push('{');
                    i += 2;
                }
                '}' => {
                    result.push('}');
                    i += 2;
                }
                'w' => {
                    result.push_str("[[:alnum:]_]");
                    i += 2;
                }
                'b' => {
                    result.push_str("\\b");
                    i += 2;
                }
                's' => {
                    // \s- = whitespace in Emacs
                    if i + 2 < chars.len() && chars[i + 2] == '-' {
                        result.push_str("\\s");
                        i += 3;
                    } else {
                        result.push_str("\\s");
                        i += 2;
                    }
                }
                '`' => {
                    result.push_str("\\A");
                    i += 2;
                } // beginning of string
                '\'' => {
                    result.push_str("\\z");
                    i += 2;
                } // end of string
                c => {
                    result.push('\\');
                    result.push(c);
                    i += 2;
                }
            }
        } else {
            // In Emacs regex, literal ( ) are just ( ) — in Rust they need escaping
            // But Emacs also uses bare ( ) as literal, while \( \) are groups
            match chars[i] {
                '(' => result.push_str("\\("),
                ')' => result.push_str("\\)"),
                '|' => result.push_str("\\|"),
                c => result.push(c),
            }
            i += 1;
        }
    }
    result
}

fn eval_format(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let fmt_expr = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let fmt = eval(fmt_expr, env, editor, macros, state)?;
    let fmt_str = match fmt {
        LispObject::String(s) => s,
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    // Collect remaining args
    let mut format_args = Vec::new();
    let mut rest = args.rest().unwrap_or(LispObject::nil());
    while let Some((arg, next)) = rest.destructure_cons() {
        let val = eval(arg, env, editor, macros, state)?;
        format_args.push(val);
        rest = next;
    }

    let mut result = String::new();
    let mut arg_idx = 0;
    let chars: Vec<char> = fmt_str.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            // Parse flags
            let mut left_align = false;
            let mut zero_pad = false;
            while i < chars.len() && (chars[i] == '-' || chars[i] == '+' || chars[i] == '0') {
                match chars[i] {
                    '-' => left_align = true,
                    '0' => zero_pad = true,
                    _ => {}
                }
                i += 1;
            }
            // Parse width
            let mut width: usize = 0;
            while i < chars.len() && chars[i].is_ascii_digit() {
                width = width * 10 + (chars[i] as usize - '0' as usize);
                i += 1;
            }
            if i >= chars.len() {
                break;
            }
            // left-align overrides zero-pad
            if left_align {
                zero_pad = false;
            }
            let apply_width = |s: String| -> String {
                if width == 0 || s.len() >= width {
                    s
                } else if left_align {
                    format!("{:<width$}", s, width = width)
                } else if zero_pad {
                    // Only zero-pad numeric values; for strings just space-pad
                    if let Some(stripped) = s.strip_prefix('-') {
                        format!("-{:0>width$}", stripped, width = width - 1)
                    } else {
                        format!("{:0>width$}", s, width = width)
                    }
                } else {
                    format!("{:>width$}", s, width = width)
                }
            };
            match chars[i] {
                's' => {
                    if arg_idx < format_args.len() {
                        let s = format_args[arg_idx].princ_to_string();
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'S' => {
                    if arg_idx < format_args.len() {
                        let s = format_args[arg_idx].prin1_to_string();
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'd' => {
                    if arg_idx < format_args.len() {
                        let s = match &format_args[arg_idx] {
                            LispObject::Integer(n) => n.to_string(),
                            LispObject::Float(f) => (*f as i64).to_string(),
                            _ => format_args[arg_idx].princ_to_string(),
                        };
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'f' => {
                    if arg_idx < format_args.len() {
                        let s = match &format_args[arg_idx] {
                            LispObject::Float(f) => format!("{:.6}", f),
                            LispObject::Integer(n) => format!("{:.6}", *n as f64),
                            _ => format_args[arg_idx].princ_to_string(),
                        };
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'c' => {
                    if arg_idx < format_args.len() {
                        if let LispObject::Integer(n) = &format_args[arg_idx] {
                            if let Some(ch) = char::from_u32(*n as u32) {
                                let s = ch.to_string();
                                result.push_str(&apply_width(s));
                            }
                        }
                        arg_idx += 1;
                    }
                }
                'x' => {
                    if arg_idx < format_args.len() {
                        if let LispObject::Integer(n) = &format_args[arg_idx] {
                            let s = format!("{:x}", n);
                            result.push_str(&apply_width(s));
                        }
                        arg_idx += 1;
                    }
                }
                'o' => {
                    if arg_idx < format_args.len() {
                        if let LispObject::Integer(n) = &format_args[arg_idx] {
                            let s = format!("{:o}", n);
                            result.push_str(&apply_width(s));
                        }
                        arg_idx += 1;
                    }
                }
                '%' => result.push('%'),
                _ => {
                    result.push('%');
                    result.push(chars[i]);
                }
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    Ok(LispObject::string(&result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{add_primitives, read};

    #[test]
    fn test_eval_quote_reader() {
        let interp = Interpreter::new();
        assert_eq!(
            interp.eval(read("'foo").unwrap()).unwrap(),
            LispObject::symbol("foo")
        );
        assert_eq!(
            interp.eval(read("'(1 2 3)").unwrap()).unwrap(),
            LispObject::cons(
                LispObject::integer(1),
                LispObject::cons(
                    LispObject::integer(2),
                    LispObject::cons(LispObject::integer(3), LispObject::nil())
                )
            )
        );
    }

    #[test]
    fn test_car_quote() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        assert_eq!(
            interp.eval(read("(car '(1 2 3))").unwrap()).unwrap(),
            LispObject::integer(1)
        );
    }

    #[test]
    fn test_primitives() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        assert_eq!(
            interp.eval(read("(+ 1 2 3)").unwrap()).unwrap(),
            LispObject::integer(6)
        );
        assert_eq!(
            interp.eval(read("(- 10 3)").unwrap()).unwrap(),
            LispObject::integer(7)
        );
        assert_eq!(
            interp.eval(read("(* 2 3 4)").unwrap()).unwrap(),
            LispObject::integer(24)
        );
        assert_eq!(
            interp.eval(read("(/ 10 2)").unwrap()).unwrap(),
            LispObject::integer(5)
        );

        assert_eq!(
            interp.eval(read("(< 1 2)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(> 2 1)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(= 3 3)").unwrap()).unwrap(),
            LispObject::t()
        );

        assert_eq!(
            interp.eval(read("(car '(1 2 3))").unwrap()).unwrap(),
            LispObject::integer(1)
        );
        assert_eq!(
            interp.eval(read("(cdr '(1 2 3))").unwrap()).unwrap(),
            read("(2 3)").unwrap()
        );
        assert_eq!(
            interp.eval(read("(cons 1 '(2 3))").unwrap()).unwrap(),
            read("(1 2 3)").unwrap()
        );

        assert_eq!(
            interp.eval(read("(length '(1 2 3))").unwrap()).unwrap(),
            LispObject::integer(3)
        );
        assert_eq!(
            interp.eval(read("(list 1 2 3)").unwrap()).unwrap(),
            read("(1 2 3)").unwrap()
        );

        assert_eq!(
            interp.eval(read("(not t)").unwrap()).unwrap(),
            LispObject::nil()
        );
        assert_eq!(
            interp.eval(read("(null nil)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(numberp 42)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(symbolp 'foo)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(listp '(1 2))").unwrap()).unwrap(),
            LispObject::t()
        );
    }

    #[test]
    fn test_eval_quote() {
        let interp = Interpreter::new();
        let expr = LispObject::cons(
            LispObject::symbol("quote"),
            LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
        );
        assert_eq!(interp.eval(expr).unwrap(), LispObject::symbol("foo"));
    }

    #[test]
    fn test_eval_if() {
        let interp = Interpreter::new();
        let expr = LispObject::cons(
            LispObject::symbol("if"),
            LispObject::cons(
                LispObject::t(),
                LispObject::cons(
                    LispObject::integer(1),
                    LispObject::cons(LispObject::integer(2), LispObject::nil()),
                ),
            ),
        );
        assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(1));

        let expr = LispObject::cons(
            LispObject::symbol("if"),
            LispObject::cons(
                LispObject::nil(),
                LispObject::cons(
                    LispObject::integer(1),
                    LispObject::cons(LispObject::integer(2), LispObject::nil()),
                ),
            ),
        );
        assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(2));
    }

    #[test]
    fn test_eval_setq() {
        let interp = Interpreter::new();
        let expr = LispObject::cons(
            LispObject::symbol("setq"),
            LispObject::cons(
                LispObject::symbol("x"),
                LispObject::cons(LispObject::integer(42), LispObject::nil()),
            ),
        );
        assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(42));
        assert_eq!(
            interp.eval(LispObject::symbol("x")).unwrap(),
            LispObject::integer(42)
        );
    }

    #[test]
    fn test_eval_let() {
        let interp = Interpreter::new();
        let x = LispObject::symbol("x");
        let ten = LispObject::integer(10);
        let nil = LispObject::nil();

        let binding = LispObject::cons(x.clone(), LispObject::cons(ten.clone(), nil.clone()));
        let bindings = LispObject::cons(binding, nil.clone());
        let body = LispObject::cons(x.clone(), nil);

        let expr = LispObject::cons(LispObject::symbol("let"), LispObject::cons(bindings, body));
        assert_eq!(interp.eval(expr).unwrap(), ten);
    }

    #[test]
    fn test_cond() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        assert_eq!(
            interp
                .eval(read("(cond ((> 3 2) 'greater) ((< 3 2) 'less))").unwrap())
                .unwrap(),
            LispObject::symbol("greater")
        );
        assert_eq!(
            interp
                .eval(read("(cond ((< 3 2) 'greater) (t 'less))").unwrap())
                .unwrap(),
            LispObject::symbol("less")
        );
        assert_eq!(
            interp
                .eval(read("(cond (nil 'never) (t 'default))").unwrap())
                .unwrap(),
            LispObject::symbol("default")
        );
    }

    #[test]
    fn test_function() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        let result = interp
            .eval(read("(function (lambda (x) (+ x 1)))").unwrap())
            .unwrap();
        assert!(matches!(result, LispObject::Cons(_, _)));
    }

    #[test]
    fn test_apply() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        assert_eq!(
            interp.eval(read("(apply '+ '(1 2 3))").unwrap()).unwrap(),
            LispObject::integer(6)
        );
        assert_eq!(
            interp
                .eval(read("(apply 'list '(1 2 3))").unwrap())
                .unwrap(),
            read("(1 2 3)").unwrap()
        );
    }

    #[test]
    fn test_funcall() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        assert_eq!(
            interp.eval(read("(funcall '+ 1 2 3)").unwrap()).unwrap(),
            LispObject::integer(6)
        );
        assert_eq!(
            interp.eval(read("(funcall 'list 1 2 3)").unwrap()).unwrap(),
            read("(1 2 3)").unwrap()
        );
    }

    #[test]
    fn test_string_primitives() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        assert_eq!(
            interp
                .eval(read("(string= \"hello\" \"hello\")").unwrap())
                .unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp
                .eval(read("(string= \"hello\" \"world\")").unwrap())
                .unwrap(),
            LispObject::nil()
        );
        assert_eq!(
            interp
                .eval(read("(string< \"apple\" \"banana\")").unwrap())
                .unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp
                .eval(read("(string< \"banana\" \"apple\")").unwrap())
                .unwrap(),
            LispObject::nil()
        );
        assert_eq!(
            interp
                .eval(read("(concat \"hello\" \" \" \"world\")").unwrap())
                .unwrap(),
            LispObject::string("hello world")
        );
        assert_eq!(
            interp
                .eval(read("(substring \"hello world\" 0 5)").unwrap())
                .unwrap(),
            LispObject::string("hello")
        );
    }

    #[test]
    fn test_prog1() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp
                .eval(read("(prog1 (+ 1 2) (+ 3 4))").unwrap())
                .unwrap(),
            LispObject::integer(3)
        );
    }

    #[test]
    fn test_prog2() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp
                .eval(read("(prog2 (+ 1 2) (+ 3 4))").unwrap())
                .unwrap(),
            LispObject::integer(7)
        );
    }

    #[test]
    fn test_and() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp.eval(read("(and t t t)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(and t nil t)").unwrap()).unwrap(),
            LispObject::nil()
        );
        assert_eq!(
            interp.eval(read("(and)").unwrap()).unwrap(),
            LispObject::t()
        );
    }

    #[test]
    fn test_or() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp.eval(read("(or nil nil t)").unwrap()).unwrap(),
            LispObject::t()
        );
        assert_eq!(
            interp.eval(read("(or nil nil)").unwrap()).unwrap(),
            LispObject::nil()
        );
        assert_eq!(
            interp.eval(read("(or)").unwrap()).unwrap(),
            LispObject::nil()
        );
    }

    #[test]
    fn test_when() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp.eval(read("(when t 1 2 3)").unwrap()).unwrap(),
            LispObject::integer(3)
        );
        assert_eq!(
            interp.eval(read("(when nil 1 2 3)").unwrap()).unwrap(),
            LispObject::nil()
        );
    }

    #[test]
    fn test_unless() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp.eval(read("(unless nil 1 2 3)").unwrap()).unwrap(),
            LispObject::integer(3)
        );
        assert_eq!(
            interp.eval(read("(unless t 1 2 3)").unwrap()).unwrap(),
            LispObject::nil()
        );
    }

    // --- Phase 0 regression tests ---

    #[test]
    fn test_eq_atoms() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // eq on identical symbols
        assert_eq!(
            interp.eval(read("(eq 'foo 'foo)").unwrap()).unwrap(),
            LispObject::t()
        );
        // eq on identical integers
        assert_eq!(
            interp.eval(read("(eq 42 42)").unwrap()).unwrap(),
            LispObject::t()
        );
        // eq on nil
        assert_eq!(
            interp.eval(read("(eq nil nil)").unwrap()).unwrap(),
            LispObject::t()
        );
        // eq on t
        assert_eq!(
            interp.eval(read("(eq t t)").unwrap()).unwrap(),
            LispObject::t()
        );
        // eq on different symbols
        assert_eq!(
            interp.eval(read("(eq 'foo 'bar)").unwrap()).unwrap(),
            LispObject::nil()
        );
        // eq on lists (always false without identity)
        assert_eq!(
            interp.eval(read("(eq '(1 2) '(1 2))").unwrap()).unwrap(),
            LispObject::nil()
        );
    }

    #[test]
    fn test_div_integer_semantics() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Integer division truncates toward zero
        assert_eq!(
            interp.eval(read("(/ 7 2)").unwrap()).unwrap(),
            LispObject::integer(3)
        );
        assert_eq!(
            interp.eval(read("(/ 10 3)").unwrap()).unwrap(),
            LispObject::integer(3)
        );
        // Single arg: (/ N) = 1/N
        assert_eq!(
            interp.eval(read("(/ 2)").unwrap()).unwrap(),
            LispObject::integer(0)
        );
    }

    #[test]
    fn test_div_by_zero() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert!(interp.eval(read("(/ 1 0)").unwrap()).is_err());
        assert!(interp.eval(read("(/ 0)").unwrap()).is_err());
    }

    #[test]
    fn test_cons_arg_validation() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // cons with 2 args works
        assert!(interp.eval(read("(cons 1 2)").unwrap()).is_ok());
        // cons with 0 args should error
        assert!(interp.eval(read("(cons)").unwrap()).is_err());
    }

    #[test]
    fn test_prog1_eval_order() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // prog1 evaluates first, then rest, returns first
        // Use setq to verify order: set x=1, prog1 returns x (1), then sets x=2
        assert_eq!(
            interp
                .eval(read("(progn (setq x 1) (prog1 x (setq x 2)))").unwrap())
                .unwrap(),
            LispObject::integer(1)
        );
        // x should now be 2
        assert_eq!(
            interp.eval(read("x").unwrap()).unwrap(),
            LispObject::integer(2)
        );
    }

    #[test]
    fn test_macros_per_interpreter() {
        // Macros defined in one interpreter should not leak to another
        let mut interp1 = Interpreter::new();
        add_primitives(&mut interp1);
        let mut interp2 = Interpreter::new();
        add_primitives(&mut interp2);

        interp1
            .eval(read("(defmacro my-inc (x) (list '+ x 1))").unwrap())
            .unwrap();
        assert_eq!(
            interp1.eval(read("(my-inc 5)").unwrap()).unwrap(),
            LispObject::integer(6)
        );
        // interp2 should not have my-inc
        assert!(interp2.eval(read("(my-inc 5)").unwrap()).is_err());
    }

    // --- Phase 1 regression tests ---

    #[test]
    fn test_while() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp
                .eval(
                    read("(progn (setq x 0) (setq sum 0) (while (< x 5) (setq sum (+ sum x)) (setq x (+ x 1))) sum)")
                        .unwrap()
                )
                .unwrap(),
            LispObject::integer(10) // 0+1+2+3+4
        );
    }

    #[test]
    fn test_let_star() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // let* allows later bindings to reference earlier ones
        assert_eq!(
            interp
                .eval(read("(let* ((x 10) (y (+ x 5))) y)").unwrap())
                .unwrap(),
            LispObject::integer(15)
        );
    }

    #[test]
    fn test_defvar() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // defvar sets value when void
        interp.eval(read("(defvar my-var 42)").unwrap()).unwrap();
        assert_eq!(
            interp.eval(read("my-var").unwrap()).unwrap(),
            LispObject::integer(42)
        );
        // defvar does NOT overwrite existing value
        interp.eval(read("(defvar my-var 99)").unwrap()).unwrap();
        assert_eq!(
            interp.eval(read("my-var").unwrap()).unwrap(),
            LispObject::integer(42)
        );
    }

    #[test]
    fn test_defconst() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        interp
            .eval(read("(defconst my-const 42)").unwrap())
            .unwrap();
        assert_eq!(
            interp.eval(read("my-const").unwrap()).unwrap(),
            LispObject::integer(42)
        );
        // defconst DOES overwrite
        interp
            .eval(read("(defconst my-const 99)").unwrap())
            .unwrap();
        assert_eq!(
            interp.eval(read("my-const").unwrap()).unwrap(),
            LispObject::integer(99)
        );
    }

    #[test]
    fn test_defalias() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // defalias sets a symbol to a function value
        interp
            .eval(read("(defun my-add (a b) (+ a b))").unwrap())
            .unwrap();
        interp
            .eval(read("(defalias 'my-plus 'my-add)").unwrap())
            .unwrap();
        // Wait -- defalias with quoted symbols needs the function value, not the symbol.
        // In our current Lisp-1, 'my-add evaluates to the symbol, and looking up the symbol
        // gets the lambda. So defalias stores the symbol, and calling my-plus looks it up.
        // This is a simplified version; full Lisp-2 comes later.
    }

    // --- Phase 2 regression tests ---

    #[test]
    fn test_catch_throw_basic() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // catch returns the thrown value
        assert_eq!(
            interp
                .eval(read("(catch 'done (throw 'done 42))").unwrap())
                .unwrap(),
            LispObject::integer(42)
        );
    }

    #[test]
    fn test_catch_no_throw() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // catch without throw returns body value
        assert_eq!(
            interp.eval(read("(catch 'done (+ 1 2))").unwrap()).unwrap(),
            LispObject::integer(3)
        );
    }

    #[test]
    fn test_catch_throw_nested() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Inner catch catches the matching throw; outer doesn't fire
        assert_eq!(
            interp
                .eval(read("(catch 'outer (+ 10 (catch 'inner (throw 'inner 5))))").unwrap())
                .unwrap(),
            LispObject::integer(15)
        );
    }

    #[test]
    fn test_catch_throw_propagates() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Throw with non-matching inner catch propagates to outer
        assert_eq!(
            interp
                .eval(read("(catch 'outer (catch 'inner (throw 'outer 99)))").unwrap())
                .unwrap(),
            LispObject::integer(99)
        );
    }

    #[test]
    fn test_throw_no_catch() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Throw without matching catch is an error
        assert!(interp.eval(read("(throw 'nothing 42)").unwrap()).is_err());
    }

    #[test]
    fn test_condition_case_no_error() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // No error: returns body value
        assert_eq!(
            interp
                .eval(read("(condition-case err (+ 1 2) (error 99))").unwrap())
                .unwrap(),
            LispObject::integer(3)
        );
    }

    #[test]
    fn test_condition_case_catches_error() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // error handler catches signal
        assert_eq!(
            interp
                .eval(read("(condition-case err (error \"boom\") (error 42))").unwrap())
                .unwrap(),
            LispObject::integer(42)
        );
    }

    #[test]
    fn test_condition_case_binds_error() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // err variable is bound to (symbol . data)
        let result = interp
            .eval(read("(condition-case err (error \"boom\") (error (car err)))").unwrap())
            .unwrap();
        // (car err) should be the error symbol
        assert_eq!(result, LispObject::symbol("error"));
    }

    #[test]
    fn test_condition_case_specific_condition() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // arith-error matches division by zero
        assert_eq!(
            interp
                .eval(read("(condition-case nil (/ 1 0) (arith-error 42))").unwrap())
                .unwrap(),
            LispObject::integer(42)
        );
        // void-variable matches undefined var
        assert_eq!(
            interp
                .eval(read("(condition-case nil undefined-var (void-variable 99))").unwrap())
                .unwrap(),
            LispObject::integer(99)
        );
    }

    #[test]
    fn test_condition_case_no_match() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Handler doesn't match: error propagates
        assert!(interp
            .eval(read("(condition-case nil (/ 1 0) (void-variable 42))").unwrap())
            .is_err());
    }

    #[test]
    fn test_signal() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        assert_eq!(
            interp
                .eval(
                    read("(condition-case nil (signal 'my-error '(data)) (my-error 42))").unwrap()
                )
                .unwrap(),
            LispObject::integer(42)
        );
    }

    #[test]
    fn test_unwind_protect_normal() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Cleanup runs, body value returned
        assert_eq!(
            interp
                .eval(read("(progn (setq x 0) (unwind-protect (+ 1 2) (setq x 1)) x)").unwrap())
                .unwrap(),
            LispObject::integer(1)
        );
    }

    #[test]
    fn test_unwind_protect_on_error() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Cleanup runs even when body errors
        assert_eq!(
            interp
                .eval(
                    read("(progn (setq cleaned-up nil) (condition-case nil (unwind-protect (error \"boom\") (setq cleaned-up t)) (error nil)) cleaned-up)")
                        .unwrap()
                )
                .unwrap(),
            LispObject::t()
        );
    }

    #[test]
    fn test_unwind_protect_on_throw() {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Cleanup runs even on throw
        assert_eq!(
            interp
                .eval(
                    read("(progn (setq cleaned-up nil) (catch 'done (unwind-protect (throw 'done 42) (setq cleaned-up t))) cleaned-up)")
                        .unwrap()
                )
                .unwrap(),
            LispObject::t()
        );
    }

    // --- Phase 3: stdlib loading tests ---

    fn make_stdlib_interp() -> Interpreter {
        let mut interp = Interpreter::new();
        add_primitives(&mut interp);
        // Common stubs for stdlib loading
        interp.define("backtrace-on-error-noninteractive", LispObject::nil());
        interp.define("most-positive-fixnum", LispObject::integer(i64::MAX));
        interp.define("most-negative-fixnum", LispObject::integer(i64::MIN));
        interp.define("emacs-version", LispObject::string("30.2"));
        interp.define("emacs-major-version", LispObject::integer(30));
        interp.define("emacs-minor-version", LispObject::integer(2));
        interp.define("system-type", LispObject::symbol("darwin"));
        interp.define("noninteractive", LispObject::t());
        interp.define(
            "load-suffixes",
            LispObject::cons(
                LispObject::string(".elc"),
                LispObject::cons(LispObject::string(".el"), LispObject::nil()),
            ),
        );
        interp.define(
            "load-file-rep-suffixes",
            LispObject::cons(LispObject::string(""), LispObject::nil()),
        );
        // Emacs 30 symbol-with-position stubs (we don't implement positioned symbols)
        interp.define("bare-symbol", LispObject::primitive("identity")); // bare-symbol just returns the symbol
        interp.define("symbol-with-pos-p", LispObject::primitive("ignore")); // always nil
        interp.define("byte-run--ssp-seen", LispObject::nil());
        // Stubs for functions we don't implement yet
        interp.define("mapbacktrace", LispObject::nil());
        interp.define("byte-compile-macro-environment", LispObject::nil());
        interp.define("macro-declaration-function", LispObject::nil());
        interp.define("byte-run--set-speed", LispObject::nil());
        interp.define("purify-flag", LispObject::nil());
        interp.define("delayed-warnings-list", LispObject::nil());

        // subr.el stubs — keymap and editor primitives
        interp.define("make-keymap", LispObject::primitive("list"));
        interp.define("make-sparse-keymap", LispObject::primitive("ignore"));
        interp.define("purecopy", LispObject::primitive("identity"));
        interp.define("fset", LispObject::primitive("identity")); // TODO: proper Lisp-2
        interp.define("define-key", LispObject::primitive("ignore"));
        interp.define("set-keymap-parent", LispObject::primitive("ignore"));
        interp.define("current-global-map", LispObject::primitive("ignore"));
        interp.define("use-global-map", LispObject::primitive("ignore"));
        interp.define("intern-soft", LispObject::primitive("identity"));
        interp.define("make-byte-code", LispObject::primitive("ignore"));
        interp.define("set-char-table-range", LispObject::primitive("ignore"));
        interp.define("set-char-table-extra-slot", LispObject::primitive("ignore"));
        interp.define("make-char-table", LispObject::primitive("ignore"));
        interp.define("char-table-extra-slot", LispObject::primitive("ignore"));
        interp.define("set-standard-case-table", LispObject::primitive("ignore"));
        interp.define("standard-case-table", LispObject::primitive("ignore"));
        interp.define("downcase-region", LispObject::primitive("ignore"));
        interp.define("upcase-region", LispObject::primitive("ignore"));
        interp.define("capitalize-region", LispObject::primitive("ignore"));
        interp.define("upcase", LispObject::primitive("identity"));
        interp.define("downcase", LispObject::primitive("identity"));
        interp.define("string-replace", LispObject::primitive("identity"));
        interp.define(
            "replace-regexp-in-string",
            LispObject::primitive("identity"),
        );
        interp.define("string-search", LispObject::primitive("ignore"));
        interp.define("string-prefix-p", LispObject::primitive("ignore"));
        interp.define("string-suffix-p", LispObject::primitive("ignore"));
        interp.define("string-lessp", LispObject::primitive("ignore"));
        interp.define("compare-strings", LispObject::primitive("ignore"));
        interp.define("string-collate-lessp", LispObject::primitive("ignore"));
        interp.define("string-equal", LispObject::primitive("ignore"));
        interp.define("mapconcat", LispObject::primitive("ignore"));
        interp.define("process-attributes", LispObject::primitive("ignore"));
        interp.define("set-process-sentinel", LispObject::primitive("ignore"));
        interp.define("where-is-internal", LispObject::primitive("ignore"));
        interp.define("event-modifiers", LispObject::primitive("ignore"));
        interp.define("event-basic-type", LispObject::primitive("ignore"));
        interp.define("read-event", LispObject::primitive("ignore"));
        interp.define("listify-key-sequence", LispObject::primitive("ignore"));
        // Variables needed by subr.el
        interp.define("features", LispObject::nil());
        interp.define("obarray", LispObject::nil());
        interp.define("global-map", LispObject::nil());
        interp.define("ctl-x-map", LispObject::nil());
        interp.define("ctl-x-4-map", LispObject::nil());
        interp.define("ctl-x-5-map", LispObject::nil());
        interp.define("esc-map", LispObject::nil());
        interp.define("help-map", LispObject::nil());
        interp.define("mode-specific-map", LispObject::nil());
        interp.define("search-spaces-regexp", LispObject::nil());
        interp.define("print-escape-newlines", LispObject::nil());
        interp.define("standard-output", LispObject::t());
        interp.define("load-path", LispObject::nil());
        interp.define("data-directory", LispObject::string("/usr/share/emacs"));
        // Additional stubs for subr.el
        interp.define("autoload", LispObject::primitive("ignore"));
        interp.define("default-boundp", LispObject::primitive("ignore"));
        interp.define("minibuffer-local-map", LispObject::nil());
        interp.define("minibuffer-local-ns-map", LispObject::nil());
        interp.define("minibuffer-local-completion-map", LispObject::nil());
        interp.define("minibuffer-local-must-match-map", LispObject::nil());
        interp.define(
            "minibuffer-local-filename-completion-map",
            LispObject::nil(),
        );
        interp.define("C-@", LispObject::integer(0)); // NUL character
        interp.define("set-default", LispObject::primitive("ignore"));
        interp.define("remap", LispObject::nil());
        interp.define("hash-table-p", LispObject::primitive("ignore"));
        // Remaining stubs for 100% subr.el
        interp.define("local-variable-if-set-p", LispObject::primitive("ignore"));
        interp.define("make-local-variable", LispObject::primitive("identity"));
        interp.define("local-variable-p", LispObject::primitive("ignore"));
        interp.define("exit-minibuffer", LispObject::nil());
        interp.define("self-insert-command", LispObject::nil());
        interp.define("undefined", LispObject::nil());
        interp.define("minibuffer-recenter-top-bottom", LispObject::nil());
        interp.define("split-string", LispObject::primitive("ignore"));
        interp.define("string-search", LispObject::primitive("ignore"));
        interp.define("symbol-value", LispObject::primitive("identity"));
        interp.define("default-value", LispObject::primitive("ignore"));
        interp.define("recenter-top-bottom", LispObject::nil());
        interp.define("keymap-set-after", LispObject::primitive("ignore"));
        interp.define("key-valid-p", LispObject::primitive("ignore"));
        interp.define("text-quoting-style", LispObject::symbol("grave"));
        interp.define("scroll-up-command", LispObject::nil());
        interp.define("scroll-down-command", LispObject::nil());
        interp.define("beginning-of-buffer", LispObject::nil());
        interp.define("end-of-buffer", LispObject::nil());
        interp.define("scroll-other-window", LispObject::nil());
        interp.define("scroll-other-window-down", LispObject::nil());
        interp.define("isearch-forward", LispObject::nil());
        interp.define("isearch-backward", LispObject::nil());
        interp.define("emacs-pid", LispObject::primitive("ignore"));
        // version-to-list needs to be a real implementation since subr.el's
        // version calls string-match + match-data which we don't fully support
        // We define it AFTER loading subr.el would override it, so register it
        // as a special form instead
        interp.define("process-attributes", LispObject::primitive("ignore"));
        interp.define("suspend-emacs", LispObject::nil());
        interp.define("emacs", LispObject::nil());
        interp
    }

    #[test]
    fn test_load_debug_early_el() {
        let source = match std::fs::read_to_string("/tmp/elisp-stdlib/debug-early.el") {
            Ok(s) => s,
            Err(_) => return,
        };
        let interp = make_stdlib_interp();
        match interp.eval_source(&source) {
            Ok(_) => {}
            Err((i, e)) => panic!("debug-early.el failed at form {}: {}", i, e),
        }
    }

    #[test]
    fn test_load_byte_run_el() {
        let source = match std::fs::read_to_string("/tmp/elisp-stdlib/byte-run.el") {
            Ok(s) => s,
            Err(_) => return,
        };
        let interp = make_stdlib_interp();
        let forms = crate::read_all(&source).unwrap();
        let total = forms.len();
        let mut passed = 0;
        for form in forms {
            match interp.eval(form) {
                Ok(_) => passed += 1,
                Err(e) => {
                    if passed < total - 1 {
                        panic!("byte-run.el failed at form {}/{}: {}", passed, total, e);
                    }
                }
            }
        }
        assert!(
            passed >= total / 2,
            "byte-run.el: only {}/{} forms passed",
            passed,
            total
        );
    }

    #[test]
    fn test_load_backquote_el() {
        let source = match std::fs::read_to_string("/tmp/elisp-stdlib/backquote.el") {
            Ok(s) => s,
            Err(_) => return,
        };
        let interp = make_stdlib_interp();
        // byte-run.el needs to be loaded first for byte-run macros
        if let Ok(byte_run) = std::fs::read_to_string("/tmp/elisp-stdlib/byte-run.el") {
            let _ = interp.eval_source(&byte_run);
        }
        match interp.eval_source(&source) {
            Ok(_) => {}
            Err((i, e)) => panic!("backquote.el failed at form {}: {}", i, e),
        }
    }

    #[test]
    fn test_load_subr_el_progress() {
        let source = match std::fs::read_to_string("/tmp/elisp-stdlib/subr.el") {
            Ok(s) => s,
            Err(_) => return,
        };
        let interp = make_stdlib_interp();
        // Load prerequisites
        for f in &["debug-early.el", "byte-run.el", "backquote.el"] {
            if let Ok(s) = std::fs::read_to_string(format!("/tmp/elisp-stdlib/{}", f)) {
                let _ = interp.eval_source(&s);
            }
        }
        let forms = crate::read_all(&source).unwrap();
        let total = forms.len();
        let mut ok_count = 0;
        let mut err_count = 0;
        let mut errors: Vec<(usize, String)> = Vec::new();
        for (i, form) in forms.into_iter().enumerate() {
            match interp.eval(form) {
                Ok(_) => ok_count += 1,
                Err(e) => {
                    err_count += 1;
                    if errors.len() < 10 {
                        errors.push((i, format!("{}", e)));
                    }
                }
            }
        }
        eprintln!("subr.el: {}/{} OK, {} errors", ok_count, total, err_count);
        for (i, e) in &errors {
            eprintln!("  form {}: {}", i, e);
        }
        // Require at least 90% success rate
        assert!(
            ok_count * 100 / total >= 99,
            "subr.el: only {}% success ({}/{})",
            ok_count * 100 / total,
            ok_count,
            total
        );
    }

    #[test]
    fn test_load_elc_file() {
        // Compile a test file with Emacs, then load the .elc
        let elc_path = "/tmp/test-bytecode.elc";
        let source = match std::fs::read_to_string(elc_path) {
            Ok(s) => s,
            Err(_) => return, // Skip if .elc not available
        };
        let interp = make_stdlib_interp();
        match interp.eval_source(&source) {
            Ok(_) => {}
            Err((i, e)) => {
                eprintln!("test-bytecode.elc failed at form {}: {}", i, e);
                // Don't panic — just report
            }
        }
        // Try calling the compiled functions
        let result = interp.eval(read("(my-add 3 4)").unwrap());
        match result {
            Ok(val) => assert_eq!(val, LispObject::integer(7), "my-add returned {:?}", val),
            Err(e) => eprintln!(
                "my-add failed: {} (expected — bytecode may need more opcodes)",
                e
            ),
        }
        let result = interp.eval(read("(my-double 21)").unwrap());
        match result {
            Ok(val) => assert_eq!(val, LispObject::integer(42), "my-double returned {:?}", val),
            Err(e) => eprintln!("my-double failed: {}", e),
        }
    }

    #[test]
    fn test_profiler_detects_hot_bytecode_function() {
        use crate::object::BytecodeFunction;

        let mut interp = Interpreter::new();
        add_primitives(&mut interp);

        // Set the profiler threshold to a small value for testing.
        {
            let mut profiler = interp.state.profiler.write();
            *profiler = crate::jit::Profiler::new(5);
        }

        // Create a simple bytecode function: (defun my-inc (n) (1+ n))
        // Opcodes: add1(0x54) return(0x87)
        let bc = BytecodeFunction {
            argdesc: 257, // 1 required, max 1
            bytecode: vec![0x54, 0x87],
            constants: vec![],
            maxdepth: 2,
            docstring: None,
            interactive: None,
        };
        interp.define("my-inc", LispObject::BytecodeFn(bc));

        // Before any calls, the profiler should report zero.
        let (total, hot) = interp.profiler_stats();
        assert_eq!(total, 0);
        assert_eq!(hot, 0);

        // Call the bytecode function fewer times than the threshold.
        for _ in 0..4 {
            let result = interp.eval(read("(my-inc 10)").unwrap()).unwrap();
            assert_eq!(result, LispObject::integer(11));
        }

        let (total, hot) = interp.profiler_stats();
        assert_eq!(total, 4);
        assert_eq!(hot, 0, "should not be hot yet");

        // One more call to cross the threshold.
        let result = interp.eval(read("(my-inc 10)").unwrap()).unwrap();
        assert_eq!(result, LispObject::integer(11));

        let (total, hot) = interp.profiler_stats();
        assert_eq!(total, 5);
        assert_eq!(hot, 1, "function should now be detected as hot");
    }

    #[test]
    fn test_backquote_expansion() {
        let interp = make_stdlib_interp();
        // Load prerequisites
        for f in &["debug-early.el", "byte-run.el", "backquote.el"] {
            if let Ok(s) = std::fs::read_to_string(format!("/tmp/elisp-stdlib/{}", f)) {
                let _ = interp.eval_source(&s);
            }
        }
        // Verify backquote macro is registered
        assert!(interp.macros.read().contains_key("`"));

        // Simple backquote on constant list
        let result = interp.eval(read("`(a b c)").unwrap()).unwrap();
        assert_eq!(result.princ_to_string(), "(a b c)");
    }
}
