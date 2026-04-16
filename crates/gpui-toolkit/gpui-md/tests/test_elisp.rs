use gpui_md::state::MdAppState;

fn state_with(text: &str) -> MdAppState {
    let mut s = MdAppState::new();
    s.document.set_text(text);
    s.cursor.position = 0;
    s.cursor.clear_selection();
    s
}

#[test]
fn elisp_interpreter_initialized() {
    let _s = MdAppState::new();
    assert!(true);
}

#[test]
fn elisp_eval_arithmetic() {
    let s = state_with("");
    let result = s.elisp.eval(gpui_elisp::read("(+ 1 2 3)").unwrap());
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), gpui_elisp::LispObject::integer(6));
}

#[test]
fn elisp_eval_list_operations() {
    let s = state_with("");
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(cons 1 '(2 3))").unwrap())
            .unwrap(),
        gpui_elisp::read("(1 2 3)").unwrap()
    );
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(car '(1 2 3))").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::integer(1)
    );
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(cdr '(1 2 3))").unwrap())
            .unwrap(),
        gpui_elisp::read("(2 3)").unwrap()
    );
}

#[test]
fn elisp_eval_string_operations() {
    let s = state_with("");
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(concat \"hello\" \" \" \"world\")").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::string("hello world")
    );
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(substring \"hello world\" 0 5)").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::string("hello")
    );
}

#[test]
fn elisp_eval_special_forms() {
    let s = state_with("");
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(if t 1 2)").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::integer(1)
    );
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(if nil 1 2)").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::integer(2)
    );
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(and t t t)").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::t()
    );
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(or nil nil t)").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::t()
    );
}

#[test]
fn elisp_eval_defun_and_call() {
    let mut s = state_with("");
    s.elisp
        .eval(gpui_elisp::read("(defun add (x y) (+ x y))").unwrap())
        .unwrap();
    assert_eq!(
        s.elisp
            .eval(gpui_elisp::read("(add 3 4)").unwrap())
            .unwrap(),
        gpui_elisp::LispObject::integer(7)
    );
}

#[test]
fn elisp_eval_expression_command() {
    let s = state_with("");
    let handler = s.commands.get("eval-expression");
    assert!(handler.is_some());
}

#[test]
fn elisp_primitives_registered() {
    let s = MdAppState::new();
    assert!(s.elisp.eval(gpui_elisp::read("(+ 1 2)").unwrap()).is_ok());
    assert!(s
        .elisp
        .eval(gpui_elisp::read("(cons 1 2)").unwrap())
        .is_ok());
    assert!(s
        .elisp
        .eval(gpui_elisp::read("(car '(1 2))").unwrap())
        .is_ok());
}

#[test]
fn elisp_macro_defmacro_and_call() {
    let mut s = state_with("");
    s.elisp
        .eval(gpui_elisp::read("(defmacro my-not (x) (list 'if x nil t))").unwrap())
        .unwrap();
    let result = s
        .elisp
        .eval(gpui_elisp::read("(my-not t)").unwrap())
        .unwrap();
    assert_eq!(result, gpui_elisp::LispObject::nil());
}
