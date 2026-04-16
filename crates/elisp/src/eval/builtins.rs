// Builtin functions: put, get, provide, featurep, require, mapcar, mapc, dolist, format.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::functions::call_function;
use super::{eval, eval_progn, Environment, InterpreterState, MacroTable};

pub(super) fn eval_put(
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
pub(super) fn eval_get(
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
pub(super) fn eval_provide(
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
pub(super) fn eval_featurep(
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
pub(super) fn eval_require(
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
pub(super) fn eval_mapcar(
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
pub(super) fn eval_mapc(
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
pub(super) fn eval_dolist(
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
pub(super) fn eval_format(
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
