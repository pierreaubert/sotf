mod error;
mod eval;
pub mod gc;
pub mod jit;
pub mod obarray;
mod object;
mod primitives;
mod reader;
pub mod value;
pub mod vm;

pub use error::{ElispError, ElispResult};
pub use eval::Interpreter;
pub use object::{global_cons_count, BytecodeFunction, LispObject};
pub use primitives::add_primitives;
pub use reader::{detect_lexical_binding, read, read_all};

pub trait EditorCallbacks: Send + Sync {
    fn buffer_string(&self) -> String;
    fn buffer_size(&self) -> usize;
    fn point(&self) -> usize;
    fn insert(&mut self, text: &str);
    fn delete_char(&mut self, n: i64);
    fn goto_char(&mut self, pos: usize);
    fn forward_char(&mut self, n: i64);
    fn find_file(&mut self, path: &str) -> bool;
    fn save_buffer(&mut self) -> bool;
}
