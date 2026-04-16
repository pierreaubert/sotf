// Builtin functions: put, get, provide, featurep, require, mapcar, mapc, dolist, format.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::functions::call_function;
use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

pub(super) fn eval_put(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let sym = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let prop = value_to_obj(eval(
        obj_to_value(args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let val = value_to_obj(eval(
        obj_to_value(args_obj.nth(2).ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);

    let sym_name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_name = prop
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let key = format!("{}:{}", sym_name, prop_name);
    state.plists.write().insert(key, val.clone());
    Ok(obj_to_value(val))
}
pub(super) fn eval_get(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let sym = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let prop = value_to_obj(eval(
        obj_to_value(args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);

    let sym_name = sym
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_name = prop
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let key = format!("{}:{}", sym_name, prop_name);
    Ok(obj_to_value(
        state
            .plists
            .read()
            .get(&key)
            .cloned()
            .unwrap_or(LispObject::nil()),
    ))
}
pub(super) fn eval_provide(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let feature = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let mut features = state.features.write();
    if !features.contains(&name) {
        features.push(name);
    }
    Ok(obj_to_value(feature))
}
pub(super) fn eval_featurep(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let feature = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let features = state.features.read();
    Ok(obj_to_value(LispObject::from(features.contains(&name))))
}
pub(super) fn eval_require(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let feature = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let features = state.features.read();
    if features.contains(&name) {
        return Ok(obj_to_value(feature));
    }
    drop(features);
    Ok(obj_to_value(feature))
}
pub(super) fn eval_mapcar(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func = value_to_obj(eval(obj_to_value(func_expr), env, editor, macros, state)?);
    let list = value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);

    let mut results = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        let call_args = LispObject::cons(car, LispObject::nil());
        let result = call_function(
            obj_to_value(func.clone()),
            obj_to_value(call_args),
            env,
            editor,
            macros,
            state,
        )?;
        results.push(value_to_obj(result));
        current = cdr;
    }
    let mut result = LispObject::nil();
    for r in results.into_iter().rev() {
        result = LispObject::cons(r, result);
    }
    Ok(obj_to_value(result))
}
pub(super) fn eval_mapc(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func = value_to_obj(eval(obj_to_value(func_expr), env, editor, macros, state)?);
    let list = value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);

    let mut current = list.clone();
    while let Some((car, cdr)) = current.destructure_cons() {
        let call_args = LispObject::cons(car, LispObject::nil());
        call_function(
            obj_to_value(func.clone()),
            obj_to_value(call_args),
            env,
            editor,
            macros,
            state,
        )?;
        current = cdr;
    }
    Ok(obj_to_value(list))
}
pub(super) fn eval_dolist(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let spec = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let var = spec.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let var_name = var
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let list_expr = spec.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result_expr = spec.nth(2);

    let list = value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);

    let parent_env = Arc::new(env.read().clone());
    let loop_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let body_val = obj_to_value(body);
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        loop_env.write().set(&var_name, car);
        eval_progn(body_val, &loop_env, editor, macros, state)?;
        current = cdr;
    }

    loop_env.write().set(&var_name, LispObject::nil());
    if let Some(result_expr) = result_expr {
        eval(obj_to_value(result_expr), &loop_env, editor, macros, state)
    } else {
        Ok(Value::nil())
    }
}
pub(super) fn emacs_regex_to_rust(emacs: &str) -> String {
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
                }
                '\'' => {
                    result.push_str("\\z");
                    i += 2;
                }
                c => {
                    result.push('\\');
                    result.push(c);
                    i += 2;
                }
            }
        } else {
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
pub(super) fn eval_format(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let fmt_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let fmt = value_to_obj(eval(obj_to_value(fmt_expr), env, editor, macros, state)?);
    let fmt_str = match fmt {
        LispObject::String(s) => s,
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    let mut format_args = Vec::new();
    let mut rest = args_obj.rest().unwrap_or(LispObject::nil());
    while let Some((arg, next)) = rest.destructure_cons() {
        let val = value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
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
            let mut width: usize = 0;
            while i < chars.len() && chars[i].is_ascii_digit() {
                width = width * 10 + (chars[i] as usize - '0' as usize);
                i += 1;
            }
            if i >= chars.len() {
                break;
            }
            if left_align {
                zero_pad = false;
            }
            let apply_width = |s: String| -> String {
                if width == 0 || s.len() >= width {
                    s
                } else if left_align {
                    format!("{:<width$}", s, width = width)
                } else if zero_pad {
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
    Ok(obj_to_value(LispObject::string(&result)))
}
