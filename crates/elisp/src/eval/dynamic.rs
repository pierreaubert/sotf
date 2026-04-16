// Dynamic binding and special variable handling: unwind-specpdl, bind_param_dynamic.

use crate::obarray;
use crate::object::LispObject;
use parking_lot::RwLock;
use std::sync::Arc;

use super::{Environment, InterpreterState};

pub(super) fn unwind_specpdl(state: &InterpreterState, depth: usize) {
    let global = &state.global_env;
    let mut specpdl = state.specpdl.write();
    while specpdl.len() > depth {
        if let Some((id, Some(val))) = specpdl.pop() {
            global.write().set_id(id, val);
        }
    }
}
pub(super) fn bind_param_dynamic(
    name: &str,
    value: LispObject,
    new_env: &Arc<RwLock<Environment>>,
    state: &InterpreterState,
) {
    let id = obarray::intern(name);
    if state.special_vars.read().contains(&id) {
        let global = &state.global_env;
        let old = global.read().get_id(id);
        state.specpdl.write().push((id, old));
        global.write().set_id(id, value);
    } else {
        new_env.write().define_id(id, value);
    }
}
