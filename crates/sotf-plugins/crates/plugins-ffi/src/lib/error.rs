use super::PluginError;
use std::os::raw::c_int;

impl From<PluginError> for c_int {
    fn from(err: PluginError) -> c_int {
        err as c_int
    }
}
