// Error and exception handling forms: catch, throw, condition-case, signal, unwind-protect.

use crate::error::{ElispError, ElispResult, SignalData, ThrowData};
use crate::object::LispObject;
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::builtins::eval_format;
use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

pub(super) fn eval_catch(
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
pub(super) fn eval_throw(
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
pub(super) fn eval_condition_case(
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
                            handler_env.write().define(&var_name, err_value);
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
pub(super) fn eval_signal(
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
pub(super) fn eval_error_fn(
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
pub(super) fn eval_unwind_protect(
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
