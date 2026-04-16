// Function calling and application: funcall, apply, lambda application.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::{bind_param_dynamic, unwind_specpdl};
use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

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
        env.read()
            .get_function(&name)
            .map(obj_to_value)
            .ok_or(ElispError::VoidFunction(name))
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
            let val = env
                .read()
                .get(&name)
                .ok_or(ElispError::VoidFunction(name))?;
            call_function(obj_to_value(val), args, env, editor, macros, state)
        }
        _ => Err(ElispError::WrongTypeArgument("function".to_string())),
    }
}
