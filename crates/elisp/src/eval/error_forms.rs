// Error and exception handling forms: catch, throw, condition-case, signal, unwind-protect.

use crate::error::{ElispError, ElispResult, SignalData, ThrowData};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::builtins::eval_format;
use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

pub(super) fn eval_catch(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let tag_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let tag = value_to_obj(eval(obj_to_value(tag_expr), env, editor, macros, state)?);

    match eval_progn(obj_to_value(body), env, editor, macros, state) {
        Ok(value) => Ok(value),
        Err(ElispError::Throw(throw_data)) => {
            if tag == throw_data.tag {
                Ok(obj_to_value(throw_data.value))
            } else {
                Err(ElispError::Throw(throw_data))
            }
        }
        Err(e) => Err(e),
    }
}
pub(super) fn eval_throw(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let tag_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let tag = value_to_obj(eval(obj_to_value(tag_expr), env, editor, macros, state)?);
    let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);

    Err(ElispError::Throw(Box::new(ThrowData { tag, value })))
}
pub(super) fn eval_condition_case(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let var = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let bodyform = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args_obj
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());

    match eval(obj_to_value(bodyform), env, editor, macros, state) {
        Ok(value) => Ok(value),
        Err(ref err @ ElispError::Throw(..)) => Err(err.clone()),
        Err(err) => {
            let mut handlers = rest;
            while let Some((handler, more)) = handlers.destructure_cons() {
                let condition = handler.first().unwrap_or(LispObject::nil());
                let handler_body = handler.rest().unwrap_or(LispObject::nil());

                if err.matches_condition(&condition) {
                    let parent_env = Arc::new(env.read().clone());
                    let handler_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

                    if !var.is_nil() {
                        if let Some(var_name) = var.as_symbol() {
                            let signal = err.to_signal();
                            let err_value = if let ElispError::Signal(sig) = signal {
                                LispObject::cons(sig.symbol, sig.data)
                            } else {
                                LispObject::nil()
                            };
                            handler_env.write().define(&var_name, err_value);
                        }
                    }

                    return eval_progn(
                        obj_to_value(handler_body),
                        &handler_env,
                        editor,
                        macros,
                        state,
                    );
                }
                handlers = more;
            }
            Err(err)
        }
    }
}
pub(super) fn eval_signal(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let symbol_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let symbol = value_to_obj(eval(obj_to_value(symbol_expr), env, editor, macros, state)?);
    let data = value_to_obj(eval(obj_to_value(data_expr), env, editor, macros, state)?);

    Err(ElispError::Signal(Box::new(SignalData { symbol, data })))
}
pub(super) fn eval_error_fn(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let formatted = value_to_obj(eval_format(args, env, editor, macros, state)?);
    let msg_str = formatted.princ_to_string();

    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg_str), LispObject::nil()),
    })))
}
pub(super) fn eval_user_error_fn(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let formatted = value_to_obj(eval_format(args, env, editor, macros, state)?);
    let msg_str = formatted.princ_to_string();

    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("user-error"),
        data: LispObject::cons(LispObject::string(&msg_str), LispObject::nil()),
    })))
}
pub(super) fn eval_unwind_protect(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bodyform = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cleanup_forms = args_obj.rest().unwrap_or(LispObject::nil());

    let body_result = eval(obj_to_value(bodyform), env, editor, macros, state);

    let _ = eval_progn(obj_to_value(cleanup_forms), env, editor, macros, state);

    body_result
}
