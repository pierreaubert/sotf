// Editor callbacks: buffer-string, insert, point, goto-char, find-file, etc.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

pub(super) fn eval_buffer_string(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::string(&cb.buffer_string()))),
        None => Ok(obj_to_value(LispObject::string(""))),
    }
}
pub(super) fn eval_buffer_size(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.buffer_size() as i64))),
        None => Ok(obj_to_value(LispObject::integer(0))),
    }
}
pub(super) fn eval_insert(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let text_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let text = value_to_obj(eval(obj_to_value(text_arg), env, editor, macros, state)?);
    let text_str = match &text {
        LispObject::String(s) => s.clone(),
        LispObject::Integer(i) => i.to_string(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => format!("{:?}", text),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.insert(&text_str);
    }
    Ok(Value::nil())
}
pub(super) fn eval_point(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(obj_to_value(LispObject::integer(cb.point() as i64))),
        None => Ok(obj_to_value(LispObject::integer(0))),
    }
}
pub(super) fn eval_goto_char(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let pos_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let pos = value_to_obj(eval(obj_to_value(pos_arg), env, editor, macros, state)?);
    let pos = match pos {
        LispObject::Integer(i) => i as usize,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.goto_char(pos);
    }
    Ok(Value::nil())
}
pub(super) fn eval_delete_char(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let n_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = value_to_obj(eval(obj_to_value(n_arg), env, editor, macros, state)?);
    let n = match n {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.delete_char(n);
    }
    Ok(Value::nil())
}
pub(super) fn eval_forward_char(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let n_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = value_to_obj(eval(obj_to_value(n_arg), env, editor, macros, state)?);
    let n = match n {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        cb.forward_char(n);
    }
    Ok(Value::nil())
}
pub(super) fn eval_find_file(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let path_arg = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let path = value_to_obj(eval(obj_to_value(path_arg), env, editor, macros, state)?);
    let path_str = match &path {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        let success = cb.find_file(&path_str);
        Ok(if success { Value::t() } else { Value::nil() })
    } else {
        Ok(Value::nil())
    }
}
pub(super) fn eval_save_buffer(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<Value> {
    let mut e = editor.write();
    if let Some(cb) = e.as_mut() {
        let success = cb.save_buffer();
        Ok(if success { Value::t() } else { Value::nil() })
    } else {
        Ok(Value::nil())
    }
}

/// (save-excursion BODY...)
/// Save point position, evaluate BODY, restore point. Returns result of last body form.
pub(super) fn eval_save_excursion(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let saved_point = {
        let e = editor.read();
        e.as_ref().map(|cb| cb.point()).unwrap_or(0)
    };
    let result = eval_progn(body, env, editor, macros, state);
    // Restore point even if body signaled an error
    {
        let mut e = editor.write();
        if let Some(cb) = e.as_mut() {
            cb.goto_char(saved_point);
        }
    }
    result
}

/// (save-current-buffer BODY...)
/// Save the current buffer, evaluate BODY, restore it. Since we only have a single buffer
/// right now this is equivalent to progn, but the structure is here for future expansion.
pub(super) fn eval_save_current_buffer(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // With a single-buffer model, save-current-buffer is just progn.
    eval_progn(body, env, editor, macros, state)
}
