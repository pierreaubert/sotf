// Function calling and application: funcall, apply, lambda application.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::{bind_param_dynamic, unwind_specpdl};
use super::{eval, eval_progn, Environment, InterpreterState, Macro, MacroTable};

/// Phase 7a: **stateful primitives** — functions that are traditionally
/// implemented as special forms in the source-level dispatch (because
/// they need env/macros/state access) but are *semantically* regular
/// functions with evaluated arguments. When a bytecode function from
/// the VM (or any other caller that resolves through the symbol's
/// function cell) invokes `defalias`, `fset`, `eval`, `funcall`,
/// `apply`, etc., we route through this table instead of the regular
/// stateless `primitives::call_primitive` — the caller has already
/// evaluated the arguments, so we must not re-evaluate.
///
/// Returns `Some(result)` when `name` matches; `None` when the caller
/// should fall through to the regular primitive dispatch.
pub(crate) fn call_stateful_primitive(
    name: &str,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> Option<ElispResult<LispObject>> {
    match name {
        "defalias" => Some(stateful_defalias(args, macros)),
        "fset" => Some(stateful_fset(args)),
        "eval" => Some(stateful_eval(args, env, editor, macros, state)),
        "funcall" => Some(stateful_funcall(args, env, editor, macros, state)),
        "apply" => Some(stateful_apply(args, env, editor, macros, state)),
        "put" => Some(stateful_put(args)),
        "get" => Some(stateful_get(args)),
        _ => None,
    }
}

/// `(defalias SYMBOL DEFINITION)` — set SYMBOL's function definition.
/// When DEFINITION is `(macro lambda ARGS . BODY)`, register as a
/// macro in the macros table; otherwise write the obarray function
/// cell. Args are pre-evaluated.
fn stateful_defalias(args: &LispObject, macros: &MacroTable) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let name_str = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // Macro case: DEFINITION = (macro lambda ARGS . BODY)
    if let Some((car, rest)) = value.destructure_cons() {
        if car.as_symbol().as_deref() == Some("macro") {
            if let Some((lambda_sym, lambda_rest)) = rest.destructure_cons() {
                if lambda_sym.as_symbol().as_deref() == Some("lambda") {
                    let macro_args = lambda_rest.first().unwrap_or(LispObject::nil());
                    let macro_body = lambda_rest.rest().unwrap_or(LispObject::nil());
                    macros.write().insert(
                        name_str.clone(),
                        Macro {
                            args: macro_args,
                            body: macro_body,
                        },
                    );
                    return Ok(LispObject::symbol(&name_str));
                }
            }
        }
    }

    // Function case: write the obarray function cell directly.
    let id = crate::obarray::intern(&name_str);
    crate::obarray::set_function_cell(id, value);
    Ok(LispObject::symbol(&name_str))
}

/// `(fset SYMBOL DEFINITION)` — set SYMBOL's function cell. Like
/// `defalias` but doesn't check for the macro form. Args pre-evaluated.
fn stateful_fset(args: &LispObject) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let def = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = name
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    crate::obarray::set_function_cell(sym_id, def.clone());
    Ok(def)
}

/// `(eval FORM)` — evaluate FORM. Args pre-evaluated means FORM is the
/// ACTUAL form to evaluate (not wrapped in another eval).
fn stateful_eval(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = eval(obj_to_value(form), env, editor, macros, state)?;
    Ok(value_to_obj(result))
}

/// `(funcall FUNC &rest ARGS)` — call FUNC with ARGS. All args
/// pre-evaluated.
fn stateful_funcall(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args.rest().unwrap_or(LispObject::nil());
    let result = call_function(
        obj_to_value(func),
        obj_to_value(rest),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(value_to_obj(result))
}

/// `(put SYMBOL PROPERTY VALUE)` — set SYMBOL's plist PROPERTY to
/// VALUE. All args pre-evaluated.
fn stateful_put(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    crate::obarray::put_plist(sym_id, prop_id, val.clone());
    Ok(val)
}

/// `(get SYMBOL PROPERTY)` — get SYMBOL's plist PROPERTY. Args
/// pre-evaluated.
fn stateful_get(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(crate::obarray::get_plist(sym_id, prop_id))
}

/// `(apply FUNC &rest ARGS)` — last ARG is a list; splice its contents
/// as trailing arguments. All args pre-evaluated.
fn stateful_apply(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;

    // Walk args[1..], collecting into `collected`; the final element
    // stays in `last` until we know whether more arguments follow.
    // When the loop exits, `last` is the list to splice.
    let mut collected: Vec<LispObject> = Vec::new();
    let mut last: Option<LispObject> = None;
    let mut current = args.rest().unwrap_or(LispObject::nil());
    while let Some((car, cdr)) = current.destructure_cons() {
        if let Some(prev) = last.take() {
            collected.push(prev);
        }
        last = Some(car);
        current = cdr;
    }
    if let Some(list) = last {
        let mut cur = list;
        while let Some((car, cdr)) = cur.destructure_cons() {
            collected.push(car);
            cur = cdr;
        }
    }

    // Rebuild a proper Lisp argument list (nil-terminated cons chain).
    let mut arg_list = LispObject::nil();
    for item in collected.into_iter().rev() {
        arg_list = LispObject::cons(item, arg_list);
    }

    let result = call_function(
        obj_to_value(func),
        obj_to_value(arg_list),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(value_to_obj(result))
}

pub(super) fn eval_funcall(
    func: Value,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func = resolve_function(func, env, editor, macros, state)?;
    let args = eval_list(args, env, editor, macros, state)?;

    call_function(func, args, env, editor, macros, state)
}
pub(super) fn resolve_function(
    func: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func_obj = value_to_obj(func);
    if let LispObject::Symbol(id) = func_obj {
        let name = crate::obarray::symbol_name(id);
        if let Some(val) = env.read().get_function(&name) {
            return Ok(obj_to_value(val));
        }
        // Check autoloads — load the file and retry
        if let Some(file) = state.autoloads.read().get(&name).cloned() {
            let load_args = LispObject::cons(
                LispObject::string(&file),
                LispObject::cons(LispObject::t(), LispObject::nil()),
            );
            let _ = super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state);
            if let Some(val) = env.read().get_function(&name) {
                return Ok(obj_to_value(val));
            }
        }
        Err(ElispError::VoidFunction(name))
    } else {
        eval(func, env, editor, macros, state)
    }
}
pub(super) fn eval_apply(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(obj_to_value(func), env, editor, macros, state)?;

    let mut raw_args: Vec<LispObject> = Vec::new();
    let mut cur = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    while let Some((car, rest)) = cur.destructure_cons() {
        raw_args.push(car);
        cur = rest;
    }
    if raw_args.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }

    let mut evaled: Vec<LispObject> = Vec::new();
    for a in &raw_args {
        evaled.push(value_to_obj(eval(
            obj_to_value(a.clone()),
            env,
            editor,
            macros,
            state,
        )?));
    }

    let last = evaled.pop().unwrap();
    let mut combined: Vec<LispObject> = evaled;

    let mut tail = last;
    while let Some((car, rest)) = tail.destructure_cons() {
        combined.push(car);
        tail = rest;
    }

    let mut all_args = LispObject::nil();
    for arg in combined.iter().rev() {
        all_args = LispObject::cons(arg.clone(), all_args);
    }

    call_function(func_val, obj_to_value(all_args), env, editor, macros, state)
}
pub(super) fn eval_funcall_form(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest_args = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(obj_to_value(func), env, editor, macros, state)?;
    let evaled_args = eval_list(obj_to_value(rest_args), env, editor, macros, state)?;

    call_function(func_val, evaled_args, env, editor, macros, state)
}
pub(super) fn eval_list(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    match args_obj {
        LispObject::Nil => Ok(Value::nil()),
        LispObject::Cons(_) => {
            let (car, cdr) = args_obj.destructure();
            let car_eval = eval(obj_to_value(car), env, editor, macros, state)?;
            let cdr_eval = eval_list(obj_to_value(cdr), env, editor, macros, state)?;
            Ok(obj_to_value(LispObject::cons(
                value_to_obj(car_eval),
                value_to_obj(cdr_eval),
            )))
        }
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}
pub(super) fn apply_lambda(
    params: &LispObject,
    body: &LispObject,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let specpdl_depth = state.specpdl.read().len();

    let mut params_list = params.clone();
    let mut args_list = value_to_obj(args);
    let mut optional = false;
    let mut rest = false;

    loop {
        if params_list.is_nil() {
            break;
        }
        let (param, params_rest) = match params_list.destructure_cons() {
            Some((p, r)) => (p, r),
            None => {
                if let Some(name) = params_list.as_symbol() {
                    bind_param_dynamic(&name, args_list, &new_env, state);
                }
                break;
            }
        };

        if let Some(name) = param.as_symbol() {
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
                bind_param_dynamic(&name, args_list.clone(), &new_env, state);
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
                        unwind_specpdl(state, specpdl_depth);
                        return Err(ElispError::WrongNumberOfArguments);
                    }
                }
            };
            bind_param_dynamic(&name, arg, &new_env, state);
            args_list = args_rest;
        }
        params_list = params_rest;
    }

    let result = eval_progn(obj_to_value(body.clone()), &new_env, editor, macros, state);
    unwind_specpdl(state, specpdl_depth);
    result
}
pub fn call_function(
    func: Value,
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let func_obj = value_to_obj(func);
    match func_obj {
        LispObject::Cons(ref cell) => {
            let (car_val, cdr_val) = {
                let b = cell.lock();
                (b.0.clone(), b.1.clone())
            };
            if let Some(s) = car_val.as_symbol() {
                if s == "lambda" {
                    let params = cdr_val.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr_val.rest().ok_or(ElispError::WrongNumberOfArguments)?;
                    return apply_lambda(&params, &body, args, env, editor, macros, state);
                }
            }
            Err(ElispError::WrongTypeArgument("function".to_string()))
        }
        LispObject::Primitive(ref name) => {
            let args_obj = value_to_obj(args);
            // Phase 7a: try state-aware primitives first (defalias,
            // fset, eval, funcall, apply…). These are registered as
            // `LispObject::Primitive` in the function cell so they're
            // reachable from the VM's function-call path, not just
            // the source-level special-form dispatch.
            if let Some(result) =
                call_stateful_primitive(name, &args_obj, env, editor, macros, state)
            {
                return Ok(obj_to_value(result?));
            }
            let result = crate::primitives::call_primitive(name, &args_obj)?;
            Ok(obj_to_value(result))
        }
        LispObject::BytecodeFn(ref bc) => {
            let func_id = bc as *const _ as usize;
            #[allow(unused_variables)]
            let should_jit = state.profiler.write().record_call(func_id);

            let args_obj = value_to_obj(args);
            let mut arg_vec = Vec::new();
            let mut current = args_obj;
            while let Some((car, cdr)) = current.destructure_cons() {
                arg_vec.push(car);
                current = cdr;
            }

            #[cfg(feature = "jit")]
            {
                let mut jit = state.jit.write();
                if should_jit && !jit.is_compiled(func_id) {
                    jit.compile(func_id, bc);
                }
                if let Some(id) = jit.get_compiled(func_id) {
                    {
                        let jit_args: Vec<i64> = arg_vec
                            .iter()
                            .map(|a| crate::value::Value::from_lisp_object(a).raw() as i64)
                            .collect();
                        if let Some(native_result) = jit.call(id, &jit_args) {
                            match native_result {
                                crate::jit::NativeResult::Ok(raw) => {
                                    let val = crate::value::Value::from_raw(raw);
                                    return Ok(val);
                                }
                                crate::jit::NativeResult::Deoptimize => {
                                    // Fall through to VM
                                }
                            }
                        }
                    }
                }
            }

            let result = crate::vm::execute_bytecode(bc, &arg_vec, env, editor, macros, state)?;
            Ok(obj_to_value(result))
        }
        LispObject::Symbol(id) => {
            let name = crate::obarray::symbol_name(id);
            // Function-position lookup: env chain for callable, then
            // the symbol's function cell.
            if let Some(val) = env.read().get_function(&name) {
                return call_function(obj_to_value(val), args, env, editor, macros, state);
            }
            // Check autoloads — load the file and retry
            if let Some(file) = state.autoloads.read().get(&name).cloned() {
                let load_args = LispObject::cons(
                    LispObject::string(&file),
                    LispObject::cons(LispObject::t(), LispObject::nil()),
                );
                let _ =
                    super::builtins::eval_load(obj_to_value(load_args), env, editor, macros, state);
                if let Some(val) = env.read().get_function(&name) {
                    return call_function(obj_to_value(val), args, env, editor, macros, state);
                }
            }
            Err(ElispError::VoidFunction(name))
        }
        _ => Err(ElispError::WrongTypeArgument("function".to_string())),
    }
}
