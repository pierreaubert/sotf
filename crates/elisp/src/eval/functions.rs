// Function calling and application: funcall, apply, lambda application.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::{bind_param_dynamic, unwind_specpdl};
use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

pub(super) fn eval_funcall(
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
pub(super) fn resolve_function(
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
pub(super) fn eval_apply(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (apply FN ARG1 ARG2 ... LAST-LIST)
    // All args except the last are individual values; the last must be a list
    // whose elements are spread. The combined args are passed to FN.
    let func = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let func_val = eval(func, env, editor, macros, state)?;

    // Collect all remaining arg expressions
    let mut raw_args: Vec<LispObject> = Vec::new();
    let mut cur = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    while let Some((car, rest)) = cur.destructure_cons() {
        raw_args.push(car);
        cur = rest;
    }
    if raw_args.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }

    // Evaluate all arg expressions
    let mut evaled: Vec<LispObject> = Vec::new();
    for a in &raw_args {
        evaled.push(eval(a.clone(), env, editor, macros, state)?);
    }

    // All except the last are individual args; the last is a list to spread
    let last = evaled.pop().unwrap(); // safe: checked non-empty above
    let mut combined: Vec<LispObject> = evaled;

    // Spread the last argument (must be a list or nil)
    let mut tail = last;
    while let Some((car, rest)) = tail.destructure_cons() {
        combined.push(car);
        tail = rest;
    }

    // Build the args list
    let mut all_args = LispObject::nil();
    for arg in combined.iter().rev() {
        all_args = LispObject::cons(arg.clone(), all_args);
    }

    call_function(&func_val, &all_args, env, editor, macros, state)
}
pub(super) fn eval_funcall_form(
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
pub(super) fn eval_list(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    match args {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(_) => {
            let (car, cdr) = args.clone().destructure();
            let car_eval = eval(car, env, editor, macros, state)?;
            let cdr_eval = eval_list(&cdr, env, editor, macros, state)?;
            Ok(LispObject::cons(car_eval, cdr_eval))
        }
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}
pub(super) fn apply_lambda(
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

    let specpdl_depth = state.specpdl.read().len();

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
                    bind_param_dynamic(&name, args_list, &new_env, state);
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
                bind_param_dynamic(name, args_list.clone(), &new_env, state);
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
            bind_param_dynamic(name, arg, &new_env, state);
            args_list = args_rest;
        }
        params_list = params_rest;
    }

    let result = eval_progn(body, &new_env, editor, macros, state);
    unwind_specpdl(state, specpdl_depth);
    result
}
pub fn call_function(
    func: &LispObject,
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    match func {
        LispObject::Cons(cell) => {
            let (car_val, cdr_val) = {
                let b = cell.lock();
                (b.0.clone(), b.1.clone())
            };
            if let LispObject::Symbol(s) = &car_val {
                if s == "lambda" {
                    let params = cdr_val.first().ok_or(ElispError::WrongNumberOfArguments)?;
                    let body = cdr_val.rest().ok_or(ElispError::WrongNumberOfArguments)?;
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
