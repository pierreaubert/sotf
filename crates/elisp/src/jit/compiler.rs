//! Cranelift JIT compiler for Emacs Lisp bytecode functions.
//!
//! This module is only compiled when the `jit` feature is enabled.
//! For now it contains the structural skeleton -- types and lifecycle
//! methods -- without actual IR generation for individual opcodes.

use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use std::collections::HashMap;
use std::sync::Arc;

use crate::object::BytecodeFunction;

/// Opaque handle to a compiled native function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompiledFuncId(u32);

/// Outcome of attempting to run compiled native code.
pub enum NativeResult {
    /// The native code executed successfully and produced a value index
    /// (to be resolved by the caller into a `LispObject`).
    Ok(u64),
    /// A type guard or assumption failed -- the caller should fall back
    /// to the bytecode VM for this invocation.
    Deoptimize,
}

/// Cranelift-based JIT compiler.
///
/// Holds the Cranelift `JITModule` and a mapping from bytecode function
/// ids to their compiled native counterparts.
pub struct JitCompiler {
    /// The Cranelift JIT module that owns emitted machine code.
    module: JITModule,
    /// The target ISA (architecture + settings) used for compilation.
    _isa: Arc<dyn TargetIsa>,
    /// Map from bytecode function id to compiled function handle.
    compiled: HashMap<usize, CompiledFuncId>,
    /// Monotonic counter for assigning `CompiledFuncId`s.
    next_id: u32,
}

impl JitCompiler {
    /// Create a new JIT compiler targeting the host machine.
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
        // Use speed optimizations for JIT-compiled code.
        flag_builder.set("opt_level", "speed").unwrap();

        let isa_builder = cranelift_native::builder().expect("unsupported host architecture");
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .unwrap();

        let builder = JITBuilder::with_isa(isa.clone(), cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);

        Self {
            module,
            _isa: isa,
            compiled: HashMap::new(),
            next_id: 0,
        }
    }

    /// Returns `true` -- native compilation is available.
    pub fn is_available() -> bool {
        true
    }

    /// Returns `true` if `func_id` has already been compiled to native code.
    pub fn is_compiled(&self, func_id: usize) -> bool {
        self.compiled.contains_key(&func_id)
    }

    /// Compile a bytecode function to native code.
    ///
    /// Returns the handle on success, or `None` if the function contains
    /// opcodes that are not yet supported by the JIT.
    ///
    /// This is a skeleton -- actual opcode-to-IR lowering is not yet
    /// implemented. Currently always returns `None`.
    pub fn compile(&mut self, func_id: usize, _func: &BytecodeFunction) -> Option<CompiledFuncId> {
        if let Some(&id) = self.compiled.get(&func_id) {
            return Some(id);
        }

        // TODO: translate bytecode opcodes to Cranelift IR
        //  1. Create a FunctionBuilder
        //  2. Walk func.bytecode, emitting IR per opcode
        //  3. Emit type guards on the fast path
        //  4. Emit deopt exits that return NativeResult::Deoptimize
        //  5. Finalize, define, and get a native code pointer

        let _ = &self.module; // suppress unused field warning
        None // not yet implemented
    }

    /// Invalidate the compiled code for `func_id`.
    ///
    /// Called when a function is redefined at runtime so the stale
    /// native code is no longer used. The next hot invocation will
    /// trigger recompilation via the profiler.
    pub fn invalidate(&mut self, func_id: usize) {
        self.compiled.remove(&func_id);
        // Note: the machine code memory is owned by the JITModule and
        // cannot be individually freed. A future improvement could
        // track and reclaim it.
    }

    /// Allocate the next `CompiledFuncId`. (utility for future use)
    #[allow(dead_code)]
    fn alloc_id(&mut self) -> CompiledFuncId {
        let id = CompiledFuncId(self.next_id);
        self.next_id += 1;
        id
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}
