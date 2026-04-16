# 0.6.1

## Full Emacs Lisp Interpreter with Bytecode VM and Cranelift JIT

Initial release of the `gpui-elisp` crate as a comprehensive Emacs Lisp
interpreter targeting Emacs 30.x standard library compatibility.

### Reader

- Full Emacs Lisp syntax: S-expressions, dotted pairs `(a . b)`,
  backquote/unquote/splice (`` ` ``, `,`, `,@`), `#'` function shorthand
- Character literals: `?a`, `?\n`, `?\x41`, `?\M-\C-@` (meta/control modifiers)
- Number literals: integers, floats (`3.14`, `1e10`, `1.5e-3`), hex (`#xff`),
  octal (`#o77`), binary (`#b1010`), symbols-starting-with-digits (`1value`, `1+`)
- `#[arglist bytecode constants maxdepth]` bytecode function literals
- `.elc` file format: `#@` doc-string skip, `#$` file reference, octal string
  escapes (`\207`), raw control bytes
- `[vector]` literals as self-evaluating `LispObject::Vector`
- `;` line comments, `\`-escaped symbols (`\``, `\,`, `\,@`)
- Keyword symbols (`:test`, `:key`) are self-evaluating

### Evaluator

- ~60 special forms: `quote`, `if` (implicit progn for else), `setq` (multi-pair),
  `defun`, `defvar`/`defconst`, `defalias`/`defsubst`, `defmacro`/`macroexpand`,
  `let`/`let*` (bare symbol bindings), `progn`/`prog1`/`prog2`, `while`/`dolist`,
  `lambda`/`function`, `cond`, `and`/`or`/`when`/`unless`,
  `catch`/`throw`, `condition-case`/`signal`, `unwind-protect`
- Higher-order: `mapcar`, `mapc`, `mapconcat`, `funcall`, `apply` (variadic),
  `eval`, `sort` (with predicate function)
- Module system: `provide`, `featurep`, `require`
- Property lists: `put`, `get` (per-symbol plist table)
- String formatting: `format`/`message` with `%s`, `%d`, `%f`, `%c`, `%x`, `%o`,
  `%S`, `%%`, field width, zero-padding, left-alignment
- Regex: `string-match`/`string-match-p` with Emacs-to-Rust regex translation
- Symbol operations: `symbol-value`, `symbol-function`, `default-value`,
  `default-boundp`, `set-default`, `set`, `boundp`, `fboundp`, `intern`,
  `intern-soft`, `fset`, `make-symbol`
- I/O: `read-from-string`, `split-string`, `version-to-list`, `autoload`
- Hash tables: `make-hash-table` (with `:test`), `gethash`, `puthash`,
  `hash-table-p`, `hash-table-count`, `clrhash`
- Backquote expansion via `backquote.el` macro system (Lisp-2 aware dispatch)
- `error` function with format string substitution
- Stub support for ~40 editor/keymap/buffer functions for stdlib loading

### Primitives (89 builtin functions)

- Arithmetic: `+`, `-`, `*`, `/` (integer-preserving), `1+`, `1-`, `mod`, `abs`,
  `max`, `min`, `floor`, `ceiling`, `round`, `truncate`, `float`, `ash`,
  `logand`, `logior`, `lognot`, `/=`
- Comparison: `=` (exact IEEE 754), `<`, `>`, `<=`, `>=`
- List: `cons`, `car`, `cdr`, `list`, `length`, `nth`, `nthcdr`, `append`,
  `reverse`, `nreverse`, `nconc`, `member`, `memq`, `assoc`, `assq`, `delq`,
  `last`, `copy-sequence`, `make-list`, `cadr`, `cddr`, `caar`, `cdar`,
  `car-safe`, `cdr-safe`, `setcar`, `setcdr` (true in-place mutation)
- Type predicates: `atom`, `symbolp` (includes nil/t), `numberp`, `integerp`,
  `floatp`, `stringp`, `consp`, `listp`, `vectorp`, `functionp`, `subrp`,
  `zerop`, `natnump`, `null`, `not`, `boundp`, `fboundp`
- String: `string=`, `string<`, `concat` (strings only), `substring`,
  `string-to-number`, `number-to-string`, `make-string`, `prin1-to-string`
- Symbol: `symbol-name`, `eq` (atom identity), `equal` (deep equality)
- I/O: `princ`, `prin1`, `identity`, `ignore`, `type-of`

### Error System

- `catch`/`throw` non-local exits with tag matching and propagation
- `condition-case`/`signal` with Emacs error symbols (`void-function`,
  `void-variable`, `wrong-type-argument`, `arith-error`, `invalid-read-syntax`)
- `unwind-protect` with unconditional cleanup
- `error` function with format-string substitution
- `ElispError::Throw`/`Signal` boxed variants for efficient Result types

### Bytecode VM (~80 opcodes)

- Stack: `stack-ref` (0-7), `dup`, `discard`, `discardN`, `stack-set`
- Variables: `varref`, `varset`, `varbind`, `unbind` (with dynamic binding stack)
- Function calls: `call` (0-7 args), delegates to `call_function` for lambdas,
  primitives, bytecode functions, and symbol indirection
- Arithmetic: `add1`, `sub1`, `plus`, `diff`, `mult`, `quo`, `rem`, `negate`
  (all with exact integer semantics and proper type errors)
- Comparison: `eqlsign`, `gtr`, `lss`, `leq`, `geq` (exact, type-checked)
- List: `car`, `cdr`, `cons`, `list1`-`list4`, `length`, `nth`, `nthcdr`,
  `setcar`, `setcdr` (mutation), `car-safe`, `cdr-safe`, `nconc`, `memq`,
  `member`, `assq`, `eq`, `equal`, `not`, `symbolp`, `consp`, `stringp`,
  `listp`, `numberp`, `integerp`
- String: `concat2`-`concat4`, `concatN`, `string=`, `string<`, `substring`
- Array: `aref`, `aset` (mutation via `Arc<Mutex<>>`)
- Control flow: `goto`, `goto-if-nil`, `goto-if-not-nil`, `goto-if-nil-else-pop`,
  `goto-if-not-nil-else-pop`, `return`
- Symbol: `symbol-value`, `symbol-function`, `set`, `fset`, `get` (plist query)
- Variadic: `listN`, `concatN`, `insertN`
- Constants: `constant[0..63]` (opcodes 192-255)

### Interior Mutability

- Cons cells: `Arc<Mutex<(LispObject, LispObject)>>` — `setcar`/`setcdr`
  mutate in place, `nconc` destructively modifies last cdr
- Vectors: `Arc<Mutex<Vec<LispObject>>>` — `aset` modifies elements in place
- Hash tables: `Arc<Mutex<LispHashTable>>` — `puthash` visible through
  original variable binding
- Manual `PartialEq` implementation for deep equality through locks

### Garbage Collector (skeleton)

- `Heap`: mark-and-sweep with adaptive threshold
- `Arena<T>`: bump allocator with free-list recycling for fixed-size objects
- `ConsCell`: `#[repr(C)]` with `GcHeader` prefix and `u64` car/cdr
- `RootGuard`: RAII root stack management
- Stress-tested: 10K allocations, GC collects unreachable, roots survive

### NaN-boxed Value Type

- 64-bit `Copy` type using negative quiet NaN space for tagging
- Immediate types: fixnums (48-bit signed), floats (raw IEEE 754),
  nil/t/unbound, characters, symbol IDs, subr indices
- GC pointers for heap-allocated objects
- `LispObject` <-> `Value` bridge for incremental migration

### Cranelift JIT (feature-gated: `jit`)

- Profiler: invocation counters with configurable compilation threshold
- Compiler: translates 17 bytecode opcodes to Cranelift IR
  - Fast-path fixnum arithmetic with NaN-box tag guards
  - Sign-correct 48-bit payload extraction and re-tagging
  - Pre-scan for jump targets -> Cranelift basic blocks
  - 0-4 arg function pointer trampolines
- Graceful fallback: unsupported opcodes -> VM execution
- Wired into eval dispatch: hot functions auto-compile, deoptimize falls
  through to bytecode VM

### Standard Library Compatibility

- `debug-early.el`: 100% (5/5 forms)
- `byte-run.el`: 100% (~55 forms)
- `backquote.el`: 100% (~15 forms)
- `subr.el`: 99.8% (493/494 forms)
- `subr.elc`: 636 forms parsed
- End-to-end: Emacs 30.2 compiles functions, `.elc` loads, VM executes
  (including recursive factorial)

### Known Limitations

- No real obarray / symbol interning (symbols are strings; deferred to GC
  migration)
- `Rc<RefCell<>>` mutation model, not true GC — cycles will leak
- Lexical binding only (no dynamic binding flag per file)
- `match-data` / regex capture groups not tracked
- Many buffer/editor opcodes are stubs in the VM (work via EditorCallbacks
  in interpreter mode)
- `apply` with >2 args: variadic support in progress
- `split-string` separator argument: in progress
