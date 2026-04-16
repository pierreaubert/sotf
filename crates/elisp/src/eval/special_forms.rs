// Special form evaluation functions: if, setq, defun, let, progn, cond, etc.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::unwind_specpdl;
use super::{eval, Environment, InterpreterState, Macro, MacroTable};

pub(super) fn eval_if(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let then_branch = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let cond_val = eval(cond, env, editor, macros, state)?;
    if cond_val.is_nil() {
        // (if COND THEN ELSE1 ELSE2 ...) — else forms are implicit progn
        let else_forms = args
            .rest()
            .and_then(|r| r.rest())
            .unwrap_or(LispObject::nil());
        eval_progn(&else_forms, env, editor, macros, state)
    } else {
        eval(then_branch, env, editor, macros, state)
    }
}
pub(super) fn eval_setq(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    // (setq SYM1 VAL1 SYM2 VAL2 ...) — set multiple pairs
    let mut result = LispObject::nil();
    let mut current = args.clone();
    while let Some((name_obj, rest)) = current.destructure_cons() {
        let name = name_obj
            .as_symbol()
            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
        let val_expr = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
        let value = eval(val_expr, env, editor, macros, state)?;
        // Special variables are always set in the global env (current dynamic binding).
        if state.special_vars.read().contains(&name) {
            state.global_env.write().set(&name, value.clone());
        } else {
            env.write().set(&name, value.clone());
        }
        result = value;
        current = rest.rest().unwrap_or(LispObject::nil());
    }
    Ok(result)
}
pub(super) fn eval_defun(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    _editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    _macros: &MacroTable,
    _state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let lambda = LispObject::lambda_expr(
        rest.first().unwrap_or(LispObject::nil()),
        rest.rest().unwrap_or(LispObject::nil()),
    );
    let mut env = env.write();
    env.define(&name, lambda);
    Ok(LispObject::symbol(&name))
}
pub(super) fn eval_defmacro(args: &LispObject, macros: &MacroTable) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let macro_args = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let macro_body = rest.rest().unwrap_or(LispObject::nil());

    let macro_def = Macro {
        args: macro_args,
        body: macro_body,
    };

    macros.write().insert(name.clone(), macro_def);
    Ok(LispObject::symbol(&name))
}
pub(super) fn eval_macroexpand(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;

    if let LispObject::Cons(_) = form {
        let car = form.first().unwrap_or(LispObject::nil());
        if let Some(s) = car.as_symbol() {
            let macro_table = macros.read();
            if let Some(macro_) = macro_table.get(&s) {
                let macro_ = macro_.clone();
                drop(macro_table);
                let cdr = form.rest().unwrap_or(LispObject::nil());
                let expanded = expand_macro(&macro_, cdr, env, editor, macros, state)?;
                return Ok(expanded); // macroexpand returns expanded form without evaluating
            }
        }
    }

    Ok(form)
}
pub(super) fn expand_macro(
    macro_: &Macro,
    args: LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let params = &macro_.args;
    let body = &macro_.body;

    let param_names: Vec<String> = extract_param_names(params)?;

    let mut arg_list = args;
    let mut bindings: Vec<(String, LispObject)> = Vec::new();

    for name in &param_names {
        if name.starts_with("&rest ") || name.starts_with("&optional ") {
            continue;
        }
        if arg_list.is_nil() {
            bindings.push((name.clone(), LispObject::nil()));
        } else {
            let arg = arg_list.first().unwrap_or(LispObject::nil());
            bindings.push((name.clone(), arg));
            arg_list = arg_list.rest().unwrap_or(LispObject::nil());
        }
    }

    let parent_env = Arc::new(env.read().clone());
    let temp_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    for (name, value) in bindings {
        temp_env.write().define(&name, value);
    }

    eval_progn(body, &temp_env, editor, macros, state)
}
pub(super) fn extract_param_names(params: &LispObject) -> ElispResult<Vec<String>> {
    let mut names = Vec::new();
    let mut current = Some(params.clone());

    while let Some(curr) = current {
        if let Some((car, rest)) = curr.destructure_cons() {
            if let Some(s) = car.as_symbol() {
                if s == "&rest" || s == "&optional" {
                    current = Some(rest);
                    continue;
                }
                names.push(s);
                current = Some(rest);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    Ok(names)
}
pub(super) fn eval_let(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let bindings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    // Record specpdl depth so we can unwind special bindings on exit.
    let specpdl_depth = state.specpdl.read().len();

    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        // Support both (VAR VALUE) and bare VAR (binds to nil)
        let (name, value) = if let Some(name) = binding.as_symbol() {
            (name, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_name = binding_name
                .as_symbol()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val = eval(binding_val, env, editor, macros, state)?;
            (binding_name, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };

        if state.special_vars.read().contains(&name) {
            // Dynamic binding: save old value and set in the global env.
            let global = &state.global_env;
            let old = global.read().get(&name);
            state.specpdl.write().push((name.clone(), old));
            global.write().set(&name, value);
        } else {
            // Lexical binding: bind in the new local scope.
            new_env.write().define(&name, value);
        }

        bindings_list = rest;
    }

    let result = eval_progn(&body, &new_env, editor, macros, state);

    // Always unwind special bindings, even if body signaled/threw.
    unwind_specpdl(state, specpdl_depth);

    result
}
pub(super) fn eval_progn(
    body: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut result = LispObject::nil();
    let mut current = Some(body.clone());
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(expr, env, editor, macros, state)?;
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}
pub(super) fn eval_cond(
    clauses: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let mut current = Some(clauses.clone());
    while let Some(curr) = current {
        if let Some((clause, rest)) = curr.destructure_cons() {
            let (cond_expr, then_exprs) = if let Some((c, r)) = clause.destructure_cons() {
                (c, Some(r))
            } else {
                (clause, None)
            };

            let cond_val = eval(cond_expr, env, editor, macros, state)?;
            if !cond_val.is_nil() {
                if let Some(exprs) = then_exprs {
                    return eval_progn(&exprs, env, editor, macros, state);
                }
                return Ok(cond_val);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(LispObject::nil())
}
pub(super) fn eval_loop(
    body: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    loop {
        eval_progn(body, env, editor, macros, state)?;
    }
}
pub(super) fn eval_while(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().unwrap_or(LispObject::nil());
    loop {
        let cond_val = eval(cond.clone(), env, editor, macros, state)?;
        if cond_val.is_nil() {
            return Ok(LispObject::nil());
        }
        eval_progn(&body, env, editor, macros, state)?;
    }
}
pub(super) fn eval_let_star(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let bindings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    // Record specpdl depth so we can unwind special bindings on exit.
    let specpdl_depth = state.specpdl.read().len();

    // let* evaluates each binding in the new env (sequential)
    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        let (name, value) = if let Some(name) = binding.as_symbol() {
            (name, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_name = binding_name
                .as_symbol()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val = eval(binding_val, &new_env, editor, macros, state)?;
            (binding_name, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };

        if state.special_vars.read().contains(&name) {
            // Dynamic binding: save old value and set in the global env.
            let global = &state.global_env;
            let old = global.read().get(&name);
            state.specpdl.write().push((name.clone(), old));
            global.write().set(&name, value);
        } else {
            // Lexical binding: bind in the new local scope.
            new_env.write().define(&name, value);
        }

        bindings_list = rest;
    }

    let result = eval_progn(&body, &new_env, editor, macros, state);

    // Always unwind special bindings, even if body signaled/threw.
    unwind_specpdl(state, specpdl_depth);

    result
}
pub(super) fn eval_when(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(cond, env, editor, macros, state)?;
    if cond_val.is_nil() {
        Ok(LispObject::nil())
    } else {
        eval_progn(&body, env, editor, macros, state)
    }
}
pub(super) fn eval_unless(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let cond = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(cond, env, editor, macros, state)?;
    if cond_val.is_nil() {
        eval_progn(&body, env, editor, macros, state)
    } else {
        Ok(LispObject::nil())
    }
}
pub(super) fn eval_and(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if args.is_nil() {
        return Ok(LispObject::t());
    }
    let mut current = Some(args.clone());
    let mut result = LispObject::t();
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(expr, env, editor, macros, state)?;
            if result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}
pub(super) fn eval_or(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    if args.is_nil() {
        return Ok(LispObject::nil());
    }
    let mut current = Some(args.clone());
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            let result = eval(expr, env, editor, macros, state)?;
            if !result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(LispObject::nil())
}
pub(super) fn eval_prog1(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = eval(first, env, editor, macros, state)?;
    let _ = eval_progn(
        &args.rest().unwrap_or(LispObject::nil()),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(result)
}
pub(super) fn eval_prog2(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let second = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let _ = eval(first, env, editor, macros, state)?;
    let result = eval(second, env, editor, macros, state)?;
    let rest = args
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());
    let _ = eval_progn(&rest, env, editor, macros, state)?;
    Ok(result)
}
pub(super) fn eval_defvar(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // Mark variable as special (dynamically bound).
    state.special_vars.write().insert(name.clone());

    // defvar only sets value if currently void (unbound)
    let is_bound = env.read().get(&name).is_some();
    if !is_bound {
        if let Some(value_expr) = args.nth(1) {
            let value = eval(value_expr, env, editor, macros, state)?;
            env.write().define(&name, value);
        }
    }
    // Ignore docstring (3rd arg) for now
    Ok(LispObject::symbol(&name))
}
pub(super) fn eval_defconst(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // Mark variable as special (dynamically bound).
    state.special_vars.write().insert(name.clone());

    // defconst always sets the value
    if let Some(value_expr) = args.nth(1) {
        let value = eval(value_expr, env, editor, macros, state)?;
        env.write().define(&name, value);
    }
    Ok(LispObject::symbol(&name))
}
pub(super) fn eval_defalias(
    args: &LispObject,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<LispObject> {
    let name = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let definition = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let name = eval(name, env, editor, macros, state)?;
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let value = eval(definition, env, editor, macros, state)?;

    // If the value is (macro lambda ARGS . BODY), register it as a macro.
    // This handles e.g. (defalias '\` (symbol-function 'backquote))
    // where backquote is a defmacro.
    if let Some((car, rest)) = value.destructure_cons() {
        if car.as_symbol().as_deref() == Some("macro") {
            if let Some((lambda_sym, lambda_rest)) = rest.destructure_cons() {
                if lambda_sym.as_symbol().as_deref() == Some("lambda") {
                    let macro_args = lambda_rest.first().unwrap_or(LispObject::nil());
                    let macro_body = lambda_rest.rest().unwrap_or(LispObject::nil());
                    macros.write().insert(
                        name.clone(),
                        Macro {
                            args: macro_args,
                            body: macro_body,
                        },
                    );
                    return Ok(LispObject::symbol(&name));
                }
            }
        }
    }

    env.write().define(&name, value);
    Ok(LispObject::symbol(&name))
}
