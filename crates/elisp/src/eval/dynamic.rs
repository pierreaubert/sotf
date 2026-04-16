// Dynamic binding and special variable handling: unwind-specpdl, bind_param_dynamic.

use crate::object::LispObject;
use parking_lot::RwLock;
use std::sync::Arc;

use super::{Environment, InterpreterState};

pub(super) fn unwind_specpdl(state: &InterpreterState, depth: usize) {
    let global = &state.global_env;
    let mut specpdl = state.specpdl.write();
    while specpdl.len() > depth {
        if let Some((name, Some(val))) = specpdl.pop() {
            global.write().set(&name, val);
        }
    }
}
pub(super) fn bind_param_dynamic(
    name: &str,
    value: LispObject,
    new_env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) {
    if state.special_vars.read().contains(name) {
        let global = &state.global_env;
        let old = global.read().get(name);
        state.specpdl.write().push((name.to_string(), old));
        global.write().set(name, value);
    } else {
        new_env.write().define(name, value);
    }
}
