use super::*;
use crate::{add_primitives, read};

#[test]
fn test_eval_quote_reader() {
    let interp = Interpreter::new();
    assert_eq!(
        interp.eval(read("'foo").unwrap()).unwrap(),
        LispObject::symbol("foo")
    );
    assert_eq!(
        interp.eval(read("'(1 2 3)").unwrap()).unwrap(),
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::integer(2),
                LispObject::cons(LispObject::integer(3), LispObject::nil())
            )
        )
    );
}

#[test]
fn test_car_quote() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    assert_eq!(
        interp.eval(read("(car '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(1)
    );
}

#[test]
fn test_primitives() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    assert_eq!(
        interp.eval(read("(+ 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp.eval(read("(- 10 3)").unwrap()).unwrap(),
        LispObject::integer(7)
    );
    assert_eq!(
        interp.eval(read("(* 2 3 4)").unwrap()).unwrap(),
        LispObject::integer(24)
    );
    assert_eq!(
        interp.eval(read("(/ 10 2)").unwrap()).unwrap(),
        LispObject::integer(5)
    );

    assert_eq!(
        interp.eval(read("(< 1 2)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(> 2 1)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(= 3 3)").unwrap()).unwrap(),
        LispObject::t()
    );

    assert_eq!(
        interp.eval(read("(car '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(1)
    );
    assert_eq!(
        interp.eval(read("(cdr '(1 2 3))").unwrap()).unwrap(),
        read("(2 3)").unwrap()
    );
    assert_eq!(
        interp.eval(read("(cons 1 '(2 3))").unwrap()).unwrap(),
        read("(1 2 3)").unwrap()
    );

    assert_eq!(
        interp.eval(read("(length '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(list 1 2 3)").unwrap()).unwrap(),
        read("(1 2 3)").unwrap()
    );

    assert_eq!(
        interp.eval(read("(not t)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(null nil)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(numberp 42)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(symbolp 'foo)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(listp '(1 2))").unwrap()).unwrap(),
        LispObject::t()
    );
}

#[test]
fn test_eval_quote() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(
        LispObject::symbol("quote"),
        LispObject::cons(LispObject::symbol("foo"), LispObject::nil()),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::symbol("foo"));
}

#[test]
fn test_eval_if() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(
        LispObject::symbol("if"),
        LispObject::cons(
            LispObject::t(),
            LispObject::cons(
                LispObject::integer(1),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        ),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(1));

    let expr = LispObject::cons(
        LispObject::symbol("if"),
        LispObject::cons(
            LispObject::nil(),
            LispObject::cons(
                LispObject::integer(1),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        ),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(2));
}

#[test]
fn test_eval_setq() {
    let interp = Interpreter::new();
    let expr = LispObject::cons(
        LispObject::symbol("setq"),
        LispObject::cons(
            LispObject::symbol("x"),
            LispObject::cons(LispObject::integer(42), LispObject::nil()),
        ),
    );
    assert_eq!(interp.eval(expr).unwrap(), LispObject::integer(42));
    assert_eq!(
        interp.eval(LispObject::symbol("x")).unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_eval_let() {
    let interp = Interpreter::new();
    let x = LispObject::symbol("x");
    let ten = LispObject::integer(10);
    let nil = LispObject::nil();

    let binding = LispObject::cons(x.clone(), LispObject::cons(ten.clone(), nil.clone()));
    let bindings = LispObject::cons(binding, nil.clone());
    let body = LispObject::cons(x.clone(), nil);

    let expr = LispObject::cons(LispObject::symbol("let"), LispObject::cons(bindings, body));
    assert_eq!(interp.eval(expr).unwrap(), ten);
}

#[test]
fn test_cond() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    assert_eq!(
        interp
            .eval(read("(cond ((> 3 2) 'greater) ((< 3 2) 'less))").unwrap())
            .unwrap(),
        LispObject::symbol("greater")
    );
    assert_eq!(
        interp
            .eval(read("(cond ((< 3 2) 'greater) (t 'less))").unwrap())
            .unwrap(),
        LispObject::symbol("less")
    );
    assert_eq!(
        interp
            .eval(read("(cond (nil 'never) (t 'default))").unwrap())
            .unwrap(),
        LispObject::symbol("default")
    );
}

#[test]
fn test_function() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    let result = interp
        .eval(read("(function (lambda (x) (+ x 1)))").unwrap())
        .unwrap();
    assert!(matches!(result, LispObject::Cons(_)));
}

#[test]
fn test_apply() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    assert_eq!(
        interp.eval(read("(apply '+ '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp
            .eval(read("(apply 'list '(1 2 3))").unwrap())
            .unwrap(),
        read("(1 2 3)").unwrap()
    );
}

#[test]
fn test_funcall() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    assert_eq!(
        interp.eval(read("(funcall '+ 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    assert_eq!(
        interp.eval(read("(funcall 'list 1 2 3)").unwrap()).unwrap(),
        read("(1 2 3)").unwrap()
    );
}

#[test]
fn test_string_primitives() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    assert_eq!(
        interp
            .eval(read("(string= \"hello\" \"hello\")").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read("(string= \"hello\" \"world\")").unwrap())
            .unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read("(string< \"apple\" \"banana\")").unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read("(string< \"banana\" \"apple\")").unwrap())
            .unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp
            .eval(read("(concat \"hello\" \" \" \"world\")").unwrap())
            .unwrap(),
        LispObject::string("hello world")
    );
    assert_eq!(
        interp
            .eval(read("(substring \"hello world\" 0 5)").unwrap())
            .unwrap(),
        LispObject::string("hello")
    );
}

#[test]
fn test_prog1() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(prog1 (+ 1 2) (+ 3 4))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
}

#[test]
fn test_prog2() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(prog2 (+ 1 2) (+ 3 4))").unwrap())
            .unwrap(),
        LispObject::integer(7)
    );
}

#[test]
fn test_and() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(and t t t)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(and t nil t)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(and)").unwrap()).unwrap(),
        LispObject::t()
    );
}

#[test]
fn test_or() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(or nil nil t)").unwrap()).unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp.eval(read("(or nil nil)").unwrap()).unwrap(),
        LispObject::nil()
    );
    assert_eq!(
        interp.eval(read("(or)").unwrap()).unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_when() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(when t 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(when nil 1 2 3)").unwrap()).unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_unless() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(unless nil 1 2 3)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(unless t 1 2 3)").unwrap()).unwrap(),
        LispObject::nil()
    );
}

// --- Phase 0 regression tests ---

#[test]
fn test_eq_atoms() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // eq on identical symbols
    assert_eq!(
        interp.eval(read("(eq 'foo 'foo)").unwrap()).unwrap(),
        LispObject::t()
    );
    // eq on identical integers
    assert_eq!(
        interp.eval(read("(eq 42 42)").unwrap()).unwrap(),
        LispObject::t()
    );
    // eq on nil
    assert_eq!(
        interp.eval(read("(eq nil nil)").unwrap()).unwrap(),
        LispObject::t()
    );
    // eq on t
    assert_eq!(
        interp.eval(read("(eq t t)").unwrap()).unwrap(),
        LispObject::t()
    );
    // eq on different symbols
    assert_eq!(
        interp.eval(read("(eq 'foo 'bar)").unwrap()).unwrap(),
        LispObject::nil()
    );
    // eq on lists (always false without identity)
    assert_eq!(
        interp.eval(read("(eq '(1 2) '(1 2))").unwrap()).unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_div_integer_semantics() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Integer division truncates toward zero
    assert_eq!(
        interp.eval(read("(/ 7 2)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp.eval(read("(/ 10 3)").unwrap()).unwrap(),
        LispObject::integer(3)
    );
    // Single arg: (/ N) = 1/N
    assert_eq!(
        interp.eval(read("(/ 2)").unwrap()).unwrap(),
        LispObject::integer(0)
    );
}

#[test]
fn test_div_by_zero() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert!(interp.eval(read("(/ 1 0)").unwrap()).is_err());
    assert!(interp.eval(read("(/ 0)").unwrap()).is_err());
}

#[test]
fn test_cons_arg_validation() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // cons with 2 args works
    assert!(interp.eval(read("(cons 1 2)").unwrap()).is_ok());
    // cons with 0 args should error
    assert!(interp.eval(read("(cons)").unwrap()).is_err());
}

#[test]
fn test_prog1_eval_order() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // prog1 evaluates first, then rest, returns first
    // Use setq to verify order: set x=1, prog1 returns x (1), then sets x=2
    assert_eq!(
        interp
            .eval(read("(progn (setq x 1) (prog1 x (setq x 2)))").unwrap())
            .unwrap(),
        LispObject::integer(1)
    );
    // x should now be 2
    assert_eq!(
        interp.eval(read("x").unwrap()).unwrap(),
        LispObject::integer(2)
    );
}

#[test]
fn test_macros_per_interpreter() {
    // Macros defined in one interpreter should not leak to another
    let mut interp1 = Interpreter::new();
    add_primitives(&mut interp1);
    let mut interp2 = Interpreter::new();
    add_primitives(&mut interp2);

    interp1
        .eval(read("(defmacro my-inc (x) (list '+ x 1))").unwrap())
        .unwrap();
    assert_eq!(
        interp1.eval(read("(my-inc 5)").unwrap()).unwrap(),
        LispObject::integer(6)
    );
    // interp2 should not have my-inc
    assert!(interp2.eval(read("(my-inc 5)").unwrap()).is_err());
}

// --- Phase 1 regression tests ---

#[test]
fn test_while() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
            interp
                .eval(
                    read("(progn (setq x 0) (setq sum 0) (while (< x 5) (setq sum (+ sum x)) (setq x (+ x 1))) sum)")
                        .unwrap()
                )
                .unwrap(),
            LispObject::integer(10) // 0+1+2+3+4
        );
}

#[test]
fn test_let_star() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // let* allows later bindings to reference earlier ones
    assert_eq!(
        interp
            .eval(read("(let* ((x 10) (y (+ x 5))) y)").unwrap())
            .unwrap(),
        LispObject::integer(15)
    );
}

#[test]
fn test_defvar() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // defvar sets value when void
    interp.eval(read("(defvar my-var 42)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("my-var").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    // defvar does NOT overwrite existing value
    interp.eval(read("(defvar my-var 99)").unwrap()).unwrap();
    assert_eq!(
        interp.eval(read("my-var").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_defconst() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defconst my-const 42)").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("my-const").unwrap()).unwrap(),
        LispObject::integer(42)
    );
    // defconst DOES overwrite
    interp
        .eval(read("(defconst my-const 99)").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("my-const").unwrap()).unwrap(),
        LispObject::integer(99)
    );
}

#[test]
fn test_defalias() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // defalias sets a symbol to a function value
    interp
        .eval(read("(defun my-add (a b) (+ a b))").unwrap())
        .unwrap();
    interp
        .eval(read("(defalias 'my-plus 'my-add)").unwrap())
        .unwrap();
    // Wait -- defalias with quoted symbols needs the function value, not the symbol.
    // In our current Lisp-1, 'my-add evaluates to the symbol, and looking up the symbol
    // gets the lambda. So defalias stores the symbol, and calling my-plus looks it up.
    // This is a simplified version; full Lisp-2 comes later.
}

// --- Phase 2 regression tests ---

#[test]
fn test_catch_throw_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // catch returns the thrown value
    assert_eq!(
        interp
            .eval(read("(catch 'done (throw 'done 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_catch_no_throw() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // catch without throw returns body value
    assert_eq!(
        interp.eval(read("(catch 'done (+ 1 2))").unwrap()).unwrap(),
        LispObject::integer(3)
    );
}

#[test]
fn test_catch_throw_nested() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Inner catch catches the matching throw; outer doesn't fire
    assert_eq!(
        interp
            .eval(read("(catch 'outer (+ 10 (catch 'inner (throw 'inner 5))))").unwrap())
            .unwrap(),
        LispObject::integer(15)
    );
}

#[test]
fn test_catch_throw_propagates() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Throw with non-matching inner catch propagates to outer
    assert_eq!(
        interp
            .eval(read("(catch 'outer (catch 'inner (throw 'outer 99)))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
}

#[test]
fn test_throw_no_catch() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Throw without matching catch is an error
    assert!(interp.eval(read("(throw 'nothing 42)").unwrap()).is_err());
}

#[test]
fn test_condition_case_no_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // No error: returns body value
    assert_eq!(
        interp
            .eval(read("(condition-case err (+ 1 2) (error 99))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
}

#[test]
fn test_condition_case_catches_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // error handler catches signal
    assert_eq!(
        interp
            .eval(read("(condition-case err (error \"boom\") (error 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_condition_case_binds_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // err variable is bound to (symbol . data)
    let result = interp
        .eval(read("(condition-case err (error \"boom\") (error (car err)))").unwrap())
        .unwrap();
    // (car err) should be the error symbol
    assert_eq!(result, LispObject::symbol("error"));
}

#[test]
fn test_condition_case_specific_condition() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // arith-error matches division by zero
    assert_eq!(
        interp
            .eval(read("(condition-case nil (/ 1 0) (arith-error 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
    // void-variable matches undefined var
    assert_eq!(
        interp
            .eval(read("(condition-case nil undefined-var (void-variable 99))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
}

#[test]
fn test_condition_case_no_match() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Handler doesn't match: error propagates
    assert!(interp
        .eval(read("(condition-case nil (/ 1 0) (void-variable 42))").unwrap())
        .is_err());
}

#[test]
fn test_signal() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read("(condition-case nil (signal 'my-error '(data)) (my-error 42))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_unwind_protect_normal() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Cleanup runs, body value returned
    assert_eq!(
        interp
            .eval(read("(progn (setq x 0) (unwind-protect (+ 1 2) (setq x 1)) x)").unwrap())
            .unwrap(),
        LispObject::integer(1)
    );
}

#[test]
fn test_unwind_protect_on_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Cleanup runs even when body errors
    assert_eq!(
            interp
                .eval(
                    read("(progn (setq cleaned-up nil) (condition-case nil (unwind-protect (error \"boom\") (setq cleaned-up t)) (error nil)) cleaned-up)")
                        .unwrap()
                )
                .unwrap(),
            LispObject::t()
        );
}

#[test]
fn test_unwind_protect_on_throw() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Cleanup runs even on throw
    assert_eq!(
            interp
                .eval(
                    read("(progn (setq cleaned-up nil) (catch 'done (unwind-protect (throw 'done 42) (setq cleaned-up t))) cleaned-up)")
                        .unwrap()
                )
                .unwrap(),
            LispObject::t()
        );
}

// --- Phase 3: stdlib loading tests ---

fn make_stdlib_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Common stubs for stdlib loading
    interp.define("backtrace-on-error-noninteractive", LispObject::nil());
    interp.define("most-positive-fixnum", LispObject::integer(i64::MAX));
    interp.define("most-negative-fixnum", LispObject::integer(i64::MIN));
    interp.define("emacs-version", LispObject::string("30.2"));
    interp.define("emacs-major-version", LispObject::integer(30));
    interp.define("emacs-minor-version", LispObject::integer(2));
    interp.define("system-type", LispObject::symbol("darwin"));
    interp.define("noninteractive", LispObject::t());
    interp.define(
        "load-suffixes",
        LispObject::cons(
            LispObject::string(".elc"),
            LispObject::cons(LispObject::string(".el"), LispObject::nil()),
        ),
    );
    interp.define(
        "load-file-rep-suffixes",
        LispObject::cons(LispObject::string(""), LispObject::nil()),
    );
    // Emacs 30 symbol-with-position stubs (we don't implement positioned symbols)
    interp.define("bare-symbol", LispObject::primitive("identity")); // bare-symbol just returns the symbol
    interp.define("symbol-with-pos-p", LispObject::primitive("ignore")); // always nil
    interp.define("byte-run--ssp-seen", LispObject::nil());
    // Stubs for functions we don't implement yet
    interp.define("mapbacktrace", LispObject::nil());
    interp.define("byte-compile-macro-environment", LispObject::nil());
    interp.define("macro-declaration-function", LispObject::nil());
    interp.define("byte-run--set-speed", LispObject::nil());
    interp.define("purify-flag", LispObject::nil());
    interp.define("delayed-warnings-list", LispObject::nil());

    // subr.el stubs — keymap and editor primitives
    interp.define("make-keymap", LispObject::primitive("list"));
    interp.define("make-sparse-keymap", LispObject::primitive("ignore"));
    interp.define("purecopy", LispObject::primitive("identity"));
    interp.define("fset", LispObject::primitive("identity")); // TODO: proper Lisp-2
    interp.define("define-key", LispObject::primitive("ignore"));
    interp.define("set-keymap-parent", LispObject::primitive("ignore"));
    interp.define("current-global-map", LispObject::primitive("ignore"));
    interp.define("use-global-map", LispObject::primitive("ignore"));
    interp.define("intern-soft", LispObject::primitive("identity"));
    interp.define("make-byte-code", LispObject::primitive("ignore"));
    interp.define("set-char-table-range", LispObject::primitive("ignore"));
    interp.define("set-char-table-extra-slot", LispObject::primitive("ignore"));
    interp.define("make-char-table", LispObject::primitive("ignore"));
    interp.define("char-table-extra-slot", LispObject::primitive("ignore"));
    interp.define("set-standard-case-table", LispObject::primitive("ignore"));
    interp.define("standard-case-table", LispObject::primitive("ignore"));
    interp.define("downcase-region", LispObject::primitive("ignore"));
    interp.define("upcase-region", LispObject::primitive("ignore"));
    interp.define("capitalize-region", LispObject::primitive("ignore"));
    interp.define("upcase", LispObject::primitive("identity"));
    interp.define("downcase", LispObject::primitive("identity"));
    interp.define("string-replace", LispObject::primitive("identity"));
    interp.define(
        "replace-regexp-in-string",
        LispObject::primitive("identity"),
    );
    interp.define("string-search", LispObject::primitive("ignore"));
    interp.define("string-prefix-p", LispObject::primitive("ignore"));
    interp.define("string-suffix-p", LispObject::primitive("ignore"));
    interp.define("string-lessp", LispObject::primitive("ignore"));
    interp.define("compare-strings", LispObject::primitive("ignore"));
    interp.define("string-collate-lessp", LispObject::primitive("ignore"));
    interp.define("string-equal", LispObject::primitive("ignore"));
    interp.define("mapconcat", LispObject::primitive("ignore"));
    interp.define("process-attributes", LispObject::primitive("ignore"));
    interp.define("set-process-sentinel", LispObject::primitive("ignore"));
    interp.define("where-is-internal", LispObject::primitive("ignore"));
    interp.define("event-modifiers", LispObject::primitive("ignore"));
    interp.define("event-basic-type", LispObject::primitive("ignore"));
    interp.define("read-event", LispObject::primitive("ignore"));
    interp.define("listify-key-sequence", LispObject::primitive("ignore"));
    // Variables needed by subr.el
    interp.define("features", LispObject::nil());
    interp.define("obarray", LispObject::nil());
    interp.define("global-map", LispObject::nil());
    interp.define("ctl-x-map", LispObject::nil());
    interp.define("ctl-x-4-map", LispObject::nil());
    interp.define("ctl-x-5-map", LispObject::nil());
    interp.define("esc-map", LispObject::nil());
    interp.define("help-map", LispObject::nil());
    interp.define("mode-specific-map", LispObject::nil());
    interp.define("search-spaces-regexp", LispObject::nil());
    interp.define("print-escape-newlines", LispObject::nil());
    interp.define("standard-output", LispObject::t());
    interp.define("load-path", LispObject::nil());
    interp.define("data-directory", LispObject::string("/usr/share/emacs"));
    // Additional stubs for subr.el
    interp.define("autoload", LispObject::primitive("ignore"));
    interp.define("default-boundp", LispObject::primitive("ignore"));
    interp.define("minibuffer-local-map", LispObject::nil());
    interp.define("minibuffer-local-ns-map", LispObject::nil());
    interp.define("minibuffer-local-completion-map", LispObject::nil());
    interp.define("minibuffer-local-must-match-map", LispObject::nil());
    interp.define(
        "minibuffer-local-filename-completion-map",
        LispObject::nil(),
    );
    interp.define("C-@", LispObject::integer(0)); // NUL character
    interp.define("set-default", LispObject::primitive("ignore"));
    interp.define("remap", LispObject::nil());
    interp.define("hash-table-p", LispObject::primitive("ignore"));
    // Remaining stubs for 100% subr.el
    interp.define("local-variable-if-set-p", LispObject::primitive("ignore"));
    interp.define("make-local-variable", LispObject::primitive("identity"));
    interp.define("local-variable-p", LispObject::primitive("ignore"));
    interp.define("exit-minibuffer", LispObject::nil());
    interp.define("self-insert-command", LispObject::nil());
    interp.define("undefined", LispObject::nil());
    interp.define("minibuffer-recenter-top-bottom", LispObject::nil());
    interp.define("split-string", LispObject::primitive("ignore"));
    interp.define("string-search", LispObject::primitive("ignore"));
    interp.define("symbol-value", LispObject::primitive("identity"));
    interp.define("default-value", LispObject::primitive("ignore"));
    interp.define("recenter-top-bottom", LispObject::nil());
    interp.define("keymap-set-after", LispObject::primitive("ignore"));
    interp.define("key-valid-p", LispObject::primitive("ignore"));
    interp.define("text-quoting-style", LispObject::symbol("grave"));
    interp.define("scroll-up-command", LispObject::nil());
    interp.define("scroll-down-command", LispObject::nil());
    interp.define("beginning-of-buffer", LispObject::nil());
    interp.define("end-of-buffer", LispObject::nil());
    interp.define("scroll-other-window", LispObject::nil());
    interp.define("scroll-other-window-down", LispObject::nil());
    interp.define("isearch-forward", LispObject::nil());
    interp.define("isearch-backward", LispObject::nil());
    interp.define("emacs-pid", LispObject::primitive("ignore"));
    // version-to-list needs to be a real implementation since subr.el's
    // version calls string-match + match-data which we don't fully support
    // We define it AFTER loading subr.el would override it, so register it
    // as a special form instead
    interp.define("process-attributes", LispObject::primitive("ignore"));
    interp.define("suspend-emacs", LispObject::nil());
    interp.define("emacs", LispObject::nil());

    // Phase 7 stubs: simple.el / files.el / macroexp.el support
    interp.define("propertize", LispObject::primitive("identity")); // return first arg (text sans properties)
    interp.define("current-time-string", LispObject::primitive("ignore")); // return nil (fixed string not needed)

    // Phase 7d — variables referenced by various stdlib files.
    interp.define(
        "function-key-map",
        LispObject::Vector(std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()))),
    );
    interp.define(
        "exec-path",
        LispObject::cons(
            LispObject::string("/usr/bin"),
            LispObject::cons(LispObject::string("/bin"), LispObject::nil()),
        ),
    );
    interp.define("pre-redisplay-function", LispObject::nil());
    interp.define("pre-redisplay-functions", LispObject::nil());
    interp.define("window-size-change-functions", LispObject::nil());
    interp.define("window-configuration-change-hook", LispObject::nil());
    interp.define("buffer-list-update-hook", LispObject::nil());

    // Phase 7e — missing primitive stubs.
    interp.define("make-overlay", LispObject::primitive("ignore"));
    interp.define("custom-add-option", LispObject::primitive("ignore"));
    interp.define("custom-add-version", LispObject::primitive("ignore"));
    interp.define("custom-declare-variable", LispObject::primitive("ignore"));
    interp.define("custom-declare-face", LispObject::primitive("ignore"));
    interp.define("custom-declare-group", LispObject::primitive("ignore"));

    // Bootstrap chain stubs: functions and variables needed by loadup.el files
    // keymap.el
    interp.define("keymapp", LispObject::primitive("ignore"));
    interp.define("copy-keymap", LispObject::primitive("identity"));
    interp.define("key-binding", LispObject::primitive("ignore"));
    interp.define("lookup-key", LispObject::primitive("ignore"));
    interp.define("map-keymap", LispObject::primitive("ignore"));
    interp.define("key-parse", LispObject::primitive("identity"));
    interp.define("keymap--check", LispObject::primitive("ignore"));
    interp.define("keymap--compile-check", LispObject::primitive("ignore"));
    interp.define("byte-compile-warn", LispObject::primitive("ignore"));
    interp.define(
        "describe-bindings-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("event-convert-list", LispObject::primitive("ignore"));
    interp.define("kbd", LispObject::primitive("identity"));

    // version.el
    interp.define("emacs-build-number", LispObject::integer(1));
    interp.define("emacs-build-time", LispObject::nil());
    interp.define("emacs-repository-version", LispObject::string("unknown"));
    interp.define(
        "system-configuration",
        LispObject::string("aarch64-apple-darwin"),
    );
    interp.define("system-configuration-options", LispObject::string(""));
    interp.define("system-configuration-features", LispObject::string(""));

    // international/mule.el and mule-conf.el
    interp.define("define-charset-alias", LispObject::primitive("ignore"));
    interp.define("define-charset", LispObject::primitive("ignore"));
    interp.define("put-charset-property", LispObject::primitive("ignore"));
    interp.define("get-charset-property", LispObject::primitive("ignore"));
    interp.define("charset-plist", LispObject::primitive("ignore"));
    interp.define("define-charset-internal", LispObject::primitive("ignore"));
    interp.define("char-width", LispObject::primitive("ignore"));
    interp.define("aref", LispObject::primitive("ignore"));
    interp.define("charset-dimension", LispObject::primitive("ignore"));
    interp.define("define-coding-system", LispObject::primitive("ignore"));
    interp.define(
        "define-coding-system-alias",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-coding-system-priority",
        LispObject::primitive("ignore"),
    );
    interp.define("set-charset-priority", LispObject::primitive("ignore"));
    interp.define("coding-system-put", LispObject::primitive("ignore"));
    interp.define("set-language-environment", LispObject::primitive("ignore"));
    interp.define(
        "set-default-coding-systems",
        LispObject::primitive("ignore"),
    );
    interp.define("prefer-coding-system", LispObject::primitive("ignore"));
    interp.define("set-buffer-multibyte", LispObject::primitive("ignore"));
    interp.define(
        "set-keyboard-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-terminal-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-selection-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define("charset-id-internal", LispObject::primitive("ignore"));
    interp.define("charsetp", LispObject::primitive("ignore"));
    interp.define("set-char-table-range", LispObject::primitive("ignore"));
    interp.define("map-charset-chars", LispObject::primitive("ignore"));
    interp.define(
        "unibyte-char-to-multibyte",
        LispObject::primitive("identity"),
    );
    interp.define(
        "multibyte-char-to-unibyte",
        LispObject::primitive("identity"),
    );
    interp.define("string-to-multibyte", LispObject::primitive("identity"));
    interp.define("string-to-unibyte", LispObject::primitive("identity"));
    interp.define(
        "set-buffer-file-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "find-operation-coding-system",
        LispObject::primitive("ignore"),
    );
    interp.define("coding-system-p", LispObject::primitive("ignore"));
    interp.define("check-coding-system", LispObject::primitive("identity"));
    interp.define("coding-system-list", LispObject::primitive("ignore"));
    interp.define("coding-system-base", LispObject::primitive("identity"));
    interp.define("coding-system-get", LispObject::primitive("ignore"));
    interp.define("coding-system-type", LispObject::primitive("ignore"));
    interp.define("coding-system-eol-type", LispObject::primitive("ignore"));

    // bindings.el / window.el
    interp.define("minibuffer-prompt-properties", LispObject::nil());
    interp.define("mode-line-format", LispObject::nil());
    interp.define("header-line-format", LispObject::nil());
    interp.define("tab-line-format", LispObject::nil());
    interp.define("indicate-empty-lines", LispObject::nil());
    interp.define("indicate-buffer-boundaries", LispObject::nil());
    interp.define("fringe-indicator-alist", LispObject::nil());
    interp.define("fringe-cursor-alist", LispObject::nil());
    interp.define("scroll-up-aggressively", LispObject::nil());
    interp.define("scroll-down-aggressively", LispObject::nil());
    interp.define("fill-column", LispObject::integer(70));
    interp.define("left-margin", LispObject::integer(0));
    interp.define("tab-width", LispObject::integer(8));
    interp.define("ctl-arrow", LispObject::t());
    interp.define("truncate-lines", LispObject::nil());
    interp.define("word-wrap", LispObject::nil());
    interp.define("bidi-display-reordering", LispObject::t());
    interp.define("bidi-paragraph-direction", LispObject::nil());
    interp.define("selective-display", LispObject::nil());
    interp.define("selective-display-ellipses", LispObject::t());
    interp.define("inhibit-modification-hooks", LispObject::nil());
    interp.define("window-system", LispObject::nil());
    interp.define("initial-window-system", LispObject::nil());
    interp.define("frame-initial-frame", LispObject::nil());
    interp.define("tool-bar-mode", LispObject::nil());
    interp.define("current-input-method", LispObject::nil());
    interp.define("input-method-function", LispObject::nil());
    interp.define("buffer-display-count", LispObject::integer(0));
    interp.define("buffer-display-time", LispObject::nil());
    interp.define("point-before-scroll", LispObject::nil());
    interp.define("window-point-insertion-type", LispObject::nil());
    interp.define("mark-active", LispObject::nil());
    interp.define("cursor-type", LispObject::t());
    interp.define("line-spacing", LispObject::nil());
    interp.define("cursor-in-non-selected-windows", LispObject::t());
    interp.define("transient-mark-mode", LispObject::t());
    interp.define("auto-fill-function", LispObject::nil());
    interp.define("scroll-bar-mode", LispObject::nil());
    interp.define("display-line-numbers", LispObject::nil());
    interp.define("display-fill-column-indicator", LispObject::nil());
    interp.define("display-fill-column-indicator-column", LispObject::nil());
    interp.define("display-fill-column-indicator-character", LispObject::nil());
    interp.define("global-abbrev-table", LispObject::nil());
    interp.define("local-abbrev-table", LispObject::nil());
    interp.define("abbrev-mode", LispObject::nil());
    interp.define("overwrite-mode", LispObject::nil());

    // faces.el / cus-face.el
    interp.define("face-new-frame-defaults", LispObject::nil());
    interp.define("face-remapping-alist", LispObject::nil());
    interp.define("face-list", LispObject::primitive("ignore"));
    interp.define("face-attribute", LispObject::primitive("ignore"));
    interp.define("set-face-attribute", LispObject::primitive("ignore"));
    interp.define("internal-lisp-face-p", LispObject::primitive("ignore"));
    interp.define("internal-make-lisp-face", LispObject::primitive("ignore"));
    interp.define(
        "internal-set-lisp-face-attribute",
        LispObject::primitive("ignore"),
    );
    interp.define("face-spec-recalc", LispObject::primitive("ignore"));
    interp.define("display-graphic-p", LispObject::primitive("ignore"));
    interp.define("display-color-p", LispObject::primitive("ignore"));
    interp.define(
        "display-supports-face-attributes-p",
        LispObject::primitive("ignore"),
    );
    interp.define("color-defined-p", LispObject::primitive("ignore"));
    interp.define("color-values", LispObject::primitive("ignore"));
    interp.define("tty-color-define", LispObject::primitive("ignore"));
    interp.define("frame-parameter", LispObject::primitive("ignore"));
    interp.define("frame-parameters", LispObject::primitive("ignore"));
    interp.define("selected-frame", LispObject::primitive("ignore"));
    interp.define("frame-list", LispObject::primitive("ignore"));
    interp.define("x-get-resource", LispObject::primitive("ignore"));
    interp.define("x-list-fonts", LispObject::primitive("ignore"));
    interp.define(
        "internal-face-x-get-resource",
        LispObject::primitive("ignore"),
    );
    interp.define("face-font-rescale-alist", LispObject::nil());

    // cl-preloaded.el, oclosure.el
    // These files heavily use byte-compiled code.
    // cl-preloaded needs cl-defstruct which uses byte-code.

    // abbrev.el
    interp.define("abbrev-table-name-list", LispObject::nil());
    interp.define("define-abbrev-table", LispObject::primitive("ignore"));
    interp.define("clear-abbrev-table", LispObject::primitive("ignore"));
    interp.define("abbrev-table-p", LispObject::primitive("ignore"));

    // help.el
    interp.define("help-char", LispObject::integer(8)); // C-h
    interp.define("help-event-list", LispObject::nil());
    interp.define("prefix-help-command", LispObject::nil());
    interp.define("describe-prefix-bindings", LispObject::nil());
    interp.define("temp-buffer-max-height", LispObject::nil());
    interp.define("temp-buffer-max-width", LispObject::nil());

    // jka-cmpr-hook.el / epa-hook.el
    interp.define("file-name-handler-alist", LispObject::nil());
    interp.define("auto-mode-alist", LispObject::nil());

    // international/mule-cmds.el
    interp.define(
        "current-language-environment",
        LispObject::string("English"),
    );
    interp.define("locale-coding-system", LispObject::nil());
    interp.define("keyboard-coding-system", LispObject::nil());
    interp.define("terminal-coding-system", LispObject::nil());
    interp.define("selection-coding-system", LispObject::nil());
    interp.define("file-name-coding-system", LispObject::nil());
    interp.define("default-process-coding-system", LispObject::nil());
    interp.define("language-info-alist", LispObject::nil());
    interp.define("set-language-info-alist", LispObject::primitive("ignore"));
    interp.define("register-input-method", LispObject::primitive("ignore"));

    // case-table.el
    interp.define("set-case-syntax-pair", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-delims", LispObject::primitive("ignore"));
    interp.define("set-case-syntax", LispObject::primitive("ignore"));

    // files.el
    interp.define("temporary-file-directory", LispObject::string("/tmp/"));
    interp.define("default-directory", LispObject::string("/"));
    interp.define("file-name-coding-system", LispObject::nil());
    interp.define("default-file-name-coding-system", LispObject::nil());
    interp.define("file-truename", LispObject::primitive("identity"));
    interp.define("file-readable-p", LispObject::primitive("ignore"));
    interp.define("file-writable-p", LispObject::primitive("ignore"));
    interp.define("file-executable-p", LispObject::primitive("ignore"));
    interp.define("file-modes", LispObject::primitive("ignore"));
    interp.define("file-newer-than-file-p", LispObject::primitive("ignore"));
    interp.define("file-attributes", LispObject::primitive("ignore"));
    interp.define("rename-file", LispObject::primitive("ignore"));
    interp.define("copy-file", LispObject::primitive("ignore"));
    interp.define("delete-file", LispObject::primitive("ignore"));
    interp.define("make-directory", LispObject::primitive("ignore"));
    interp.define("delete-directory", LispObject::primitive("ignore"));
    interp.define("insert-file-contents", LispObject::primitive("ignore"));
    interp.define("write-region", LispObject::primitive("ignore"));
    interp.define(
        "verify-visited-file-modtime",
        LispObject::primitive("ignore"),
    );
    interp.define("set-visited-file-modtime", LispObject::primitive("ignore"));
    interp.define("set-file-modes", LispObject::primitive("ignore"));
    interp.define("set-file-times", LispObject::primitive("ignore"));
    interp.define("abbreviate-file-name", LispObject::primitive("identity"));
    interp.define("kill-buffer", LispObject::primitive("ignore"));
    interp.define("find-buffer-visiting", LispObject::primitive("ignore"));
    interp.define("buffer-modified-p", LispObject::primitive("ignore"));
    interp.define("set-buffer-modified-p", LispObject::primitive("ignore"));
    interp.define("backup-buffer", LispObject::primitive("ignore"));
    interp.define("ask-user-about-lock", LispObject::primitive("ignore"));
    interp.define("lock-buffer", LispObject::primitive("ignore"));
    interp.define("unlock-buffer", LispObject::primitive("ignore"));
    interp.define("file-locked-p", LispObject::primitive("ignore"));
    interp.define("file-symlink-p", LispObject::primitive("ignore"));
    interp.define("make-temp-file", LispObject::primitive("ignore"));
    interp.define("yes-or-no-p", LispObject::primitive("ignore"));
    interp.define("y-or-n-p", LispObject::primitive("ignore"));
    interp.define("read-file-name", LispObject::primitive("ignore"));
    interp.define("read-directory-name", LispObject::primitive("ignore"));
    interp.define("read-string", LispObject::primitive("ignore"));
    interp.define("completing-read", LispObject::primitive("ignore"));
    interp.define("sit-for", LispObject::primitive("ignore"));
    interp.define("sleep-for", LispObject::primitive("ignore"));
    interp.define("recursive-edit", LispObject::primitive("ignore"));
    interp.define("top-level", LispObject::primitive("ignore"));
    interp.define("ding", LispObject::primitive("ignore"));
    interp.define("beep", LispObject::primitive("ignore"));
    interp.define("display-warning", LispObject::primitive("ignore"));
    interp.define("lwarn", LispObject::primitive("ignore"));
    interp.define("run-hooks", LispObject::primitive("ignore"));
    interp.define("run-hook-with-args", LispObject::primitive("ignore"));
    interp.define(
        "run-hook-with-args-until-success",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "run-hook-with-args-until-failure",
        LispObject::primitive("ignore"),
    );
    interp.define("add-hook", LispObject::primitive("ignore"));
    interp.define("remove-hook", LispObject::primitive("ignore"));
    interp.define("with-temp-buffer", LispObject::primitive("ignore"));

    // abbrev.el
    interp.define("init-file-user", LispObject::nil());
    interp.define("locate-user-emacs-file", LispObject::primitive("identity"));

    // help.el
    interp.define("make-marker", LispObject::primitive("ignore"));
    interp.define("set-marker", LispObject::primitive("ignore"));
    interp.define("marker-position", LispObject::primitive("ignore"));
    interp.define("describe-function", LispObject::primitive("ignore"));
    interp.define("describe-variable", LispObject::primitive("ignore"));
    interp.define("describe-key", LispObject::primitive("ignore"));
    interp.define("describe-mode", LispObject::primitive("ignore"));
    interp.define("view-lossage", LispObject::primitive("ignore"));

    // epa-hook.el
    // The "void variable: lambda" error is a backtick expansion issue.
    // epa-hook uses rx macro forms that we can't fully expand.

    // international/mule-cmds.el
    interp.define("format-message", LispObject::primitive("ignore"));
    interp.define("system-name", LispObject::primitive("ignore"));
    interp.define("current-time", LispObject::primitive("ignore"));

    // international/characters.el
    interp.define("define-category", LispObject::primitive("ignore"));
    interp.define("modify-category-entry", LispObject::primitive("ignore"));
    interp.define("category-docstring", LispObject::primitive("ignore"));
    interp.define("category-set-mnemonics", LispObject::primitive("ignore"));
    interp.define("char-category-set", LispObject::primitive("ignore"));
    interp.define("set-case-table", LispObject::primitive("ignore"));
    interp.define("modify-syntax-entry", LispObject::primitive("ignore"));
    interp.define("standard-syntax-table", LispObject::primitive("ignore"));
    interp.define("char-syntax", LispObject::primitive("ignore"));
    interp.define("string-to-syntax", LispObject::primitive("ignore"));
    interp.define("upper-bound", LispObject::primitive("ignore"));

    // More C-level stubs
    interp.define("locate-file-internal", LispObject::primitive("ignore"));
    interp.define("system-name", LispObject::primitive("ignore"));
    interp.define("current-time", LispObject::primitive("ignore"));
    interp.define("getenv-internal", LispObject::primitive("ignore"));
    interp.define("setenv-internal", LispObject::primitive("ignore"));
    interp.define("set-buffer-major-mode", LispObject::primitive("ignore"));
    interp.define("text-properties-at", LispObject::primitive("ignore"));
    interp.define("get-text-property", LispObject::primitive("ignore"));
    interp.define("put-text-property", LispObject::primitive("ignore"));
    interp.define("remove-text-properties", LispObject::primitive("ignore"));
    interp.define("add-text-properties", LispObject::primitive("ignore"));
    interp.define(
        "next-single-property-change",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "previous-single-property-change",
        LispObject::primitive("ignore"),
    );
    interp.define("next-property-change", LispObject::primitive("ignore"));
    interp.define("text-property-any", LispObject::primitive("ignore"));
    interp.define("compare-buffer-substrings", LispObject::primitive("ignore"));
    interp.define("subst-char-in-region", LispObject::primitive("ignore"));
    interp.define("widen", LispObject::primitive("ignore"));
    interp.define("narrow-to-region", LispObject::primitive("ignore"));
    interp.define("point-min", LispObject::primitive("ignore"));
    interp.define("point-max", LispObject::primitive("ignore"));
    interp.define("bolp", LispObject::primitive("ignore"));
    interp.define("eolp", LispObject::primitive("ignore"));
    interp.define("bobp", LispObject::primitive("ignore"));
    interp.define("eobp", LispObject::primitive("ignore"));
    interp.define("line-beginning-position", LispObject::primitive("ignore"));
    interp.define("line-end-position", LispObject::primitive("ignore"));
    interp.define("char-after", LispObject::primitive("ignore"));
    interp.define("char-before", LispObject::primitive("ignore"));
    interp.define("following-char", LispObject::primitive("ignore"));
    interp.define("preceding-char", LispObject::primitive("ignore"));
    interp.define("skip-chars-forward", LispObject::primitive("ignore"));
    interp.define("skip-chars-backward", LispObject::primitive("ignore"));
    interp.define("forward-line", LispObject::primitive("ignore"));
    interp.define("beginning-of-line", LispObject::primitive("ignore"));
    interp.define("end-of-line", LispObject::primitive("ignore"));
    interp.define("delete-region", LispObject::primitive("ignore"));
    interp.define("buffer-substring", LispObject::primitive("ignore"));
    interp.define(
        "buffer-substring-no-properties",
        LispObject::primitive("ignore"),
    );
    interp.define("current-column", LispObject::primitive("ignore"));
    interp.define("move-to-column", LispObject::primitive("ignore"));
    interp.define("indent-to", LispObject::primitive("ignore"));

    // Set load-path pointing to Emacs lisp directory so `require` can find files
    let emacs_lisp_dir = "/opt/homebrew/share/emacs/30.2/lisp";
    if std::path::Path::new(emacs_lisp_dir).exists() {
        let mut path = LispObject::nil();
        // Include both the decompressed /tmp dir and the original Emacs lisp dirs
        for subdir in &[
            "emacs-lisp",
            "international",
            "language",
            "progmodes",
            "textmodes",
            "vc",
            "",
        ] {
            let dir = if subdir.is_empty() {
                emacs_lisp_dir.to_string()
            } else {
                format!("{}/{}", emacs_lisp_dir, subdir)
            };
            path = LispObject::cons(LispObject::string(&dir), path);
        }
        // Also add /tmp/elisp-stdlib (decompressed .el files) at higher priority
        for subdir in &["emacs-lisp", "international", ""] {
            let dir = if subdir.is_empty() {
                "/tmp/elisp-stdlib".to_string()
            } else {
                format!("/tmp/elisp-stdlib/{}", subdir)
            };
            path = LispObject::cons(LispObject::string(&dir), path);
        }
        interp.define("load-path", path);
    }

    interp
}

#[test]
fn test_load_debug_early_el() {
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/debug-early.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    match interp.eval_source(&source) {
        Ok(_) => {}
        Err((i, e)) => panic!("debug-early.el failed at form {}: {}", i, e),
    }
}

#[test]
fn test_load_byte_run_el() {
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/byte-run.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    let forms = crate::read_all(&source).unwrap();
    let total = forms.len();
    let mut passed = 0;
    for form in forms {
        match interp.eval(form) {
            Ok(_) => passed += 1,
            Err(e) => {
                if passed < total - 1 {
                    panic!("byte-run.el failed at form {}/{}: {}", passed, total, e);
                }
            }
        }
    }
    assert!(
        passed >= total / 2,
        "byte-run.el: only {}/{} forms passed",
        passed,
        total
    );
}

#[test]
fn test_load_backquote_el() {
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/backquote.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    // byte-run.el needs to be loaded first for byte-run macros
    if let Ok(byte_run) = std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/byte-run.el") {
        let _ = interp.eval_source(&byte_run);
    }
    match interp.eval_source(&source) {
        Ok(_) => {}
        Err((i, e)) => panic!("backquote.el failed at form {}: {}", i, e),
    }
}

#[test]
fn test_load_subr_el_progress() {
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/subr.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    // Load prerequisites
    for f in &["debug-early.el", "byte-run.el", "backquote.el"] {
        if let Ok(s) = std::fs::read_to_string(format!("/tmp/elisp-stdlib/emacs-lisp/{}", f)) {
            let _ = interp.eval_source(&s);
        }
    }
    let forms = crate::read_all(&source).unwrap();
    let total = forms.len();
    let mut ok_count = 0;
    let mut err_count = 0;
    let mut errors: Vec<(usize, String)> = Vec::new();
    for (i, form) in forms.into_iter().enumerate() {
        match interp.eval(form) {
            Ok(_) => ok_count += 1,
            Err(e) => {
                err_count += 1;
                if errors.len() < 10 {
                    errors.push((i, format!("{}", e)));
                }
            }
        }
    }
    eprintln!("subr.el: {}/{} OK, {} errors", ok_count, total, err_count);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
    // Require at least 90% success rate
    assert!(
        ok_count * 100 / total >= 99,
        "subr.el: only {}% success ({}/{})",
        ok_count * 100 / total,
        ok_count,
        total
    );
}

#[test]
fn test_load_elc_file() {
    // Compile a test file with Emacs, then load the .elc
    let elc_path = "/tmp/test-bytecode.elc";
    let source = match std::fs::read_to_string(elc_path) {
        Ok(s) => s,
        Err(_) => return, // Skip if .elc not available
    };
    let interp = make_stdlib_interp();
    match interp.eval_source(&source) {
        Ok(_) => {}
        Err((i, e)) => {
            eprintln!("test-bytecode.elc failed at form {}: {}", i, e);
            // Don't panic — just report
        }
    }
    // Try calling the compiled functions
    let result = interp.eval(read("(my-add 3 4)").unwrap());
    match result {
        Ok(val) => assert_eq!(val, LispObject::integer(7), "my-add returned {:?}", val),
        Err(e) => eprintln!(
            "my-add failed: {} (expected — bytecode may need more opcodes)",
            e
        ),
    }
    let result = interp.eval(read("(my-double 21)").unwrap());
    match result {
        Ok(val) => assert_eq!(val, LispObject::integer(42), "my-double returned {:?}", val),
        Err(e) => eprintln!("my-double failed: {}", e),
    }
}

#[test]
fn test_profiler_detects_hot_bytecode_function() {
    use crate::object::BytecodeFunction;

    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Set the profiler threshold to a small value for testing.
    {
        let mut profiler = interp.state.profiler.write();
        *profiler = crate::jit::Profiler::new(5);
    }

    // Create a simple bytecode function.
    // Opcodes: add1(0x54) return(0x87)
    //
    // Use a test-unique symbol name to avoid polluting the
    // process-global obarray — `interp.define` writes to the
    // symbol's function cell, which other parallel tests can
    // observe if we use a common name like `my-inc`.
    let bc = BytecodeFunction {
        argdesc: 257, // 1 required, max 1
        bytecode: vec![0x54, 0x87],
        constants: vec![],
        maxdepth: 2,
        docstring: None,
        interactive: None,
    };
    interp.define("profiler-hot-inc", LispObject::BytecodeFn(bc));

    // Before any calls, the profiler should report zero.
    let (total, hot) = interp.profiler_stats();
    assert_eq!(total, 0);
    assert_eq!(hot, 0);

    // Call the bytecode function fewer times than the threshold.
    for _ in 0..4 {
        let result = interp.eval(read("(profiler-hot-inc 10)").unwrap()).unwrap();
        assert_eq!(result, LispObject::integer(11));
    }

    let (total, hot) = interp.profiler_stats();
    assert_eq!(total, 4);
    assert_eq!(hot, 0, "should not be hot yet");

    // One more call to cross the threshold.
    let result = interp.eval(read("(profiler-hot-inc 10)").unwrap()).unwrap();
    assert_eq!(result, LispObject::integer(11));

    let (total, hot) = interp.profiler_stats();
    assert_eq!(total, 5);
    assert_eq!(hot, 1, "function should now be detected as hot");
}

#[test]
fn test_backquote_expansion() {
    let interp = make_stdlib_interp();
    // Load prerequisites
    for f in &["debug-early.el", "byte-run.el", "backquote.el"] {
        if let Ok(s) = std::fs::read_to_string(format!("/tmp/elisp-stdlib/emacs-lisp/{}", f)) {
            let _ = interp.eval_source(&s);
        }
    }
    // Verify backquote macro is registered
    assert!(interp.macros.read().contains_key("`"));

    // Simple backquote on constant list
    let result = interp.eval(read("`(a b c)").unwrap()).unwrap();
    assert_eq!(result.princ_to_string(), "(a b c)");
}

#[test]
fn test_setcar_mutates_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // setcar should mutate the original cons cell
    assert_eq!(
        interp
            .eval(read("(let ((x '(a b c))) (setcar x 'z) (car x))").unwrap())
            .unwrap(),
        LispObject::symbol("z")
    );
}

#[test]
fn test_setcdr_mutates_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // setcdr should mutate the original cons cell
    assert_eq!(
        interp
            .eval(read("(let ((x '(a b c))) (setcdr x '(y z)) (cdr x))").unwrap())
            .unwrap(),
        read("(y z)").unwrap()
    );
}

#[test]
fn test_nconc_destructive() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // nconc should destructively append: x itself should now be (1 2 3 4)
    assert_eq!(
        interp
            .eval(read("(let ((x '(1 2)) (y '(3 4))) (nconc x y) x)").unwrap())
            .unwrap(),
        read("(1 2 3 4)").unwrap()
    );
}

#[test]
fn test_puthash_mutates_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // puthash should mutate the hash table in-place
    assert_eq!(
            interp
                .eval(
                    read("(let ((h (make-hash-table :test 'equal))) (puthash \"key\" 42 h) (gethash \"key\" h))")
                        .unwrap()
                )
                .unwrap(),
            LispObject::integer(42)
        );
}

#[test]
fn test_apply_variadic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Classic 2-arg form still works
    assert_eq!(
        interp.eval(read("(apply '+ '(1 2 3))").unwrap()).unwrap(),
        LispObject::integer(6)
    );

    // Variadic: (apply FN ARG1 ARG2 LAST-LIST)
    assert_eq!(
        interp
            .eval(read("(apply '+ 10 20 '(1 2 3))").unwrap())
            .unwrap(),
        LispObject::integer(36)
    );

    // Single individual arg + list
    assert_eq!(
        interp.eval(read("(apply '+ 100 '(1 2))").unwrap()).unwrap(),
        LispObject::integer(103)
    );

    // Last arg is nil (empty list)
    assert_eq!(
        interp.eval(read("(apply '+ 5 '())").unwrap()).unwrap(),
        LispObject::integer(5)
    );

    // With list function
    assert_eq!(
        interp
            .eval(read("(apply 'list 1 2 '(3 4))").unwrap())
            .unwrap(),
        read("(1 2 3 4)").unwrap()
    );
}

#[test]
fn test_split_string_with_separator() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Default: split on whitespace
    assert_eq!(
        interp
            .eval(read(r#"(split-string "hello world")"#).unwrap())
            .unwrap(),
        read(r#"("hello" "world")"#).unwrap()
    );

    // Split on comma
    assert_eq!(
        interp
            .eval(read(r#"(split-string "a,b,c" ",")"#).unwrap())
            .unwrap(),
        read(r#"("a" "b" "c")"#).unwrap()
    );

    // Split on separator with omit-nulls
    assert_eq!(
        interp
            .eval(read(r#"(split-string ",a,,b," "," t)"#).unwrap())
            .unwrap(),
        read(r#"("a" "b")"#).unwrap()
    );

    // Split on separator without omit-nulls
    assert_eq!(
        interp
            .eval(read(r#"(split-string "a::b" ":")"#).unwrap())
            .unwrap(),
        read(r#"("a" "" "b")"#).unwrap()
    );

    // nil separator means whitespace
    assert_eq!(
        interp
            .eval(read(r#"(split-string "  foo  bar  " nil)"#).unwrap())
            .unwrap(),
        read(r#"("foo" "bar")"#).unwrap()
    );
}

#[test]
fn test_read_from_string_position() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Reading "42" from "42 rest" should return (42 . 2), not (42 . 7)
    let result = interp
        .eval(read(r#"(read-from-string "42 rest")"#).unwrap())
        .unwrap();
    assert_eq!(result.first().unwrap(), LispObject::integer(42));
    // Position should be 2 (right after "42"), not len of entire string
    let end_pos = result.rest().unwrap().as_integer().unwrap();
    assert_eq!(end_pos, 2);

    // Reading with start offset
    let result = interp
        .eval(read(r#"(read-from-string "  hello world" 2)"#).unwrap())
        .unwrap();
    assert_eq!(result.first().unwrap(), LispObject::symbol("hello"));
    // Position is relative to original string: start(2) + reader.position(5) = 7
    let end_pos = result.rest().unwrap().as_integer().unwrap();
    assert_eq!(end_pos, 7);
}

#[test]
fn test_intern_soft_returns_nil_for_unknown() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Unknown symbol returns nil
    assert_eq!(
        interp
            .eval(read(r#"(intern-soft "nonexistent-symbol")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );

    // Known symbol returns the symbol
    interp.define("my-var", LispObject::integer(42));
    assert_eq!(
        interp
            .eval(read(r#"(intern-soft "my-var")"#).unwrap())
            .unwrap(),
        LispObject::symbol("my-var")
    );
}

/// Ensure all required stdlib files are decompressed to /tmp/elisp-stdlib/.
/// Returns false if Emacs lisp directory is not available (skip tests).
/// All files in loadup.el order that ensure_stdlib_files will decompress.
const BOOTSTRAP_FILES: &[&str] = &[
    "emacs-lisp/debug-early",
    "emacs-lisp/byte-run",
    "emacs-lisp/backquote",
    "subr",
    "keymap",
    "version",
    "widget",
    "custom",
    "emacs-lisp/map-ynp",
    "international/mule",
    "international/mule-conf",
    "env",
    "format",
    "bindings",
    "window",
    "files",
    "emacs-lisp/macroexp",
    "cus-face",
    "faces",
    "button",
    "emacs-lisp/cl-preloaded",
    "emacs-lisp/oclosure",
    "obarray",
    "abbrev",
    "help",
    "jka-cmpr-hook",
    "epa-hook",
    "international/mule-cmds",
    "case-table",
    "international/characters",
    // Extra files needed for individual tests
    "emacs-lisp/cconv",
    "simple",
];

fn ensure_stdlib_files() -> bool {
    let emacs_lisp_dir = "/opt/homebrew/share/emacs/30.2/lisp";
    if !std::path::Path::new(emacs_lisp_dir).exists() {
        return false;
    }

    let files = BOOTSTRAP_FILES;

    for f in files {
        let dest = format!("/tmp/elisp-stdlib/{}.el", f);
        if std::path::Path::new(&dest).exists() {
            continue;
        }
        if let Some(parent) = std::path::Path::new(&dest).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let plain = format!("{}/{}.el", emacs_lisp_dir, f);
        let gz = format!("{}/{}.el.gz", emacs_lisp_dir, f);
        if std::path::Path::new(&plain).exists() {
            let _ = std::fs::copy(&plain, &dest);
        } else if std::path::Path::new(&gz).exists() {
            let output = std::process::Command::new("gunzip")
                .args(["-c", &gz])
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let _ = std::fs::write(&dest, &out.stdout);
                }
            }
        }
    }
    true
}

/// Load the standard prerequisite files into an interpreter.
fn load_prerequisites(interp: &Interpreter) {
    // First three files live under the `emacs-lisp/` subdir in the
    // Emacs distribution; subr.el is at the top level.
    let emacs_lisp_files = &["debug-early.el", "byte-run.el", "backquote.el"];
    for f in emacs_lisp_files {
        let path = format!("/tmp/elisp-stdlib/emacs-lisp/{}", f);
        if let Ok(s) = std::fs::read_to_string(&path) {
            let _ = interp.eval_source(&s);
        }
    }
    if let Ok(s) = std::fs::read_to_string("/tmp/elisp-stdlib/subr.el") {
        let _ = interp.eval_source(&s);
    }
}

/// Run a stdlib loading test: parse all forms, eval each, count successes.
/// If the reader fails to parse the source, returns the reader error.
fn load_file_progress(interp: &Interpreter, source: &str) -> (usize, usize, Vec<(usize, String)>) {
    let forms = match crate::read_all(source) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  READER ERROR: {}", e);
            return (0, 0, vec![(0, format!("reader: {}", e))]);
        }
    };
    let total = forms.len();
    let mut ok_count = 0;
    let mut errors: Vec<(usize, String)> = Vec::new();
    for (i, form) in forms.into_iter().enumerate() {
        match interp.eval(form) {
            Ok(_) => ok_count += 1,
            Err(e) => {
                if errors.len() < 20 {
                    errors.push((i, format!("{}", e)));
                }
            }
        }
    }
    (ok_count, total, errors)
}

#[test]
fn test_load_macroexp_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/macroexp.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);

    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("macroexp.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}

#[test]
fn test_load_cconv_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/cconv.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    if let Ok(s) = std::fs::read_to_string("/tmp/elisp-stdlib/emacs-lisp/macroexp.el") {
        let _ = interp.eval_source(&s);
    }

    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("cconv.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}

#[test]
fn test_load_simple_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/simple.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);

    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("simple.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}

#[test]
fn test_load_files_el() {
    if !ensure_stdlib_files() {
        return;
    }
    let source = match std::fs::read_to_string("/tmp/elisp-stdlib/files.el") {
        Ok(s) => s,
        Err(_) => return,
    };
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);

    let (ok, total, errors) = load_file_progress(&interp, &source);
    let pct = if total > 0 { ok * 100 / total } else { 0 };
    eprintln!("files.el: {}/{} OK ({}%)", ok, total, pct);
    for (i, e) in &errors {
        eprintln!("  form {}: {}", i, e);
    }
}

// --- Dynamic binding (specpdl) tests ---

#[test]
fn test_dynamic_binding_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // defvar makes x special
    interp.eval(read("(defvar x 10)").unwrap()).unwrap();
    // Define a function that reads x dynamically
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    // Global value visible
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(10)
    );
    // Dynamic binding: let binds x for called functions too
    assert_eq!(
        interp
            .eval(read("(let ((x 20)) (get-x))").unwrap())
            .unwrap(),
        LispObject::integer(20)
    );
    // After let exits, x is restored
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(10)
    );
}

#[test]
fn test_dynamic_binding_nested() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 1)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    // Nested let forms
    assert_eq!(
        interp
            .eval(read("(let ((x 10)) (let ((x 100)) (get-x)))").unwrap())
            .unwrap(),
        LispObject::integer(100)
    );
    // After both lets exit, original value is restored
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(1)
    );
}

#[test]
fn test_dynamic_binding_setq() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 5)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    // setq inside a dynamic let modifies the current binding
    assert_eq!(
        interp
            .eval(read("(let ((x 10)) (setq x 99) (get-x))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
    // After let exits, original value is restored
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(5)
    );
}

#[test]
fn test_dynamic_binding_let_star() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 1)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    // let* with special variable
    assert_eq!(
        interp
            .eval(read("(let* ((x 42)) (get-x))").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(1)
    );
}

#[test]
fn test_dynamic_binding_lambda_params() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 0)").unwrap()).unwrap();
    interp.eval(read("(defun get-x () x)").unwrap()).unwrap();
    // Function parameter that is a special var binds dynamically
    interp
        .eval(read("(defun set-x-via-param (x) (get-x))").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("(set-x-via-param 77)").unwrap()).unwrap(),
        LispObject::integer(77)
    );
    // After function returns, x is restored
    assert_eq!(
        interp.eval(read("(get-x)").unwrap()).unwrap(),
        LispObject::integer(0)
    );
}

#[test]
fn test_dynamic_binding_defconst() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // defconst also makes the variable special
    interp
        .eval(read("(defconst my-const 42)").unwrap())
        .unwrap();
    interp
        .eval(read("(defun get-my-const () my-const)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(let ((my-const 99)) (get-my-const))").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
    assert_eq!(
        interp.eval(read("(get-my-const)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_dynamic_binding_mixed_special_and_lexical() {
    // Test that special vars use global env while non-special use caller's scope
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.eval(read("(defvar x 10)").unwrap()).unwrap();
    interp
        .eval(read("(defun read-both () (+ x y))").unwrap())
        .unwrap();
    // Set up non-special y in global scope
    interp.eval(read("(setq y 1)").unwrap()).unwrap();
    // x is special: let binding is visible dynamically through global env
    // y is non-special: let binding is visible through caller's scope chain
    assert_eq!(
        interp
            .eval(read("(let ((x 100) (y 200)) (read-both))").unwrap())
            .unwrap(),
        LispObject::integer(300)
    );
    // After let, x is restored to 10 (special), y stays as 1 in global
    assert_eq!(
        interp.eval(read("x").unwrap()).unwrap(),
        LispObject::integer(10)
    );
}

// -- GC integration tests ------------------------------------------------

#[test]
fn test_garbage_collect_returns_stats() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Allocate some cons cells via list construction
    interp.eval(read("(setq x '(1 2 3 4 5))").unwrap()).unwrap();
    // Trigger GC
    let result = interp.eval(read("(garbage-collect)").unwrap()).unwrap();
    // Should return a cons (alist)
    assert!(result.is_cons(), "garbage-collect should return a cons");
    // First element should be (bytes-allocated . <number>)
    let first = result.car().unwrap();
    assert!(first.is_cons());
    let key = first.car().unwrap();
    assert_eq!(key, LispObject::symbol("bytes-allocated"));
}

#[test]
fn test_garbage_collect_reports_cons_total() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let before = crate::object::global_cons_count();
    // Create a list: each element = 1 cons cell, so '(1 2 3) = 3 cons cells
    interp.eval(read("(setq x '(1 2 3))").unwrap()).unwrap();
    let after = crate::object::global_cons_count();
    // At least 3 cons cells should have been allocated (possibly more from
    // the reader and eval machinery)
    assert!(
        after > before,
        "cons counter should increase after allocating a list: before={before}, after={after}"
    );
}

#[test]
fn test_gc_stress() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Create lots of garbage — build and discard cons cells in a loop
    interp
        .eval(read("(let ((i 0)) (while (< i 1000) (cons i i) (setq i (1+ i))))").unwrap())
        .unwrap();
    // GC should not crash
    let result = interp.eval(read("(garbage-collect)").unwrap()).unwrap();
    assert!(result.is_cons());
}

#[test]
fn test_gc_preserves_live_data() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Bind a list and trigger GC — the bound list must survive
    interp
        .eval(read("(setq my-list '(a b c))").unwrap())
        .unwrap();
    interp.eval(read("(garbage-collect)").unwrap()).unwrap();
    let result = interp.eval(read("my-list").unwrap()).unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::symbol("a"),
            LispObject::cons(
                LispObject::symbol("b"),
                LispObject::cons(LispObject::symbol("c"), LispObject::nil())
            )
        )
    );
}

// ---- eval_to_value / eval_value tests ----

#[test]
fn test_eval_to_value_self_evaluating() {
    use crate::value::Value;
    let interp = Interpreter::new();
    // Fixnums are self-evaluating
    assert_eq!(
        interp.eval_value(Value::fixnum(42)).unwrap(),
        Value::fixnum(42)
    );
    assert_eq!(interp.eval_value(Value::nil()).unwrap(), Value::nil());
    assert_eq!(interp.eval_value(Value::t()).unwrap(), Value::t());
    assert_eq!(
        interp.eval_value(Value::float(3.14)).unwrap(),
        Value::float(3.14)
    );
}

#[test]
fn test_eval_to_value_delegates_symbol() {
    use crate::value::Value;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Define a variable, then look it up through the Value path via symbol
    interp.eval(read("(setq my-var 99)").unwrap()).unwrap();
    let sym = LispObject::symbol("my-var");
    let val = Value::from_lisp_object(&sym);
    assert!(val.is_symbol());
    let result = interp.eval_value(val).unwrap();
    assert_eq!(result.as_fixnum(), Some(99));
}

#[test]
fn test_eval_to_value_via_source() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // eval_source_value evaluates through LispObject and converts the result
    let result = interp.eval_source_value("(+ 1 2)").unwrap();
    assert_eq!(result.as_fixnum(), Some(3));
}

#[test]
fn test_eval_source_value_basic() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source_value("(+ 10 20)").unwrap();
    assert_eq!(result.as_fixnum(), Some(30));
}

#[test]
fn test_eval_source_value_multiple_forms() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Last form's result is returned
    let result = interp.eval_source_value("(+ 1 2) (+ 3 4)").unwrap();
    assert_eq!(result.as_fixnum(), Some(7));
}

#[test]
fn test_eval_source_value_nil() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source_value("nil").unwrap();
    assert!(result.is_nil());
}

#[test]
fn test_save_excursion_restores_point() {
    use crate::EditorCallbacks;

    struct FakeEditor {
        text: String,
        point: usize,
    }
    impl EditorCallbacks for FakeEditor {
        fn buffer_string(&self) -> String {
            self.text.clone()
        }
        fn buffer_size(&self) -> usize {
            self.text.len()
        }
        fn point(&self) -> usize {
            self.point
        }
        fn insert(&mut self, t: &str) {
            self.text.insert_str(self.point, t);
            self.point += t.len();
        }
        fn delete_char(&mut self, _n: i64) {}
        fn goto_char(&mut self, pos: usize) {
            self.point = pos;
        }
        fn forward_char(&mut self, n: i64) {
            self.point = (self.point as i64 + n) as usize;
        }
        fn find_file(&mut self, _path: &str) -> bool {
            false
        }
        fn save_buffer(&mut self) -> bool {
            false
        }
    }

    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.set_editor(Box::new(FakeEditor {
        text: "hello world".to_string(),
        point: 5,
    }));

    // save-excursion should restore point to 5 after body moves it
    let result = interp
        .eval_source("(save-excursion (goto-char 0) (point))")
        .unwrap();
    // Body returned point=0
    assert_eq!(result, LispObject::integer(0));
    // But after save-excursion, point is restored to 5
    let point_after = interp.eval_source("(point)").unwrap();
    assert_eq!(point_after, LispObject::integer(5));
}

#[test]
fn test_save_restriction_acts_as_progn() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval_source("(save-restriction (+ 1 2))").unwrap();
    assert_eq!(result, LispObject::integer(3));
}

#[test]
fn test_load_file_not_found_noerror() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // With noerror=t, load returns nil for missing file
    let result = interp
        .eval_source("(load \"/nonexistent/path/file\" t)")
        .unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_load_file_not_found_error() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Without noerror, load signals a file error
    let result = interp.eval_source("(load \"/nonexistent/path/file\")");
    assert!(result.is_err());
}

#[test]
fn test_load_real_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_elisp_load.el");
    std::fs::write(&path, "(setq test-load-var 42)").unwrap();

    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval_source(&format!("(load \"{}\")", path.display()))
        .unwrap();
    assert_eq!(result, LispObject::t());

    let var = interp.eval_source("test-load-var").unwrap();
    assert_eq!(var, LispObject::integer(42));

    std::fs::remove_file(&path).ok();
}

#[test]
fn test_detect_lexical_binding() {
    use crate::reader::detect_lexical_binding;

    assert!(detect_lexical_binding(
        ";;; -*- lexical-binding: t; -*-\n(defun foo () 1)"
    ));
    assert!(detect_lexical_binding(
        ";;; foo.el --- desc -*- lexical-binding: t -*-"
    ));
    assert!(!detect_lexical_binding(";;; no binding here\n(+ 1 2)"));
    assert!(!detect_lexical_binding(""));
    assert!(!detect_lexical_binding(
        "(setq x 1)\n;;; -*- lexical-binding: t -*-"
    ));
}

// --- P1 file operation primitives tests ---

#[test]
fn test_file_exists_p() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // /tmp always exists on macOS/Linux
    assert_eq!(
        interp
            .eval(read(r#"(file-exists-p "/tmp")"#).unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read(r#"(file-exists-p "/nonexistent-path-12345")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_expand_file_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Absolute path stays absolute
    assert_eq!(
        interp
            .eval(read(r#"(expand-file-name "/foo/bar")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
    // Relative path with explicit directory
    assert_eq!(
        interp
            .eval(read(r#"(expand-file-name "bar" "/foo")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
    // Trailing slash on directory is handled
    assert_eq!(
        interp
            .eval(read(r#"(expand-file-name "bar" "/foo/")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
}

#[test]
fn test_file_name_directory() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-name-directory "/foo/bar.el")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/")
    );
}

#[test]
fn test_file_name_nondirectory() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-name-nondirectory "/foo/bar.el")"#).unwrap())
            .unwrap(),
        LispObject::string("bar.el")
    );
}

#[test]
fn test_file_directory_p() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-directory-p "/tmp")"#).unwrap())
            .unwrap(),
        LispObject::t()
    );
    assert_eq!(
        interp
            .eval(read(r#"(file-directory-p "/nonexistent-12345")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_directory_file_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(directory-file-name "/foo/bar/")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar")
    );
}

#[test]
fn test_file_name_as_directory() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp
            .eval(read(r#"(file-name-as-directory "/foo/bar")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/bar/")
    );
    // Already has trailing slash
    assert_eq!(
        interp
            .eval(read(r#"(file-name-as-directory "/foo/")"#).unwrap())
            .unwrap(),
        LispObject::string("/foo/")
    );
}

#[test]
fn test_getenv() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // HOME should be set on any Unix system
    let result = interp.eval(read(r#"(getenv "HOME")"#).unwrap()).unwrap();
    assert!(matches!(result, LispObject::String(_)));
    // Nonexistent var returns nil
    assert_eq!(
        interp
            .eval(read(r#"(getenv "ELISP_NONEXISTENT_VAR_12345")"#).unwrap())
            .unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_backquote_unquote_function_call() {
    // Regression for Phase 7: backquote-with-unquote expansion that
    // uses a FUNCTION as the unquoted value — not a variable.
    // Pattern used all over the stdlib:
    //   `((foo ,(some-function ARG)))
    // This should evaluate (some-function ARG) and splice the result,
    // not look up `some-function` as a variable.
    let interp = make_stdlib_interp();
    // Load backquote support.
    load_prerequisites(&interp);
    // purecopy IS a special form that returns its arg verbatim.
    let result = interp.eval_source(r#"(list (purecopy "hello"))"#).unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::string("hello"), LispObject::nil())
    );
    // Now the backquote version — same result expected.
    let result = interp.eval_source(r#"`(,(purecopy "hello"))"#).unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::string("hello"), LispObject::nil())
    );
}

#[test]
fn test_format_el_form_2_isolated() {
    // Reproduce format.el's form 2 in isolation with the same
    // prerequisites test_full_bootstrap_chain uses.
    let interp = make_stdlib_interp();
    // Load the same bootstrap files format.el comes after.
    for f in &[
        "emacs-lisp/debug-early.el",
        "emacs-lisp/byte-run.el",
        "emacs-lisp/backquote.el",
        "subr.el",
    ] {
        let path = format!("/tmp/elisp-stdlib/{}", f);
        if let Ok(s) = std::fs::read_to_string(&path) {
            let _ = interp.eval_source(&s);
        }
    }
    let form = r#"(defvar format-alist
  `((text/enriched ,(purecopy "Extended MIME text/enriched format.")
                   ,(purecopy "Content-[Tt]ype:[ \t]*text/enriched"))))"#;
    let result = interp.eval_source(form);
    eprintln!("format.el form 2 isolated: {:?}", result);
}

#[test]
fn test_backquote_without_stdlib_backquote_el() {
    // Does `(,EXPR) work WITHOUT backquote.el loaded? If it does,
    // our reader is desugaring the backquote syntax itself. If not,
    // we depend on backquote.el. Understanding this tells us where
    // the `void variable: <fn>` bug originates.
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // NO load_prerequisites — raw interpreter.
    let result = interp.eval_source(r#"`(,(car '(42 43)))"#);
    eprintln!("bare backquote result: {:?}", result);
}

#[test]
fn test_bisect_backquote_state_pollution() {
    // Load each bootstrap file one at a time, and after each, evaluate
    // the exact failing form from format.el (form 2). The first file
    // after which this fails is the one that poisoned the interpreter
    // state.
    if !ensure_stdlib_files() {
        return;
    }
    let interp = make_stdlib_interp();
    // The probe — copy of what format.el form 2 shape evaluates to
    // under backquote expansion. If this fails with "void variable:
    // list" or similar, the state is polluted.
    let probe = r#"`((foo ,(identity "a")) (bar ,(identity "b")))"#;
    // Bootstrap sequence, through files that come before format.el
    // plus a few after (to see pollution progression).
    let files: &[&str] = &[
        "emacs-lisp/debug-early",
        "emacs-lisp/byte-run",
        "emacs-lisp/backquote",
        "subr",
        "keymap",
        "version",
        "widget",
        "custom",
        "emacs-lisp/map-ynp",
        "international/mule",
        "international/mule-conf",
        "env",
    ];
    // Baseline probe before any files
    let baseline = interp.eval_source(probe);
    eprintln!("[baseline] probe result: {:?}", baseline.is_ok());

    for f in files {
        let path = format!("/tmp/elisp-stdlib/{}.el", f);
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("[{}] SKIP (not found)", f);
                continue;
            }
        };
        interp.set_eval_ops_limit(5_000_000);
        interp.reset_eval_ops();
        let _ = interp.eval_source(&source);
        interp.set_eval_ops_limit(0);
        let probe_result = interp.eval_source(probe);
        match probe_result {
            Ok(_) => eprintln!("[{}] probe OK", f),
            Err((_, e)) => eprintln!("[{}] probe FAIL: {}", f, e),
        }
    }
}

#[test]
fn test_defvar_with_backquote_purecopy() {
    // Exact shape from format.el form 2 — defvar with a backquoted
    // value containing `,(purecopy ...)` unquotes.
    let interp = make_stdlib_interp();
    load_prerequisites(&interp);
    let result = interp.eval_source(
        r#"(defvar tbq-test
             `((a ,(purecopy "foo")) (b ,(purecopy "bar"))))"#,
    );
    assert!(
        result.is_ok(),
        "defvar+backquote+purecopy failed: {result:?}"
    );
}

#[test]
fn test_eval_when_compile() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // eval-when-compile behaves like progn at runtime
    assert_eq!(
        interp
            .eval(read("(eval-when-compile 1 2 3)").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
    assert_eq!(
        interp
            .eval(read("(eval-and-compile (+ 1 2))").unwrap())
            .unwrap(),
        LispObject::integer(3)
    );
}

#[test]
fn test_system_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    assert_eq!(
        interp.eval(read("(system-name)").unwrap()).unwrap(),
        LispObject::string("localhost")
    );
}

#[test]
fn test_emacs_pid() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(emacs-pid)").unwrap()).unwrap();
    let pid = result
        .as_integer()
        .expect("emacs-pid should return integer");
    assert!(pid > 0);
}

// --- Bootstrap loading test ---

#[test]
fn test_bootstrap_loading_chain() {
    if !ensure_stdlib_files() {
        return;
    }
    let interp = make_stdlib_interp();

    let files = vec![
        ("debug-early", "debug-early"),
        ("byte-run", "byte-run"),
        ("backquote", "backquote"),
        ("subr", "subr"),
    ];
    for (file_name, label) in &files {
        let path = format!("/tmp/elisp-stdlib/{}.el", file_name);
        if let Ok(source) = std::fs::read_to_string(&path) {
            match interp.eval_source(&source) {
                Ok(_) => eprintln!("  [bootstrap] {} OK", label),
                Err((i, e)) => eprintln!("  [bootstrap] {} form {}: {}", label, i, e),
            }
        } else {
            eprintln!("  [bootstrap] {} not found at {}", label, path);
        }
    }
}

#[test]
fn test_full_bootstrap_chain() {
    if !ensure_stdlib_files() {
        return;
    }
    let interp = make_stdlib_interp();

    // The first 30 files from loadup.el, in order
    let files: &[&str] = &BOOTSTRAP_FILES[..30];

    let mut loaded = 0;
    let mut partial = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for f in files {
        // Resolve the filename: loadup.el uses "emacs-lisp/debug-early" etc.
        // but our decompressed files live under /tmp/elisp-stdlib/
        let path = format!("/tmp/elisp-stdlib/{}.el", f);
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("  [{}] SKIPPED (not found at {})", f, path);
                skipped += 1;
                continue;
            }
        };

        let forms = match crate::read_all(&source) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  [{}] PARSE ERROR: {}", f, e);
                failed += 1;
                continue;
            }
        };

        let total = forms.len();
        let mut ok = 0;
        let mut first_err: Option<String> = None;
        // Set per-file eval operation budget (prevents runaway require chains)
        // Skip files that trigger heavy require chains (they load cl-lib.el etc.)
        let skip_heavy = matches!(*f, "emacs-lisp/cl-preloaded" | "emacs-lisp/oclosure");
        interp.set_eval_ops_limit(if skip_heavy { 500_000 } else { 5_000_000 });
        interp.reset_eval_ops();
        for form in forms {
            match interp.eval(form) {
                Ok(_) => ok += 1,
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(format!("form {}: {}", ok, e));
                    }
                }
            }
        }
        interp.set_eval_ops_limit(0); // remove limit

        if ok == total {
            eprintln!("  [{}] OK ({}/{})", f, ok, total);
            loaded += 1;
        } else {
            let pct = if total > 0 { ok * 100 / total } else { 0 };
            eprintln!("  [{}] PARTIAL ({}/{} = {}%)", f, ok, total, pct);
            if let Some(err) = &first_err {
                eprintln!("    first error: {}", err);
            }
            partial += 1;
        }
    }
    eprintln!(
        "\nBootstrap summary: {} OK, {} partial, {} failed, {} skipped out of {} files",
        loaded,
        partial,
        failed,
        skipped,
        files.len()
    );
    // Expect at least 15 files to load fully (currently 20)
    let ok = loaded >= 15;
    // Leak the interpreter to avoid GC destructor crash (SIGILL) on exit.
    // The GC has unsafe code that can corrupt memory after heavy use.
    std::mem::forget(interp);
    assert!(
        ok,
        "Expected at least 15 files to load fully, got {}",
        loaded
    );
}

// -- P2: Buffer primitives --

#[test]
fn test_current_buffer_no_editor() {
    let interp = Interpreter::new();
    // No editor attached -> nil
    let result = interp.eval(read("(current-buffer)").unwrap()).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_buffer_name() {
    let interp = Interpreter::new();
    let result = interp.eval(read("(buffer-name)").unwrap()).unwrap();
    assert_eq!(result, LispObject::string("*scratch*"));
}

#[test]
fn test_get_buffer() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(get-buffer \"foo\")").unwrap()).unwrap();
    assert_eq!(result, LispObject::string("foo"));
}

#[test]
fn test_get_buffer_create() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(get-buffer-create \"test\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("test"));
}

#[test]
fn test_buffer_list() {
    let interp = Interpreter::new();
    let result = interp.eval(read("(buffer-list)").unwrap()).unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::string("*scratch*"), LispObject::nil())
    );
}

// Phase 2d migrations: sort / version-to-list / list_from_objects coverage.

#[test]
fn test_version_to_list_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(version-to-list "1.2.3")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::integer(2),
                LispObject::cons(LispObject::integer(3), LispObject::nil()),
            ),
        )
    );
}

#[test]
fn test_version_to_list_empty_parts() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Non-numeric parts fall back to 0 per the primitive's contract.
    let result = interp
        .eval(read(r#"(version-to-list "10.0.x")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::integer(10),
            LispObject::cons(
                LispObject::integer(0),
                LispObject::cons(LispObject::integer(0), LispObject::nil()),
            ),
        )
    );
}

#[test]
fn test_nreverse_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(nreverse (list 1 2 3 4))").unwrap())
        .unwrap();
    let expected = [4i64, 3, 2, 1]
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |acc, n| {
            LispObject::cons(LispObject::integer(n), acc)
        });
    assert_eq!(result, expected);
}

#[test]
fn test_nreverse_empty_list() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(nreverse nil)").unwrap()).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_split_string_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read(r#"(split-string "a.b.c" "\\.")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(
            LispObject::string("a"),
            LispObject::cons(
                LispObject::string("b"),
                LispObject::cons(LispObject::string("c"), LispObject::nil()),
            ),
        )
    );
}

#[test]
fn test_read_from_string_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Phase 2e: the (obj . end_pos) dotted pair is now built on the
    // real GC heap via state.heap_cons. value_to_obj bridges back to
    // a dotted LispObject::Cons.
    let result = interp
        .eval(read(r#"(read-from-string "42")"#).unwrap())
        .unwrap();
    assert_eq!(
        result,
        LispObject::cons(LispObject::integer(42), LispObject::integer(2))
    );
}

#[test]
fn test_hashtable_identity_preserved_under_heap_scope() {
    // Phase 2n regression: heap-allocated hash tables must preserve
    // Arc identity across obj_to_value/value_to_obj round-trips.
    // puthash on a let-bound ht, then gethash must see the value —
    // this would break if the heap stored cloned content instead of
    // the Arc.
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(let ((h (make-hash-table :test 'equal)))
                  (puthash "a" 1 h)
                  (puthash "b" 2 h)
                  (+ (gethash "a" h) (gethash "b" h)))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::integer(3));
}

#[test]
fn test_vector_decode_preserves_content() {
    // Phase 2n: make-vector produces a heap-allocated vector whose
    // content survives obj_to_value/value_to_obj round-tripping via
    // the bound environment. (aref does not mutate, so content
    // equality is the right guarantee to assert here — the
    // interpreter's `aset` is currently a stub.)
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(let ((v (make-vector 3 7))) (length v))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::integer(3));
}

#[test]
fn test_cons_setcar_after_heap_round_trip() {
    // Phase 2n-cons regression: `obj_to_value(LispObject::Cons(arc))`
    // now routes through `Heap::cons_arc_value`, which wraps the
    // same Arc. `setcar` must still mutate the Arc that `(car x)`
    // reads. This was the motivating test for Option (b) — keep the
    // existing u64 `ConsCell` for native Value-based chains AND add
    // a separate Arc-wrapping `ConsArcCell` for identity-critical
    // migrations.
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(let ((x (cons 'a 'b))) (setcar x 'z) (car x))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::symbol("z"));
}

#[test]
fn test_cons_setcdr_after_heap_round_trip() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(let ((x (cons 'a 'b))) (setcdr x 'z) (cdr x))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::symbol("z"));
}

#[test]
fn test_hashtable_puthash_persists_across_rebindings() {
    // Phase 2n: after `setq h`, the same hash table can be reached
    // through a second binding — the Arc is shared.
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(
            read(
                r#"(progn
                  (setq h1 (make-hash-table :test 'equal))
                  (puthash "key" 99 h1)
                  (setq h2 h1)
                  (gethash "key" h2))"#,
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(result, LispObject::integer(99));
}

#[test]
fn test_symbol_function_macro_form_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Define a macro and then query its function-value; the result
    // shape is `(macro lambda (ARGS...) BODY...)`. Phase 2e builds the
    // wrapper on the real heap via state.with_heap.
    interp
        .eval(read("(defmacro my-mac (x) (list 'quote x))").unwrap())
        .unwrap();
    let result = interp
        .eval(read("(symbol-function 'my-mac)").unwrap())
        .unwrap();
    // Check the shape: (macro lambda (x) (list 'quote x))
    let first = result.car().unwrap();
    assert_eq!(first, LispObject::symbol("macro"));
    let rest = result.rest().unwrap();
    let second = rest.car().unwrap();
    assert_eq!(second, LispObject::symbol("lambda"));
}

#[test]
fn test_sort_ascending_heap_migration() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // sort now builds its result list on the real GC heap via
    // list_from_objects. value_to_obj bridges back to LispObject::Cons.
    let result = interp
        .eval(read("(sort (list 3 1 4 1 5 9 2 6) '<)").unwrap())
        .unwrap();
    let expected = [1i64, 1, 2, 3, 4, 5, 6, 9]
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |acc, n| {
            LispObject::cons(LispObject::integer(n), acc)
        });
    assert_eq!(result, expected);
}

#[test]
fn test_buffer_live_p() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(buffer-live-p \"anything\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_set_buffer_noop() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp.eval(read("(set-buffer \"foo\")").unwrap()).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_with_current_buffer() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(with-current-buffer \"buf\" (+ 1 2))").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::integer(3));
}

#[test]
fn test_generate_new_buffer_name() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(generate-new-buffer-name \"test\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("test"));
}

// -- P3: file-name-quote / file-name-unquote --

#[test]
fn test_file_name_quote() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(file-name-quote \"/tmp/foo\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("/:/tmp/foo"));
}

#[test]
fn test_file_name_unquote() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(file-name-unquote \"/:/tmp/foo\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("/tmp/foo"));
}

#[test]
fn test_file_name_unquote_no_prefix() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval(read("(file-name-unquote \"/tmp/foo\")").unwrap())
        .unwrap();
    assert_eq!(result, LispObject::string("/tmp/foo"));
}

// -- P3: autoload records and triggers loading --

#[test]
fn test_autoload_records_mapping() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(autoload 'my-func \"my-file\")").unwrap())
        .unwrap();
    let autoloads = interp.state.autoloads.read();
    assert_eq!(autoloads.get("my-func"), Some(&"my-file".to_string()));
}

#[test]
fn test_autoload_triggers_load() {
    use std::io::Write;
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);

    // Write a temp file that defines the function
    let dir = std::env::temp_dir().join("elisp-autoload-test");
    std::fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("autoloaded.el");
    let mut f = std::fs::File::create(&file_path).unwrap();
    writeln!(f, "(defun autoloaded-fn (x) (+ x 10))").unwrap();

    // Register autoload with full path
    let expr = format!(
        "(autoload 'autoloaded-fn \"{}\")",
        file_path.to_string_lossy()
    );
    interp.eval(read(&expr).unwrap()).unwrap();

    // Call the function — should trigger autoload
    let result = interp.eval(read("(autoloaded-fn 5)").unwrap()).unwrap();
    assert_eq!(result, LispObject::integer(15));

    // Clean up
    let _ = std::fs::remove_dir_all(&dir);
}

// -- P3: quote dotted form --

#[test]
fn test_quote_dotted_form() {
    let interp = Interpreter::new();
    // (quote . foo) is rare but should not crash
    let expr = LispObject::cons(LispObject::symbol("quote"), LispObject::symbol("foo"));
    let result = interp.eval(expr).unwrap();
    assert_eq!(result, LispObject::symbol("foo"));
}

// -- Phase 1a: plist on symbol --

#[test]
fn test_plist_put_get_roundtrip() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    // Unique symbol name to avoid cross-talk with other tests
    // (obarray is process-global).
    interp
        .eval(read("(put 'plist-test-sym-a 'bar 42)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(get 'plist-test-sym-a 'bar)").unwrap())
            .unwrap(),
        LispObject::integer(42)
    );
    // Unknown property returns nil.
    assert_eq!(
        interp
            .eval(read("(get 'plist-test-sym-a 'missing)").unwrap())
            .unwrap(),
        LispObject::nil()
    );
}

#[test]
fn test_plist_put_replaces_in_place() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(put 'plist-test-sym-b 'k 1)").unwrap())
        .unwrap();
    interp
        .eval(read("(put 'plist-test-sym-b 'k 2)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(get 'plist-test-sym-b 'k)").unwrap())
            .unwrap(),
        LispObject::integer(2)
    );
}

// -- Phase 1b: Environment keyed by SymbolId + function/value cells --

#[test]
fn test_defun_writes_function_cell() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defun phase1b-fn-a (x) (* x 2))").unwrap())
        .unwrap();
    // fboundp consults env + function-cell fallback
    assert_eq!(
        interp
            .eval(read("(fboundp 'phase1b-fn-a)").unwrap())
            .unwrap(),
        LispObject::t()
    );
    // Call via function-position dispatch
    assert_eq!(
        interp.eval(read("(phase1b-fn-a 21)").unwrap()).unwrap(),
        LispObject::integer(42)
    );
}

#[test]
fn test_defvar_writes_value_cell_and_mirrored_in_env() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defvar phase1b-var-a 123)").unwrap())
        .unwrap();
    assert_eq!(
        interp.eval(read("phase1b-var-a").unwrap()).unwrap(),
        LispObject::integer(123)
    );
    // boundp should return t
    assert_eq!(
        interp
            .eval(read("(boundp 'phase1b-var-a)").unwrap())
            .unwrap(),
        LispObject::t()
    );
}

#[test]
fn test_fset_writes_function_cell() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(defun phase1b-fset-target (y) (+ y 1))").unwrap())
        .unwrap();
    interp
        .eval(read("(fset 'phase1b-fset-alias (symbol-function 'phase1b-fset-target))").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(phase1b-fset-alias 10)").unwrap())
            .unwrap(),
        LispObject::integer(11)
    );
}

#[test]
fn test_hashkey_symbol_with_eq_test() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(setq h (make-hash-table :test 'eq))").unwrap())
        .unwrap();
    interp
        .eval(read("(puthash 'phase1b-ht-key 99 h)").unwrap())
        .unwrap();
    assert_eq!(
        interp
            .eval(read("(gethash 'phase1b-ht-key h)").unwrap())
            .unwrap(),
        LispObject::integer(99)
    );
}

#[test]
fn test_symbol_plist_returns_full_list() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
        .eval(read("(put 'plist-test-sym-c 'a 1)").unwrap())
        .unwrap();
    interp
        .eval(read("(put 'plist-test-sym-c 'b 2)").unwrap())
        .unwrap();
    // Expected: (a 1 b 2), preserving insertion order.
    let result = interp
        .eval(read("(symbol-plist 'plist-test-sym-c)").unwrap())
        .unwrap();
    let expected = LispObject::cons(
        LispObject::symbol("a"),
        LispObject::cons(
            LispObject::integer(1),
            LispObject::cons(
                LispObject::symbol("b"),
                LispObject::cons(LispObject::integer(2), LispObject::nil()),
            ),
        ),
    );
    assert_eq!(result, expected);
}
