// Special form evaluation functions: if, setq, defun, let, progn, cond, etc.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{obj_to_value, value_to_obj, Value};
use crate::EditorCallbacks;
use parking_lot::RwLock;
use std::sync::Arc;

use super::dynamic::unwind_specpdl;
use super::{eval, Environment, InterpreterState, Macro, MacroTable};

pub(super) fn eval_if(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let then_branch = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let cond_val = eval(obj_to_value(cond), env, editor, macros, state)?;
    if cond_val.is_nil() {
        let else_forms = args_obj
            .rest()
            .and_then(|r| r.rest())
            .unwrap_or(LispObject::nil());
        eval_progn(obj_to_value(else_forms), env, editor, macros, state)
    } else {
        eval(obj_to_value(then_branch), env, editor, macros, state)
    }
}
pub(super) fn eval_setq(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let mut result = Value::nil();
    let mut current = args_obj;
    while let Some((name_obj, rest)) = current.destructure_cons() {
        let id = name_obj
            .as_symbol_id()
            .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
        let val_expr = rest.first().ok_or(ElispError::WrongNumberOfArguments)?;
        let value_val = eval(obj_to_value(val_expr), env, editor, macros, state)?;
        let value = value_to_obj(value_val);
        if state.special_vars.read().contains(&id) {
            state.global_env.write().set_id(id, value);
        } else {
            env.write().set_id(id, value);
        }
        result = value_val;
        current = rest.rest().unwrap_or(LispObject::nil());
    }
    Ok(result)
}
pub(super) fn eval_defun(
    args: Value,
    _env: &Arc<RwLock<Environment>>,
    _editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    _macros: &MacroTable,
    _state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name_obj
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    let lambda = LispObject::lambda_expr(
        rest.first().unwrap_or(LispObject::nil()),
        rest.rest().unwrap_or(LispObject::nil()),
    );
    // defun writes the function cell directly.
    crate::obarray::set_function_cell(id, lambda);
    Ok(obj_to_value(LispObject::Symbol(id)))
}
pub(super) fn eval_defmacro(args: Value, macros: &MacroTable) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
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
    Ok(obj_to_value(LispObject::symbol(&name)))
}
pub(super) fn eval_macroexpand(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let form = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;

    if let LispObject::Cons(_) = form {
        let car = form.first().unwrap_or(LispObject::nil());
        if let Some(s) = car.as_symbol() {
            let macro_table = macros.read();
            if let Some(macro_) = macro_table.get(&s) {
                let macro_ = macro_.clone();
                drop(macro_table);
                let cdr_obj = form.rest().unwrap_or(LispObject::nil());
                let expanded = expand_macro(&macro_, cdr_obj, env, editor, macros, state)?;
                return Ok(obj_to_value(expanded));
            }
        }
    }

    Ok(obj_to_value(form))
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

    let result = eval_progn(obj_to_value(body.clone()), &temp_env, editor, macros, state)?;
    Ok(value_to_obj(result))
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
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bindings = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let specpdl_depth = state.specpdl.read().len();

    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        let (id, value) = if let Some(id) = binding.as_symbol_id() {
            (id, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_id = binding_name
                .as_symbol_id()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val =
                value_to_obj(eval(obj_to_value(binding_val), env, editor, macros, state)?);
            (binding_id, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };

        if state.special_vars.read().contains(&id) {
            let global = &state.global_env;
            let old = global.read().get_id(id);
            state.specpdl.write().push((id, old));
            global.write().set_id(id, value);
        } else {
            new_env.write().define_id(id, value);
        }

        bindings_list = rest;
    }

    let result = eval_progn(obj_to_value(body), &new_env, editor, macros, state);

    unwind_specpdl(state, specpdl_depth);

    result
}
pub(super) fn eval_progn(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let body_obj = value_to_obj(body);
    let mut result = Value::nil();
    let mut current = Some(body_obj);
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(obj_to_value(expr), env, editor, macros, state)?;
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(result)
}
pub(super) fn eval_cond(
    clauses: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let clauses_obj = value_to_obj(clauses);
    let mut current = Some(clauses_obj);
    while let Some(curr) = current {
        if let Some((clause, rest)) = curr.destructure_cons() {
            let (cond_expr, then_exprs) = if let Some((c, r)) = clause.destructure_cons() {
                (c, Some(r))
            } else {
                (clause, None)
            };

            let cond_val = eval(obj_to_value(cond_expr), env, editor, macros, state)?;
            if !cond_val.is_nil() {
                if let Some(exprs) = then_exprs {
                    return eval_progn(obj_to_value(exprs), env, editor, macros, state);
                }
                return Ok(cond_val);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(Value::nil())
}
pub(super) fn eval_loop(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    loop {
        eval_progn(body, env, editor, macros, state)?;
    }
}
pub(super) fn eval_while(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());
    let body_val = obj_to_value(body);
    loop {
        let cond_val = eval(obj_to_value(cond.clone()), env, editor, macros, state)?;
        if cond_val.is_nil() {
            return Ok(Value::nil());
        }
        eval_progn(body_val, env, editor, macros, state)?;
    }
}
pub(super) fn eval_let_star(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bindings = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;

    let parent_env = Arc::new(env.read().clone());
    let new_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

    let specpdl_depth = state.specpdl.read().len();

    let mut bindings_list = bindings;
    while let Some((binding, rest)) = bindings_list.destructure_cons() {
        let (id, value) = if let Some(id) = binding.as_symbol_id() {
            (id, LispObject::nil())
        } else if let Some((binding_name, binding_val_wrapper)) = binding.destructure_cons() {
            let binding_id = binding_name
                .as_symbol_id()
                .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
            let binding_val = binding_val_wrapper.first().unwrap_or(LispObject::nil());
            let binding_val = value_to_obj(eval(
                obj_to_value(binding_val),
                &new_env,
                editor,
                macros,
                state,
            )?);
            (binding_id, binding_val)
        } else {
            return Err(ElispError::WrongTypeArgument("symbol or list".to_string()));
        };

        if state.special_vars.read().contains(&id) {
            let global = &state.global_env;
            let old = global.read().get_id(id);
            state.specpdl.write().push((id, old));
            global.write().set_id(id, value);
        } else {
            new_env.write().define_id(id, value);
        }

        bindings_list = rest;
    }

    let result = eval_progn(obj_to_value(body), &new_env, editor, macros, state);

    unwind_specpdl(state, specpdl_depth);

    result
}
pub(super) fn eval_when(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(obj_to_value(cond), env, editor, macros, state)?;
    if cond_val.is_nil() {
        Ok(Value::nil())
    } else {
        eval_progn(obj_to_value(body), env, editor, macros, state)
    }
}
pub(super) fn eval_unless(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let cond = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().ok_or(ElispError::WrongNumberOfArguments)?;
    let cond_val = eval(obj_to_value(cond), env, editor, macros, state)?;
    if cond_val.is_nil() {
        eval_progn(obj_to_value(body), env, editor, macros, state)
    } else {
        Ok(Value::nil())
    }
}
pub(super) fn eval_and(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    if args_obj.is_nil() {
        return Ok(Value::t());
    }
    let mut current = Some(args_obj);
    let mut result = Value::t();
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            result = eval(obj_to_value(expr), env, editor, macros, state)?;
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
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    if args_obj.is_nil() {
        return Ok(Value::nil());
    }
    let mut current = Some(args_obj);
    while let Some(curr) = current {
        if let Some((expr, rest)) = curr.destructure_cons() {
            let result = eval(obj_to_value(expr), env, editor, macros, state)?;
            if !result.is_nil() {
                return Ok(result);
            }
            current = Some(rest);
        } else {
            break;
        }
    }
    Ok(Value::nil())
}
pub(super) fn eval_prog1(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let first = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = eval(obj_to_value(first), env, editor, macros, state)?;
    let _ = eval_progn(
        obj_to_value(args_obj.rest().unwrap_or(LispObject::nil())),
        env,
        editor,
        macros,
        state,
    )?;
    Ok(result)
}
pub(super) fn eval_prog2(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let first = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let second = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let _ = eval(obj_to_value(first), env, editor, macros, state)?;
    let result = eval(obj_to_value(second), env, editor, macros, state)?;
    let rest = args_obj
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());
    let _ = eval_progn(obj_to_value(rest), env, editor, macros, state)?;
    Ok(result)
}
pub(super) fn eval_defvar(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name_obj
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    state.special_vars.write().insert(id);
    crate::obarray::mark_special(id);

    // defvar only initializes when the variable has no existing local
    // binding. We use get_id_local (env-only, no value-cell fallback)
    // because process-global value cells may hold a concurrent test's
    // value — that must NOT block initialization of this interpreter's
    // binding.
    let is_bound = env.read().get_id_local(id).is_some();
    if !is_bound {
        if let Some(value_expr) = args_obj.nth(1) {
            let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
            env.write().define_id(id, value.clone());
            // Mirror to the value cell so fresh envs elsewhere can see it.
            crate::obarray::set_value_cell(id, value);
        }
    }
    Ok(obj_to_value(LispObject::Symbol(id)))
}
pub(super) fn eval_defconst(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_obj = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let id = name_obj
        .as_symbol_id()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    state.special_vars.write().insert(id);
    crate::obarray::mark_special(id);

    if let Some(value_expr) = args_obj.nth(1) {
        let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);
        env.write().define_id(id, value.clone());
        crate::obarray::set_value_cell(id, value);
    }
    Ok(obj_to_value(LispObject::Symbol(id)))
}
pub(super) fn eval_defalias(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let name_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let definition = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let name = value_to_obj(eval(obj_to_value(name_expr), env, editor, macros, state)?);
    let name = name
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let value = value_to_obj(eval(obj_to_value(definition), env, editor, macros, state)?);

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
                    return Ok(obj_to_value(LispObject::symbol(&name)));
                }
            }
        }
    }

    // defalias writes the function cell — value is a function definition.
    let id = crate::obarray::intern(&name);
    crate::obarray::set_function_cell(id, value);
    Ok(obj_to_value(LispObject::Symbol(id)))
}
