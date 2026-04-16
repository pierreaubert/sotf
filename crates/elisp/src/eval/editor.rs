// Editor callbacks: buffer-string, insert, point, goto-char, find-file, etc.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::{eval, Environment, InterpreterState, MacroTable};

pub(super) fn eval_buffer_string(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<LispObject> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(LispObject::string(&cb.buffer_string())),
        None => Ok(LispObject::string("")),
    }
}
pub(super) fn eval_buffer_size(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<LispObject> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(LispObject::integer(cb.buffer_size() as i64)),
        None => Ok(LispObject::integer(0)),
    }
}
pub(super) fn eval_insert(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let text_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let text = eval(text_arg, env, editor, macros, state)?;
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
    Ok(LispObject::nil())
}
pub(super) fn eval_point(
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
) -> ElispResult<LispObject> {
    let e = editor.read();
    match e.as_ref() {
        Some(cb) => Ok(LispObject::integer(cb.point() as i64)),
        None => Ok(LispObject::integer(0)),
    }
}
pub(super) fn eval_goto_char(
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
pub(super) fn eval_delete_char(
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
pub(super) fn eval_forward_char(
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
pub(super) fn eval_find_file(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let path_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let path = eval(path_arg, env, editor, macros, state)?;
    let path_str = match &path {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
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
pub(super) fn eval_save_buffer(
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
