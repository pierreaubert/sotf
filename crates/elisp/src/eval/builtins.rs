// Builtin functions: put, get, provide, featurep, require, mapcar, mapc, dolist, format.

use crate::error::{ElispError, ElispResult};
use crate::obarray;
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

    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    obarray::put_plist(sym_id, prop_id, val.clone());
    let _ = state; // state no longer needed for plist ops
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

    let sym_id = sym
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let prop_id = prop
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let _ = state;
    Ok(obj_to_value(obarray::get_plist(sym_id, prop_id)))
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

    // Already provided — nothing to do
    if state.features.read().contains(&name) {
        return Ok(obj_to_value(feature));
    }

    // Determine the file to load: use explicit filename arg if given, else the feature name
    let file = args_obj
        .nth(1)
        .and_then(|f| {
            let val = value_to_obj(eval(obj_to_value(f), env, editor, macros, state).ok()?);
            val.as_string().map(|s| s.to_string())
        })
        .unwrap_or_else(|| name.clone());

    // Build (load FILE nil) — noerror=nil so missing files signal
    let load_args = LispObject::cons(
        LispObject::string(&file),
        LispObject::cons(LispObject::nil(), LispObject::nil()),
    );
    eval_load(obj_to_value(load_args), env, editor, macros, state)?;

    // Return the feature symbol regardless of whether `provide` was called.
    // Some files don't call provide — that's OK for our purposes.
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

/// (load FILE &optional NOERROR NOMESSAGE NOSUFFIX MUST-SUFFIX)
/// Search `load-path` for FILE with .elc / .el suffixes, read and eval all forms.
pub(super) fn eval_load(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let file_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let file = value_to_obj(eval(obj_to_value(file_expr), env, editor, macros, state)?);
    let file_str = file
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();

    // 4th arg (nosuffix): when non-nil, don't add .el / .elc suffixes
    let nosuffix = args_obj.nth(3).map(|v| !v.is_nil()).unwrap_or(false);

    let suffixes: &[&str] = if nosuffix {
        &["", ".gz"]
    } else {
        &[".elc", ".el", ".el.gz", ""]
    };

    // Gather load-path directories
    let load_path = state
        .global_env
        .read()
        .get("load-path")
        .unwrap_or(LispObject::nil());
    let mut load_dirs = Vec::new();
    let mut cur = load_path;
    while let Some((dir, rest)) = cur.destructure_cons() {
        if let Some(d) = dir.as_string() {
            load_dirs.push(d.clone());
        }
        cur = rest;
    }

    // Build candidate paths
    let mut paths_to_try = Vec::new();
    for suffix in suffixes {
        let full = format!("{}{}", file_str, suffix);
        // Try as absolute/relative path first
        paths_to_try.push(full.clone());
        // Then try in each load-path directory
        for d in &load_dirs {
            paths_to_try.push(format!("{}/{}", d, full));
        }
    }

    for path in &paths_to_try {
        let source = if path.ends_with(".gz") {
            // Decompress gzipped files via gunzip
            if std::path::Path::new(path).exists() {
                std::process::Command::new("gunzip")
                    .args(["-c", path])
                    .output()
                    .ok()
                    .and_then(|out| {
                        if out.status.success() {
                            String::from_utf8(out.stdout).ok()
                        } else {
                            None
                        }
                    })
            } else {
                None
            }
        } else {
            std::fs::read_to_string(path).ok()
        };
        if let Some(source) = source {
            let forms = crate::read_all(&source).map_err(|_| ElispError::FileError {
                operation: "load".into(),
                path: path.clone(),
                message: "read error".into(),
            })?;
            // Phase 7: be tolerant of per-form errors during load.
            // Emacs's own behaviour is to propagate; we diverge because
            // our interpreter is incomplete (missing primitives, some
            // bytecode bugs) and most stdlib files are useful even when
            // a few forms fail. Errors are surfaced via stderr so
            // debugging still works.
            for (i, form) in forms.into_iter().enumerate() {
                if let Err(e) = eval(obj_to_value(form), env, editor, macros, state) {
                    eprintln!("load {path}: form {i}: {e}");
                }
            }
            return Ok(Value::t());
        }
    }

    // 2nd arg (noerror): when non-nil, return nil instead of signaling
    let noerror = args_obj.nth(1).map(|v| !v.is_nil()).unwrap_or(false);
    if noerror {
        Ok(Value::nil())
    } else {
        Err(ElispError::FileError {
            operation: "load".into(),
            path: file_str,
            message: "file not found".into(),
        })
    }
}

/// Evaluate body forms in sequence, returning the result of the last one.
/// Used by save-excursion and save-restriction which wrap progn-style bodies.
pub(super) fn eval_progn_value(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    eval_progn(body, env, editor, macros, state)
}
