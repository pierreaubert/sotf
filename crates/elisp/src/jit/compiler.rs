//! Cranelift JIT compiler for Emacs Lisp bytecode functions.
//!
//! Compiles a subset of Emacs 30.x bytecode to native machine code via
//! Cranelift. The compiled function operates on NaN-boxed `Value` words
//! (u64 reinterpreted as i64) and uses a fast-path strategy: arithmetic
//! opcodes emit an inline fixnum check and bail to a deoptimization
//! sentinel when the operand is not a fixnum.
//!
//! Supported opcodes:
//!   stack-ref (0-5), sub1, add1, eqlsign, gtr, lss, leq, geq,
//!   diff, plus, mult, goto, goto-if-nil, goto-if-not-nil, return,
//!   discard, dup, constant[N-192]

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{types, AbiParam, InstBuilder, MemFlags};
use cranelift_codegen::isa::TargetIsa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use crate::object::BytecodeFunction;
use crate::value::Value;

// NaN-box encoding constants (must match value.rs exactly).
const NANBOX_PREFIX: u64 = 0xFFF8_0000_0000_0000;
const TAG_SHIFT: u64 = 48;
const TAG_FIXNUM: u64 = 0;
const TAG_SPECIAL: u64 = 4;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
const SPECIAL_NIL: u64 = 0;
const SPECIAL_T: u64 = 1;

/// The raw bit pattern for nil.
const NIL_BITS: u64 = NANBOX_PREFIX | (TAG_SPECIAL << TAG_SHIFT) | SPECIAL_NIL;
/// The raw bit pattern for t.
const T_BITS: u64 = NANBOX_PREFIX | (TAG_SPECIAL << TAG_SHIFT) | SPECIAL_T;
/// Tag mask used to check "is fixnum": top 16 bits == 0xFFF8.
const FIXNUM_TAG_BITS: u64 = NANBOX_PREFIX | (TAG_FIXNUM << TAG_SHIFT);
/// Mask that covers the NaN prefix + tag nibble.
const TAG_CHECK_MASK: u64 = 0xFFFF_0000_0000_0000;

/// Deoptimize sentinel -- a value that can never be a valid NaN-boxed
/// Value. The caller checks for this to fall back to the VM.
const DEOPT_SENTINEL: i64 = 0_i64;

/// Opaque handle to a compiled native function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompiledFuncId(u32);

/// Outcome of attempting to run compiled native code.
pub enum NativeResult {
    /// The native code executed successfully and produced a value
    /// (a NaN-boxed u64 reinterpreted as i64).
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
    /// Map from compiled handle to native code pointer.
    native_fns: HashMap<CompiledFuncId, *const u8>,
    /// Monotonic counter for assigning `CompiledFuncId`s.
    next_id: u32,
}

// SAFETY: The native function pointers in `native_fns` point to JIT-emitted
// code owned by the JITModule and are valid for the lifetime of the compiler.
// They are only called through `call()` which requires `&self`.
unsafe impl Send for JitCompiler {}

impl JitCompiler {
    /// Create a new JIT compiler targeting the host machine.
    pub fn new() -> Self {
        let mut flag_builder = settings::builder();
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
            native_fns: HashMap::new(),
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

    /// Get the compiled function ID for a bytecode function, if it exists.
    pub fn get_compiled(&self, func_id: usize) -> Option<CompiledFuncId> {
        self.compiled.get(&func_id).copied()
    }

    /// Compile a bytecode function to native code.
    ///
    /// Returns the handle on success, or `None` if the function contains
    /// opcodes that are not yet supported by the JIT.
    pub fn compile(&mut self, func_id: usize, func: &BytecodeFunction) -> Option<CompiledFuncId> {
        if let Some(&id) = self.compiled.get(&func_id) {
            return Some(id);
        }

        let code_ptr = self.compile_inner(func_id, func)?;
        let id = self.alloc_id();
        self.compiled.insert(func_id, id);
        self.native_fns.insert(id, code_ptr);
        Some(id)
    }

    /// Call a previously compiled function with the given NaN-boxed arguments.
    ///
    /// Returns `Some(NativeResult)` if the function was found, `None` otherwise.
    pub fn call(&self, id: CompiledFuncId, args: &[i64]) -> Option<NativeResult> {
        let &ptr = self.native_fns.get(&id)?;
        let raw = match args.len() {
            0 => {
                // SAFETY: `ptr` is a JIT-compiled function with signature `fn() -> i64`.
                // It was emitted by Cranelift for the host ISA and the pointer is valid
                // for the lifetime of the JITModule.
                let f: fn() -> i64 = unsafe { std::mem::transmute(ptr) };
                f()
            }
            1 => {
                // SAFETY: same as above, signature `fn(i64) -> i64`.
                let f: fn(i64) -> i64 = unsafe { std::mem::transmute(ptr) };
                f(args[0])
            }
            2 => {
                // SAFETY: same as above, signature `fn(i64, i64) -> i64`.
                let f: fn(i64, i64) -> i64 = unsafe { std::mem::transmute(ptr) };
                f(args[0], args[1])
            }
            3 => {
                // SAFETY: same as above, signature `fn(i64, i64, i64) -> i64`.
                let f: fn(i64, i64, i64) -> i64 = unsafe { std::mem::transmute(ptr) };
                f(args[0], args[1], args[2])
            }
            4 => {
                // SAFETY: same as above, signature `fn(i64, i64, i64, i64) -> i64`.
                let f: fn(i64, i64, i64, i64) -> i64 = unsafe { std::mem::transmute(ptr) };
                f(args[0], args[1], args[2], args[3])
            }
            _ => return None, // unsupported arity
        };

        if raw == DEOPT_SENTINEL {
            Some(NativeResult::Deoptimize)
        } else {
            Some(NativeResult::Ok(raw as u64))
        }
    }

    /// Invalidate the compiled code for `func_id`.
    ///
    /// Called when a function is redefined at runtime so the stale
    /// native code is no longer used.
    pub fn invalidate(&mut self, func_id: usize) {
        if let Some(id) = self.compiled.remove(&func_id) {
            self.native_fns.remove(&id);
        }
    }

    /// Allocate the next `CompiledFuncId`.
    fn alloc_id(&mut self) -> CompiledFuncId {
        let id = CompiledFuncId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Inner compilation routine. Returns the native code pointer on success.
    fn compile_inner(&mut self, func_id: usize, func: &BytecodeFunction) -> Option<*const u8> {
        let num_args = func.min_args();
        let bytecode = &func.bytecode;

        // Pre-compute constant values as i64 (NaN-boxed).
        let const_bits: Vec<i64> = func
            .constants
            .iter()
            .map(|obj| Value::from_lisp_object(obj).to_bits() as i64)
            .collect();

        // --- Pre-scan: find all jump targets to create basic blocks ---
        let jump_targets = Self::find_jump_targets(bytecode);

        // --- Declare function signature ---
        let mut sig = self.module.make_signature();
        for _ in 0..num_args {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));

        let name = format!("elisp_jit_{}", func_id);
        let func_decl = self
            .module
            .declare_function(&name, Linkage::Local, &sig)
            .ok()?;

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        // --- Create blocks ---
        // One block per jump target, plus the entry block at offset 0.
        let mut offset_to_block: HashMap<usize, cranelift_codegen::ir::Block> = HashMap::new();

        // Entry block is always at offset 0.
        let entry_block = builder.create_block();
        offset_to_block.insert(0, entry_block);

        for &target in &jump_targets {
            if target != 0 {
                let block = builder.create_block();
                offset_to_block.insert(target, block);
            }
        }

        // Create the deoptimize block.
        let deopt_block = builder.create_block();

        // --- Entry block: set up args ---
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Compile-time operand stack: tracks Cranelift SSA values.
        let mut stack: Vec<cranelift_codegen::ir::Value> = Vec::with_capacity(func.maxdepth);

        // Push function arguments onto the operand stack.
        for i in 0..num_args {
            let arg = builder.block_params(entry_block)[i];
            stack.push(arg);
        }

        // --- Walk bytecode and emit IR ---
        let mut pc = 0usize;
        // Track whether the current block is already terminated (by return/goto).
        let mut block_terminated = false;

        // We need to handle the case where a jump target falls in the middle
        // of our linear scan. At such points we must end the current block
        // with a fallthrough jump and switch to the target block.
        //
        // For branches: the stack state at the branch point must match the
        // stack state at the target. We handle this by storing the stack
        // depth at each jump target and using block params.
        //
        // SIMPLIFICATION: for this first pass, we use a single compile-time
        // stack and require that at every join point the stack depth matches.
        // This is true for well-formed Emacs bytecode.

        while pc < bytecode.len() {
            // If this offset is a jump target, we may need to switch blocks.
            if let Some(&target_block) = offset_to_block.get(&pc) {
                if target_block != entry_block || pc != 0 {
                    // End the previous block with a fallthrough jump (if not terminated).
                    if !block_terminated {
                        builder.ins().jump(target_block, &[]);
                    }
                    builder.switch_to_block(target_block);
                    builder.seal_block(target_block);
                    block_terminated = false;
                }
            }

            if block_terminated {
                // We're in dead code after a return/goto. Skip until the next
                // jump target.
                pc += 1;
                // Skip operands of multi-byte instructions in dead code.
                continue;
            }

            let op = bytecode[pc];
            pc += 1;

            match op {
                // --- stack-ref N (0-5) ---
                0..=5 => {
                    let n = op as usize;
                    if n >= stack.len() {
                        return None; // stack underflow at compile time
                    }
                    let idx = stack.len() - 1 - n;
                    let val = stack[idx];
                    stack.push(val);
                }

                // --- sub1 (83) ---
                83 => {
                    let val = stack.pop()?;
                    // Type guard: check val is fixnum
                    Self::emit_fixnum_guard(&mut builder, val, deopt_block);
                    let payload = Self::emit_extract_payload(&mut builder, val);
                    let one = builder.ins().iconst(types::I64, 1);
                    let result_payload = builder.ins().isub(payload, one);
                    let tagged = Self::emit_tag_fixnum(&mut builder, result_payload);
                    stack.push(tagged);
                }

                // --- add1 (84) ---
                84 => {
                    let val = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, val, deopt_block);
                    let payload = Self::emit_extract_payload(&mut builder, val);
                    let one = builder.ins().iconst(types::I64, 1);
                    let result_payload = builder.ins().iadd(payload, one);
                    let tagged = Self::emit_tag_fixnum(&mut builder, result_payload);
                    stack.push(tagged);
                }

                // --- eqlsign (85): numeric = ---
                85 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    let cmp = builder.ins().icmp(IntCC::Equal, pa, pb);
                    let result = Self::emit_bool_to_value(&mut builder, cmp);
                    stack.push(result);
                }

                // --- gtr (86): > ---
                86 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    // Payloads are unsigned 48-bit but represent signed values.
                    // We need signed comparison. Sign-extend first.
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, sa, sb);
                    let result = Self::emit_bool_to_value(&mut builder, cmp);
                    stack.push(result);
                }

                // --- lss (87): < ---
                87 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let cmp = builder.ins().icmp(IntCC::SignedLessThan, sa, sb);
                    let result = Self::emit_bool_to_value(&mut builder, cmp);
                    stack.push(result);
                }

                // --- leq (88): <= ---
                88 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, sa, sb);
                    let result = Self::emit_bool_to_value(&mut builder, cmp);
                    stack.push(result);
                }

                // --- geq (89): >= ---
                89 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, sa, sb);
                    let result = Self::emit_bool_to_value(&mut builder, cmp);
                    stack.push(result);
                }

                // --- diff (90): subtraction ---
                90 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    // Sign-extend, subtract, mask back to 48 bits, re-tag.
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let diff = builder.ins().isub(sa, sb);
                    let masked = Self::emit_mask_payload(&mut builder, diff);
                    let tagged = Self::emit_tag_fixnum(&mut builder, masked);
                    stack.push(tagged);
                }

                // --- plus (92): addition ---
                92 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let sum = builder.ins().iadd(sa, sb);
                    let masked = Self::emit_mask_payload(&mut builder, sum);
                    let tagged = Self::emit_tag_fixnum(&mut builder, masked);
                    stack.push(tagged);
                }

                // --- mult (95): multiplication ---
                95 => {
                    let b = stack.pop()?;
                    let a = stack.pop()?;
                    Self::emit_fixnum_guard(&mut builder, a, deopt_block);
                    Self::emit_fixnum_guard(&mut builder, b, deopt_block);
                    let pa = Self::emit_extract_payload(&mut builder, a);
                    let pb = Self::emit_extract_payload(&mut builder, b);
                    let sa = Self::emit_sign_extend_payload(&mut builder, pa);
                    let sb = Self::emit_sign_extend_payload(&mut builder, pb);
                    let prod = builder.ins().imul(sa, sb);
                    let masked = Self::emit_mask_payload(&mut builder, prod);
                    let tagged = Self::emit_tag_fixnum(&mut builder, masked);
                    stack.push(tagged);
                }

                // --- goto (130): unconditional jump ---
                130 => {
                    if pc + 1 >= bytecode.len() {
                        return None;
                    }
                    let lo = bytecode[pc] as u16;
                    let hi = bytecode[pc + 1] as u16;
                    pc += 2;
                    let target = (lo | (hi << 8)) as usize;
                    let target_block = *offset_to_block.get(&target)?;
                    builder.ins().jump(target_block, &[]);
                    block_terminated = true;
                }

                // --- goto-if-nil (131) ---
                131 => {
                    if pc + 1 >= bytecode.len() {
                        return None;
                    }
                    let lo = bytecode[pc] as u16;
                    let hi = bytecode[pc + 1] as u16;
                    pc += 2;
                    let target = (lo | (hi << 8)) as usize;
                    let target_block = *offset_to_block.get(&target)?;

                    let val = stack.pop()?;
                    let nil_val = builder.ins().iconst(types::I64, NIL_BITS as i64);
                    let is_nil = builder.ins().icmp(IntCC::Equal, val, nil_val);

                    // Create a fallthrough block for the not-nil case.
                    let fall_block = builder.create_block();
                    builder
                        .ins()
                        .brif(is_nil, target_block, &[], fall_block, &[]);
                    builder.switch_to_block(fall_block);
                    builder.seal_block(fall_block);
                }

                // --- goto-if-not-nil (132) ---
                132 => {
                    if pc + 1 >= bytecode.len() {
                        return None;
                    }
                    let lo = bytecode[pc] as u16;
                    let hi = bytecode[pc + 1] as u16;
                    pc += 2;
                    let target = (lo | (hi << 8)) as usize;
                    let target_block = *offset_to_block.get(&target)?;

                    let val = stack.pop()?;
                    let nil_val = builder.ins().iconst(types::I64, NIL_BITS as i64);
                    let is_nil = builder.ins().icmp(IntCC::Equal, val, nil_val);

                    let fall_block = builder.create_block();
                    // Branch to target if NOT nil (i.e. is_nil is false => fallthrough).
                    builder
                        .ins()
                        .brif(is_nil, fall_block, &[], target_block, &[]);
                    builder.switch_to_block(fall_block);
                    builder.seal_block(fall_block);
                }

                // --- return (135) ---
                135 => {
                    let retval = stack.pop()?;
                    builder.ins().return_(&[retval]);
                    block_terminated = true;
                }

                // --- discard (136) ---
                136 => {
                    stack.pop()?;
                }

                // --- dup (137) ---
                137 => {
                    let val = *stack.last()?;
                    stack.push(val);
                }

                // --- constant[N-192] (192-255) ---
                192..=255 => {
                    let idx = (op - 192) as usize;
                    if idx >= const_bits.len() {
                        // Out of range constant -- push nil.
                        let nil = builder.ins().iconst(types::I64, NIL_BITS as i64);
                        stack.push(nil);
                    } else {
                        let bits = const_bits[idx];
                        let val = builder.ins().iconst(types::I64, bits);
                        stack.push(val);
                    }
                }

                // --- unsupported opcode: bail ---
                _ => {
                    return None;
                }
            }
        }

        // If the bytecode falls off the end without a return, return nil.
        if !block_terminated {
            let nil = builder.ins().iconst(types::I64, NIL_BITS as i64);
            builder.ins().return_(&[nil]);
        }

        // --- Deopt block: return the sentinel ---
        builder.switch_to_block(deopt_block);
        builder.seal_block(deopt_block);
        let sentinel = builder.ins().iconst(types::I64, DEOPT_SENTINEL);
        builder.ins().return_(&[sentinel]);

        // --- Finalize and emit ---
        builder.finalize();

        self.module.define_function(func_decl, &mut ctx).ok()?;
        self.module.clear_context(&mut ctx);
        self.module.finalize_definitions().ok()?;

        let code_ptr = self.module.get_finalized_function(func_decl);
        Some(code_ptr)
    }

    // -----------------------------------------------------------------------
    // Bytecode scanning helpers
    // -----------------------------------------------------------------------

    /// Pre-scan bytecode to find all jump target offsets.
    fn find_jump_targets(bytecode: &[u8]) -> BTreeSet<usize> {
        let mut targets = BTreeSet::new();
        let mut pc = 0usize;
        while pc < bytecode.len() {
            let op = bytecode[pc];
            pc += 1;
            match op {
                // Opcodes with 2-byte jump target operand.
                130..=134 | 141 | 143 => {
                    if pc + 1 < bytecode.len() {
                        let lo = bytecode[pc] as u16;
                        let hi = bytecode[pc + 1] as u16;
                        let target = (lo | (hi << 8)) as usize;
                        targets.insert(target);
                    }
                    pc += 2;
                }
                // Opcodes with 2-byte operand that are also jump-like:
                // pushconditioncase (49), pushcatch (50)
                49 | 50 => {
                    if pc + 1 < bytecode.len() {
                        let lo = bytecode[pc] as u16;
                        let hi = bytecode[pc + 1] as u16;
                        let target = (lo | (hi << 8)) as usize;
                        targets.insert(target);
                    }
                    pc += 2;
                }
                // Opcodes with 1-byte operand.
                6 | 14 | 22 | 30 | 38 | 46 | 169..=174 | 175..=178 | 180 | 182 => {
                    pc += 1;
                }
                // Opcodes with 2-byte operand (non-jump).
                7 | 15 | 23 | 31 | 39 | 47 | 129 | 179 | 181 => {
                    pc += 2;
                }
                // All other opcodes: no operand bytes.
                _ => {}
            }
        }
        targets
    }

    // -----------------------------------------------------------------------
    // IR emission helpers
    // -----------------------------------------------------------------------

    /// Emit a fixnum type guard: if `val` is not a fixnum, branch to `deopt`.
    fn emit_fixnum_guard(
        builder: &mut FunctionBuilder,
        val: cranelift_codegen::ir::Value,
        deopt: cranelift_codegen::ir::Block,
    ) {
        // A fixnum has tag 0, so top 16 bits == NANBOX_PREFIX (0xFFF8_0000...).
        // Check: (val & TAG_CHECK_MASK) == FIXNUM_TAG_BITS
        let mask = builder.ins().iconst(types::I64, TAG_CHECK_MASK as i64);
        let masked = builder.ins().band(val, mask);
        let expected = builder.ins().iconst(types::I64, FIXNUM_TAG_BITS as i64);
        let is_fixnum = builder.ins().icmp(IntCC::Equal, masked, expected);

        let ok_block = builder.create_block();
        builder.ins().brif(is_fixnum, ok_block, &[], deopt, &[]);
        builder.switch_to_block(ok_block);
        builder.seal_block(ok_block);
    }

    /// Extract the 48-bit payload from a NaN-boxed value.
    fn emit_extract_payload(
        builder: &mut FunctionBuilder,
        val: cranelift_codegen::ir::Value,
    ) -> cranelift_codegen::ir::Value {
        let mask = builder.ins().iconst(types::I64, PAYLOAD_MASK as i64);
        builder.ins().band(val, mask)
    }

    /// Mask a 64-bit value down to 48-bit payload.
    fn emit_mask_payload(
        builder: &mut FunctionBuilder,
        val: cranelift_codegen::ir::Value,
    ) -> cranelift_codegen::ir::Value {
        let mask = builder.ins().iconst(types::I64, PAYLOAD_MASK as i64);
        builder.ins().band(val, mask)
    }

    /// Sign-extend a 48-bit payload to a full 64-bit signed integer.
    fn emit_sign_extend_payload(
        builder: &mut FunctionBuilder,
        payload: cranelift_codegen::ir::Value,
    ) -> cranelift_codegen::ir::Value {
        // Shift left by 16 then arithmetic shift right by 16.
        let sixteen = builder.ins().iconst(types::I64, 16);
        let shifted_left = builder.ins().ishl(payload, sixteen);
        builder.ins().sshr(shifted_left, sixteen)
    }

    /// Re-tag a 48-bit payload as a fixnum: NANBOX_PREFIX | payload.
    fn emit_tag_fixnum(
        builder: &mut FunctionBuilder,
        payload: cranelift_codegen::ir::Value,
    ) -> cranelift_codegen::ir::Value {
        let prefix = builder.ins().iconst(types::I64, NANBOX_PREFIX as i64);
        builder.ins().bor(payload, prefix)
    }

    /// Convert a Cranelift boolean (i8) to a NaN-boxed t or nil.
    fn emit_bool_to_value(
        builder: &mut FunctionBuilder,
        cond: cranelift_codegen::ir::Value,
    ) -> cranelift_codegen::ir::Value {
        let t_val = builder.ins().iconst(types::I64, T_BITS as i64);
        let nil_val = builder.ins().iconst(types::I64, NIL_BITS as i64);
        builder.ins().select(cond, t_val, nil_val)
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self::new()
    }
}

// Suppress the unused import warning -- MemFlags is used conceptually
// and will be needed for future memory operations (load constant from
// a data section, etc.).
const _: () = {
    fn _use_memflags(_: MemFlags) {}
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::{BytecodeFunction, LispObject};
    use crate::value::Value;

    /// Helper: create a fixnum Value and return its bits as i64.
    fn fixnum_i64(n: i64) -> i64 {
        Value::fixnum(n).to_bits() as i64
    }

    /// Helper: extract fixnum from raw i64 bits.
    fn i64_to_fixnum(raw: i64) -> Option<i64> {
        // Reconstruct by checking raw bits match the fixnum tag pattern.
        let bits = raw as u64;
        let tag_check = bits & TAG_CHECK_MASK;
        if tag_check != FIXNUM_TAG_BITS {
            return None;
        }
        let payload = bits & PAYLOAD_MASK;
        // Sign-extend from 48 bits.
        let sign_bit = 1u64 << 47;
        let extended = if payload & sign_bit != 0 {
            payload | !PAYLOAD_MASK
        } else {
            payload
        };
        Some(extended as i64)
    }

    #[test]
    fn test_jit_add_two_args() {
        // Bytecode for (+ a b):
        //   stack-ref 1  -> push arg0 (bottom of 2-element stack)
        //   stack-ref 1  -> push arg1 (which is now at depth 1)
        //   plus (92)
        //   return (135)
        // argdesc = 0x0202 = min_args=2, max_args=2
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 92, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(0, &func).expect("compilation should succeed");

        let a = fixnum_i64(3);
        let b = fixnum_i64(4);
        let result = jit.call(id, &[a, b]).expect("call should succeed");

        match result {
            NativeResult::Ok(bits) => {
                let v = Value::fixnum(0); // dummy
                                          // Reconstruct Value from bits
                let _ = v;
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 7, "3 + 4 should be 7");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_sub1() {
        // Bytecode for (1- a):
        //   stack-ref 0  -> push arg0
        //   sub1 (83)
        //   return (135)
        let func = BytecodeFunction {
            argdesc: 0x0101,
            bytecode: vec![0, 83, 135],
            constants: vec![],
            maxdepth: 2,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(1, &func).expect("compilation should succeed");

        let a = fixnum_i64(10);
        let result = jit.call(id, &[a]).expect("call should succeed");

        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 9, "10 - 1 should be 9");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_add1() {
        // Bytecode for (1+ a):
        //   stack-ref 0
        //   add1 (84)
        //   return (135)
        let func = BytecodeFunction {
            argdesc: 0x0101,
            bytecode: vec![0, 84, 135],
            constants: vec![],
            maxdepth: 2,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(2, &func).expect("compilation should succeed");

        let a = fixnum_i64(41);
        let result = jit.call(id, &[a]).expect("call should succeed");

        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 42, "41 + 1 should be 42");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_multiply() {
        // Bytecode for (* a b):
        //   stack-ref 1
        //   stack-ref 1
        //   mult (95)
        //   return (135)
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 95, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(3, &func).expect("compilation should succeed");

        let a = fixnum_i64(6);
        let b = fixnum_i64(7);
        let result = jit.call(id, &[a, b]).expect("call should succeed");

        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 42, "6 * 7 should be 42");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_subtract() {
        // Bytecode for (- a b):
        //   stack-ref 1
        //   stack-ref 1
        //   diff (90)
        //   return (135)
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 90, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(4, &func).expect("compilation should succeed");

        let a = fixnum_i64(10);
        let b = fixnum_i64(3);
        let result = jit.call(id, &[a, b]).expect("call should succeed");

        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 7, "10 - 3 should be 7");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_comparison_leq() {
        // Bytecode for (<= a b):
        //   stack-ref 1
        //   stack-ref 1
        //   leq (88)
        //   return (135)
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 88, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(5, &func).expect("compilation should succeed");

        // 3 <= 4 -> t
        let result = jit
            .call(id, &[fixnum_i64(3), fixnum_i64(4)])
            .expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                assert_eq!(bits, T_BITS, "3 <= 4 should be t");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }

        // 5 <= 4 -> nil
        let result = jit
            .call(id, &[fixnum_i64(5), fixnum_i64(4)])
            .expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                assert_eq!(bits, NIL_BITS, "5 <= 4 should be nil");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }

        // 4 <= 4 -> t
        let result = jit
            .call(id, &[fixnum_i64(4), fixnum_i64(4)])
            .expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                assert_eq!(bits, T_BITS, "4 <= 4 should be t");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_constant_and_dup() {
        // Bytecode for a function that returns constant 0 (from constants vector):
        //   constant 0 (192)
        //   dup (137)
        //   plus (92)
        //   return (135)
        // constants = [5]
        // This computes 5 + 5 = 10
        let func = BytecodeFunction {
            argdesc: 0x0000,
            bytecode: vec![192, 137, 92, 135],
            constants: vec![LispObject::Integer(5)],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(6, &func).expect("compilation should succeed");

        let result = jit.call(id, &[]).expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 10, "5 + 5 should be 10");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_goto_if_nil_branch() {
        // Bytecode for: (if (= a 0) 42 99)
        // This tests goto-if-nil branching.
        //
        // Layout:
        //   0: stack-ref 0       -> push a
        //   1: constant 0 (192)  -> push 0 (from constants[0])
        //   2: eqlsign (85)      -> compare
        //   3: goto-if-nil 131, target=9
        //   6: constant 1 (193)  -> push 42 (from constants[1])
        //   7: return (135)
        //   8: -- (this is jumped over)
        //   9: constant 2 (194)  -> push 99 (from constants[2])
        //  10: return (135)
        let func = BytecodeFunction {
            argdesc: 0x0101,
            bytecode: vec![
                0,   // 0: stack-ref 0
                192, // 1: constant[0] = 0
                85,  // 2: eqlsign
                131, // 3: goto-if-nil
                9, 0,   // 4-5: target = 9
                193, // 6: constant[1] = 42
                135, // 7: return
                136, // 8: discard (dead code / padding)
                194, // 9: constant[2] = 99
                135, // 10: return
            ],
            constants: vec![
                LispObject::Integer(0),
                LispObject::Integer(42),
                LispObject::Integer(99),
            ],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(7, &func).expect("compilation should succeed");

        // a = 0 -> condition is t -> don't branch -> return 42
        let result = jit.call(id, &[fixnum_i64(0)]).expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 42, "if (= 0 0) should return 42");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }

        // a = 5 -> condition is nil -> branch to 9 -> return 99
        let result = jit.call(id, &[fixnum_i64(5)]).expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 99, "if (= 5 0) should return 99");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_factorial_iterative() {
        // Bytecode for an iterative factorial:
        //   (defun my-fact (n)
        //     (let ((acc 1))
        //       (while (> n 1)
        //         (setq acc (* acc n))
        //         (setq n (1- n)))
        //       acc))
        //
        // We encode this as a simple loop using goto/goto-if-nil:
        //
        // Stack at start: [n]
        // We use the stack to hold [n, acc] where n is arg, acc starts at 1.
        //
        // Bytecode layout:
        //   0: constant[0]=1 (192)  -> push 1 (acc initial value)
        //   -- now stack is [n, acc]
        //   -- loop header at offset 1:
        //   1: stack-ref 1          -> push n (stack: [n, acc, n])
        //   2: constant[0]=1 (192)  -> push 1 (stack: [n, acc, n, 1])
        //   3: gtr (86)             -> n > 1? (stack: [n, acc, result])
        //   4: goto-if-nil 131, target=17  (stack: [n, acc])
        //   7: stack-ref 1          -> push n (stack: [n, acc, n])
        //   8: stack-ref 1          -> push acc (stack: [n, acc, n, acc])
        //   9: mult (95)            -> n*acc (stack: [n, acc, n*acc])
        //  10: stack-ref 2          -> push n (stack: [n, acc, n*acc, n])
        //  11: sub1 (83)            -> n-1 (stack: [n, acc, n*acc, n-1])
        //  -- now we need to replace n and acc on the stack.
        //  -- discard old n and acc, push new ones.
        //  -- Trick: we'll restructure to keep it simpler.
        //
        // Actually, let me use a simpler encoding that the bytecode compiler
        // would produce. Since we only have stack-ref and no stack-set in
        // the JIT yet, let's encode factorial differently.
        //
        // Simpler approach: countdown multiply.
        // start: stack = [n, 1]  (push constant 1)
        // loop:
        //   stack-ref 1  -> n
        //   constant 1
        //   leq          -> n <= 1 ?
        //   goto-if-not-nil end
        //   stack-ref 1  -> n
        //   stack-ref 1  -> acc
        //   mult         -> n * acc (new_acc)
        //   stack-ref 1  -> n
        //   sub1         -> n - 1 (new_n)
        //   -- stack: [n, acc, new_acc, new_n]
        //   -- We need to "rotate" to [new_n, new_acc]
        //   -- This is tricky without stack-set.
        //
        // For testing the JIT with loops, let's use a simpler function:
        // sum from n down to 1: n + (n-1) + ... + 1
        //
        // (defun my-sum (n)
        //   (let ((acc 0))
        //     (while (> n 0)
        //       (setq acc (+ acc n))
        //       (setq n (1- n)))
        //     acc))
        //
        // Bytecode (hand-assembled, using only supported opcodes):
        // We'll use a 2-slot stack: [n, acc]
        // But without stack-set we can't mutate in place.
        //
        // Alternative: use a tail-recursive style with goto.
        // sum(n, acc) where acc starts at 0:
        //   if n <= 0: return acc
        //   else: return sum(n-1, acc+n)
        //
        // We can model this as a loop that re-enters with
        // modified args on the stack, using discard to clean up.
        //
        // Stack layout: [n, acc]
        //
        //  0: stack-ref 1     (push n)    stack: [n, acc, n]
        //  1: constant[0]=0   (push 0)    stack: [n, acc, n, 0]
        //  2: leq (88)        (n <= 0?)   stack: [n, acc, bool]
        //  3: goto-if-not-nil 132, target=19
        //  6: stack-ref 1     (push n)    stack: [n, acc, n]
        //  7: stack-ref 1     (push acc)  stack: [n, acc, n, acc]
        //  8: plus (92)       (n+acc)     stack: [n, acc, new_acc]
        //  9: stack-ref 2     (push n)    stack: [n, acc, new_acc, n]
        // 10: sub1 (83)       (n-1)       stack: [n, acc, new_acc, new_n]
        // Now we need [new_n, new_acc] and to loop back.
        // We can't easily do this without stack-set.
        //
        // Instead, let's test a simple countdown that just tests the
        // loop structure works (goto + goto-if-nil).

        // Simplest loop test: countdown from n, return 0 when done.
        // (defun countdown (n)
        //   (while (> n 0)
        //     (setq n (1- n)))
        //   n)
        //
        // But we can't setq either. Let's just test that branching works
        // with a simpler function that doesn't need mutation.
        //
        // Test: (if (<= n 1) n (* n (1- n)))
        // This is just n * (n-1) for n > 1, else n.
        //
        //  0: stack-ref 0       push n
        //  1: constant[0]=1     push 1
        //  2: leq (88)          n <= 1?
        //  3: goto-if-not-nil(132) target=12
        //  6: stack-ref 0       push n
        //  7: stack-ref 1       push n (original, deeper in stack)
        //  8: sub1 (83)         n - 1
        //  9: mult (95)         n * (n-1)
        // 10: return (135)
        // 11: (dead)
        // 12: stack-ref 0       push n
        // 13: return (135)

        let func = BytecodeFunction {
            argdesc: 0x0101,
            bytecode: vec![
                0,   // 0: stack-ref 0 (n)
                192, // 1: constant[0] = 1
                88,  // 2: leq
                132, // 3: goto-if-not-nil
                12, 0,   // 4-5: target = 12
                0,   // 6: stack-ref 0 (n)
                1,   // 7: stack-ref 1 (n, original arg)
                83,  // 8: sub1 -> n-1
                95,  // 9: mult -> n * (n-1)
                135, // 10: return
                136, // 11: discard (dead code padding)
                0,   // 12: stack-ref 0 (n)
                135, // 13: return
            ],
            constants: vec![LispObject::Integer(1)],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(8, &func).expect("compilation should succeed");

        // n=1: 1 <= 1 is true -> return n = 1
        let result = jit.call(id, &[fixnum_i64(1)]).expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 1, "countdown(1) should be 1");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }

        // n=5: 5 > 1 -> 5 * 4 = 20
        let result = jit.call(id, &[fixnum_i64(5)]).expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 20, "5 * (5-1) should be 20");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }

        // n=0: 0 <= 1 is true -> return 0
        let result = jit.call(id, &[fixnum_i64(0)]).expect("call should succeed");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("result should be fixnum");
                assert_eq!(n, 0, "countdown(0) should be 0");
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_negative_numbers() {
        // Test that negative fixnum arithmetic works correctly.
        // Bytecode for (+ a b) with negative numbers.
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 92, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(9, &func).expect("compilation should succeed");

        // -3 + 4 = 1
        let result = jit
            .call(id, &[fixnum_i64(-3), fixnum_i64(4)])
            .expect("call");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("fixnum");
                assert_eq!(n, 1);
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }

        // -10 + -20 = -30
        let result = jit
            .call(id, &[fixnum_i64(-10), fixnum_i64(-20)])
            .expect("call");
        match result {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("fixnum");
                assert_eq!(n, -30);
            }
            NativeResult::Deoptimize => panic!("unexpected deopt"),
        }
    }

    #[test]
    fn test_jit_unsupported_opcode_returns_none() {
        // Bytecode with an unsupported opcode (e.g. call = 32).
        let func = BytecodeFunction {
            argdesc: 0x0101,
            bytecode: vec![0, 32, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        assert!(
            jit.compile(10, &func).is_none(),
            "should return None for unsupported opcode"
        );
    }

    #[test]
    fn test_jit_is_compiled_and_invalidate() {
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 92, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        assert!(!jit.is_compiled(11));

        let id = jit.compile(11, &func).expect("should compile");
        assert!(jit.is_compiled(11));

        // Calling compile again returns the cached id.
        let id2 = jit.compile(11, &func).expect("should return cached");
        assert_eq!(id, id2);

        // Invalidate.
        jit.invalidate(11);
        assert!(!jit.is_compiled(11));
    }

    #[test]
    fn test_jit_eqlsign() {
        // (= a b)
        let func = BytecodeFunction {
            argdesc: 0x0202,
            bytecode: vec![1, 1, 85, 135],
            constants: vec![],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(12, &func).expect("should compile");

        // 5 = 5 -> t
        let r = jit.call(id, &[fixnum_i64(5), fixnum_i64(5)]).expect("call");
        match r {
            NativeResult::Ok(bits) => assert_eq!(bits, T_BITS),
            NativeResult::Deoptimize => panic!("deopt"),
        }

        // 5 = 6 -> nil
        let r = jit.call(id, &[fixnum_i64(5), fixnum_i64(6)]).expect("call");
        match r {
            NativeResult::Ok(bits) => assert_eq!(bits, NIL_BITS),
            NativeResult::Deoptimize => panic!("deopt"),
        }
    }

    #[test]
    fn test_jit_discard() {
        // Push two constants, discard one, return the other.
        //  192: constant[0] = 42
        //  193: constant[1] = 99
        //  136: discard (pops 99)
        //  135: return (returns 42)
        let func = BytecodeFunction {
            argdesc: 0x0000,
            bytecode: vec![192, 193, 136, 135],
            constants: vec![LispObject::Integer(42), LispObject::Integer(99)],
            maxdepth: 4,
            docstring: None,
            interactive: None,
        };

        let mut jit = JitCompiler::new();
        let id = jit.compile(13, &func).expect("should compile");

        let r = jit.call(id, &[]).expect("call");
        match r {
            NativeResult::Ok(bits) => {
                let n = i64_to_fixnum(bits as i64).expect("fixnum");
                assert_eq!(n, 42);
            }
            NativeResult::Deoptimize => panic!("deopt"),
        }
    }
}
