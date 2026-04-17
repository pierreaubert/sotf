use crate::error::{ElispError, ElispResult, SignalData};
use crate::object::LispObject;
use std::sync::atomic::{AtomicI64, Ordering};

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    interp.define("+", LispObject::primitive("+"));
    interp.define("-", LispObject::primitive("-"));
    interp.define("*", LispObject::primitive("*"));
    interp.define("/", LispObject::primitive("/"));
    interp.define("=", LispObject::primitive("="));
    interp.define("<", LispObject::primitive("<"));
    interp.define(">", LispObject::primitive(">"));
    interp.define("<=", LispObject::primitive("<="));
    interp.define(">=", LispObject::primitive(">="));
    interp.define("/=", LispObject::primitive("/="));
    interp.define("cons", LispObject::primitive("cons"));
    interp.define("car", LispObject::primitive("car"));
    interp.define("cdr", LispObject::primitive("cdr"));
    interp.define("list", LispObject::primitive("list"));
    interp.define("length", LispObject::primitive("length"));
    interp.define("append", LispObject::primitive("append"));
    interp.define("reverse", LispObject::primitive("reverse"));
    interp.define("member", LispObject::primitive("member"));
    interp.define("assoc", LispObject::primitive("assoc"));
    interp.define("eq", LispObject::primitive("eq"));
    interp.define("equal", LispObject::primitive("equal"));
    interp.define("not", LispObject::primitive("not"));
    interp.define("null", LispObject::primitive("null"));
    interp.define("symbolp", LispObject::primitive("symbolp"));
    interp.define("numberp", LispObject::primitive("numberp"));
    interp.define("listp", LispObject::primitive("listp"));
    interp.define("consp", LispObject::primitive("consp"));
    interp.define("stringp", LispObject::primitive("stringp"));
    interp.define("princ", LispObject::primitive("princ"));
    interp.define("prin1", LispObject::primitive("prin1"));
    interp.define("string=", LispObject::primitive("string="));
    interp.define("string<", LispObject::primitive("string<"));
    interp.define("concat", LispObject::primitive("concat"));
    interp.define("substring", LispObject::primitive("substring"));

    // New primitives — list operations
    interp.define("nth", LispObject::primitive("nth"));
    interp.define("nthcdr", LispObject::primitive("nthcdr"));
    interp.define("setcar", LispObject::primitive("setcar"));
    interp.define("setcdr", LispObject::primitive("setcdr"));
    interp.define("nconc", LispObject::primitive("nconc"));
    interp.define("nreverse", LispObject::primitive("nreverse"));
    interp.define("delq", LispObject::primitive("delq"));
    interp.define("memq", LispObject::primitive("memq"));
    interp.define("assq", LispObject::primitive("assq"));
    interp.define("last", LispObject::primitive("last"));
    interp.define("copy-sequence", LispObject::primitive("copy-sequence"));
    interp.define("cadr", LispObject::primitive("cadr"));
    interp.define("cddr", LispObject::primitive("cddr"));
    interp.define("caar", LispObject::primitive("caar"));
    interp.define("cdar", LispObject::primitive("cdar"));
    interp.define("car-safe", LispObject::primitive("car-safe"));
    interp.define("cdr-safe", LispObject::primitive("cdr-safe"));
    interp.define("make-list", LispObject::primitive("make-list"));

    // New primitives — type predicates
    interp.define("atom", LispObject::primitive("atom"));
    interp.define("integerp", LispObject::primitive("integerp"));
    interp.define("floatp", LispObject::primitive("floatp"));
    interp.define("zerop", LispObject::primitive("zerop"));
    interp.define("natnump", LispObject::primitive("natnump"));
    // boundp and fboundp are handled by the eval dispatch (they need env access).
    interp.define("functionp", LispObject::primitive("functionp"));
    interp.define("subrp", LispObject::primitive("subrp"));

    // New primitives — numeric
    interp.define("1+", LispObject::primitive("1+"));
    interp.define("1-", LispObject::primitive("1-"));
    interp.define("mod", LispObject::primitive("mod"));
    interp.define("abs", LispObject::primitive("abs"));
    interp.define("max", LispObject::primitive("max"));
    interp.define("min", LispObject::primitive("min"));
    interp.define("floor", LispObject::primitive("floor"));
    interp.define("ceiling", LispObject::primitive("ceiling"));
    interp.define("round", LispObject::primitive("round"));
    interp.define("truncate", LispObject::primitive("truncate"));
    interp.define("float", LispObject::primitive("float"));
    interp.define("ash", LispObject::primitive("ash"));
    interp.define("logand", LispObject::primitive("logand"));
    interp.define("logior", LispObject::primitive("logior"));
    interp.define("lognot", LispObject::primitive("lognot"));

    // New primitives — symbol
    interp.define("symbol-name", LispObject::primitive("symbol-name"));
    // symbol-function is handled by the eval dispatch (needs env + macro table access).

    // New primitives — string
    interp.define(
        "string-to-number",
        LispObject::primitive("string-to-number"),
    );
    interp.define(
        "number-to-string",
        LispObject::primitive("number-to-string"),
    );
    interp.define("make-string", LispObject::primitive("make-string"));
    // string-match is handled by the eval dispatch (has regex support).

    // New primitives — I/O
    interp.define("prin1-to-string", LispObject::primitive("prin1-to-string"));

    // New primitives — misc
    interp.define("identity", LispObject::primitive("identity"));
    interp.define("ignore", LispObject::primitive("ignore"));
    interp.define("type-of", LispObject::primitive("type-of"));

    // String — extended
    interp.define("upcase", LispObject::primitive("upcase"));
    interp.define("downcase", LispObject::primitive("downcase"));
    interp.define("capitalize", LispObject::primitive("capitalize"));
    interp.define("safe-length", LispObject::primitive("safe-length"));
    interp.define("read", LispObject::primitive("read"));
    interp.define("characterp", LispObject::primitive("characterp"));
    // (string &rest CHARS) → make a string from character codepoints.
    interp.define("string", LispObject::primitive("string"));
    interp.define(
        "file-name-case-insensitive-p",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "define-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "define-coding-system-alias",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-coding-system-priority",
        LispObject::primitive("ignore"),
    );
    interp.define("set-charset-priority", LispObject::primitive("ignore"));
    interp.define(
        "set-safe-terminal-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("regexp-quote", LispObject::primitive("regexp-quote"));
    interp.define("max-char", LispObject::primitive("max-char"));
    interp.define("obarray-make", LispObject::primitive("ignore"));
    interp.define("obarray-get", LispObject::primitive("ignore"));
    interp.define("obarray-put", LispObject::primitive("ignore"));
    interp.define("optimize-char-table", LispObject::primitive("ignore"));
    interp.define("make-char-table", LispObject::primitive("ignore"));
    interp.define("set-char-table-parent", LispObject::primitive("ignore"));
    interp.define("standard-case-table", LispObject::primitive("ignore"));
    interp.define("standard-syntax-table", LispObject::primitive("ignore"));
    interp.define("syntax-table", LispObject::primitive("ignore"));
    interp.define("set-syntax-table", LispObject::primitive("ignore"));
    interp.define("char-table-extra-slot", LispObject::primitive("ignore"));
    interp.define("char-table-range", LispObject::primitive("ignore"));
    // charset/unicode stubs: return the code as-is for decode-char,
    // and pass-through for encode-char. Enough for mule-conf /
    // characters.el to load without blowing up on unsupported charsets.
    interp.define("decode-char", LispObject::primitive("decode-char"));
    interp.define("encode-char", LispObject::primitive("encode-char"));
    // Deep stdlib stubs — all return nil / no-op so load can continue.
    interp.define("unify-charset", LispObject::primitive("ignore"));
    interp.define("find-file-name-handler", LispObject::primitive("ignore"));
    interp.define(
        "unicode-property-table-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("set-char-table-range", LispObject::primitive("ignore"));
    interp.define("set-char-table-extra-slot", LispObject::primitive("ignore"));
    interp.define("map-char-table", LispObject::primitive("ignore"));
    interp.define("modify-category-entry", LispObject::primitive("ignore"));
    interp.define("modify-syntax-entry", LispObject::primitive("ignore"));
    interp.define("set-category-table", LispObject::primitive("ignore"));
    interp.define("define-category", LispObject::primitive("ignore"));
    interp.define("set-case-syntax", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-pair", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-delims", LispObject::primitive("ignore"));
    interp.define("string-replace", LispObject::primitive("string-replace"));
    interp.define("string-trim", LispObject::primitive("string-trim"));
    interp.define("string-prefix-p", LispObject::primitive("string-prefix-p"));
    interp.define("string-suffix-p", LispObject::primitive("string-suffix-p"));
    interp.define("string-join", LispObject::primitive("string-join"));
    interp.define("char-to-string", LispObject::primitive("char-to-string"));
    interp.define("string-to-char", LispObject::primitive("string-to-char"));
    interp.define("string-width", LispObject::primitive("string-width"));
    interp.define(
        "multibyte-string-p",
        LispObject::primitive("multibyte-string-p"),
    );

    // Sequence — extended
    interp.define("elt", LispObject::primitive("elt"));
    interp.define("copy-alist", LispObject::primitive("copy-alist"));
    interp.define("plist-get", LispObject::primitive("plist-get"));
    interp.define("plist-put", LispObject::primitive("plist-put"));
    interp.define("plist-member", LispObject::primitive("plist-member"));
    interp.define("remove", LispObject::primitive("remove"));
    interp.define("remq", LispObject::primitive("remq"));
    interp.define("number-sequence", LispObject::primitive("number-sequence"));

    // Numeric — extended
    interp.define("random", LispObject::primitive("random"));
    interp.define("logxor", LispObject::primitive("logxor"));

    // Type — extended
    interp.define("sequencep", LispObject::primitive("sequencep"));
    interp.define(
        "char-or-string-p",
        LispObject::primitive("char-or-string-p"),
    );
    interp.define("booleanp", LispObject::primitive("booleanp"));
    interp.define("keywordp", LispObject::primitive("keywordp"));

    // Misc — extended (apply/error/signal handled by eval dispatch, but registered for functionp)
    interp.define("apply", LispObject::primitive("apply"));
    interp.define("error", LispObject::primitive("error"));
    interp.define("user-error", LispObject::primitive("user-error"));
    interp.define("signal", LispObject::primitive("signal"));

    // Phase 7a: state-aware primitives — semantically regular
    // functions (evaluated args) that happen to need env/macros/state
    // access. Registered on the function cell so the VM and any other
    // function-cell caller can dispatch them; `call_function` routes
    // through `call_stateful_primitive` before the regular primitive
    // dispatch. Source-level `(defalias ...)` etc. still hit the
    // special-form dispatch in eval_inner (same effect, different
    // entry point).
    interp.define("defalias", LispObject::primitive("defalias"));
    interp.define("fset", LispObject::primitive("fset"));
    interp.define("eval", LispObject::primitive("eval"));
    interp.define("funcall", LispObject::primitive("funcall"));
    interp.define("apply", LispObject::primitive("apply"));
    interp.define("put", LispObject::primitive("put"));
    interp.define("get", LispObject::primitive("get"));
}

pub fn call_primitive(name: &str, args: &LispObject) -> ElispResult<LispObject> {
    match name {
        "+" => prim_add(args),
        "-" => prim_sub(args),
        "*" => prim_mul(args),
        "/" => prim_div(args),
        "=" => prim_num_eq(args),
        "<" => prim_lt(args),
        ">" => prim_gt(args),
        "<=" => prim_le(args),
        ">=" => prim_ge(args),
        "/=" => prim_ne(args),
        "cons" => prim_cons(args),
        "car" => prim_car(args),
        "cdr" => prim_cdr(args),
        "list" => prim_list(args),
        "length" => prim_length(args),
        "append" => prim_append(args),
        "reverse" => prim_reverse(args),
        "member" => prim_member(args),
        "assoc" => prim_assoc(args),
        "eq" => prim_eq(args),
        "equal" => prim_equal(args),
        "not" => prim_not(args),
        "null" => prim_null(args),
        "symbolp" => prim_symbolp(args),
        "numberp" => prim_numberp(args),
        "listp" => prim_listp(args),
        "consp" => prim_consp(args),
        "stringp" => prim_stringp(args),
        "princ" => prim_princ(args),
        "prin1" => prim_prin1(args),
        "string=" => prim_string_eq(args),
        "string<" => prim_string_lt(args),
        "concat" => prim_concat(args),
        "substring" => prim_substring(args),

        // List operations
        "nth" => prim_nth(args),
        "nthcdr" => prim_nthcdr(args),
        "setcar" => prim_setcar(args),
        "setcdr" => prim_setcdr(args),
        "nconc" => prim_nconc(args),
        "nreverse" => prim_nreverse(args),
        "delq" => prim_delq(args),
        "memq" => prim_memq(args),
        "assq" => prim_assq(args),
        "last" => prim_last(args),
        "copy-sequence" => prim_copy_sequence(args),
        "cadr" => prim_cadr(args),
        "cddr" => prim_cddr(args),
        "caar" => prim_caar(args),
        "cdar" => prim_cdar(args),
        "car-safe" => prim_car_safe(args),
        "cdr-safe" => prim_cdr_safe(args),
        "make-list" => prim_make_list(args),

        // Type predicates
        "atom" => prim_atom(args),
        "integerp" => prim_integerp(args),
        "floatp" => prim_floatp(args),
        "zerop" => prim_zerop(args),
        "natnump" => prim_natnump(args),
        // boundp/fboundp handled by eval dispatch (need env access)
        "functionp" => prim_functionp(args),
        "subrp" => prim_subrp(args),

        // Numeric
        "1+" => prim_1_plus(args),
        "1-" => prim_1_minus(args),
        "mod" => prim_mod(args),
        "abs" => prim_abs(args),
        "max" => prim_max(args),
        "min" => prim_min(args),
        "floor" => prim_floor(args),
        "ceiling" => prim_ceiling(args),
        "round" => prim_round(args),
        "truncate" => prim_truncate(args),
        "float" => prim_float(args),
        "ash" => prim_ash(args),
        "logand" => prim_logand(args),
        "logior" => prim_logior(args),
        "lognot" => prim_lognot(args),

        // Symbol
        "symbol-name" => prim_symbol_name(args),
        // symbol-function handled by eval dispatch (needs env + macro table)

        // String
        "string-to-number" => prim_string_to_number(args),
        "number-to-string" => prim_number_to_string(args),
        "make-string" => prim_make_string(args),
        // string-match handled by eval dispatch (has regex support)

        // I/O
        "prin1-to-string" => prim_prin1_to_string(args),

        // Misc
        "identity" => prim_identity(args),
        "ignore" => prim_ignore(args),
        "type-of" => prim_type_of(args),

        // String — extended
        "upcase" => prim_upcase(args),
        "downcase" => prim_downcase(args),
        "capitalize" => prim_capitalize(args),
        "safe-length" => prim_safe_length(args),
        "read" => prim_read(args),
        "characterp" => prim_characterp(args),
        "string" => prim_string(args),
        "regexp-quote" => prim_regexp_quote(args),
        "max-char" => prim_max_char(args),
        "decode-char" => prim_decode_char(args),
        "encode-char" => prim_encode_char(args),
        "string-replace" => prim_string_replace(args),
        "string-trim" => prim_string_trim(args),
        "string-prefix-p" => prim_string_prefix_p(args),
        "string-suffix-p" => prim_string_suffix_p(args),
        "string-join" => prim_string_join(args),
        "char-to-string" => prim_char_to_string(args),
        "string-to-char" => prim_string_to_char(args),
        "string-width" => prim_string_width(args),
        "multibyte-string-p" => prim_multibyte_string_p(args),

        // Sequence — extended
        "elt" => prim_elt(args),
        "copy-alist" => prim_copy_alist(args),
        "plist-get" => prim_plist_get(args),
        "plist-put" => prim_plist_put(args),
        "plist-member" => prim_plist_member(args),
        "remove" => prim_remove(args),
        "remq" => prim_remq(args),
        "number-sequence" => prim_number_sequence(args),

        // Numeric — extended
        "random" => prim_random(args),
        "logxor" => prim_logxor(args),

        // Type — extended
        "sequencep" => prim_sequencep(args),
        "char-or-string-p" => prim_char_or_string_p(args),
        "booleanp" => prim_booleanp(args),
        "keywordp" => prim_keywordp(args),

        // Misc — extended (apply/error/signal/user-error: normally handled by eval dispatch)
        "apply" => Err(ElispError::EvalError(
            "apply must be called through eval dispatch".to_string(),
        )),
        "error" => prim_error(args),
        "user-error" => prim_user_error(args),
        "signal" => prim_signal(args),

        _ => Err(ElispError::VoidFunction(name.to_string())),
    }
}

fn get_number(obj: &LispObject) -> Option<f64> {
    match obj {
        LispObject::Integer(i) => Some(*i as f64),
        LispObject::Float(f) => Some(*f),
        _ => None,
    }
}

fn prim_add(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let sum: i64 = raw.iter().map(|a| a.as_integer().unwrap()).sum();
        Ok(LispObject::integer(sum))
    } else {
        let sum: f64 = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?
            .into_iter()
            .sum();
        Ok(LispObject::float(sum))
    }
}

fn prim_sub(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    if raw.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let ints: Vec<i64> = raw.iter().map(|a| a.as_integer().unwrap()).collect();
        let result = if ints.len() == 1 {
            -ints[0]
        } else {
            ints.iter().skip(1).fold(ints[0], |acc, &x| acc - x)
        };
        Ok(LispObject::integer(result))
    } else {
        let nums: Vec<f64> = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?;
        let result = if nums.len() == 1 {
            -nums[0]
        } else {
            nums.iter().skip(1).fold(nums[0], |acc, &x| acc - x)
        };
        Ok(LispObject::float(result))
    }
}

fn prim_mul(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let product: i64 = raw.iter().map(|a| a.as_integer().unwrap()).product();
        Ok(LispObject::integer(product))
    } else {
        let product: f64 = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?
            .into_iter()
            .product();
        Ok(LispObject::float(product))
    }
}

fn prim_div(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw_args: Vec<LispObject> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw_args.push(arg);
        current = rest;
    }
    if raw_args.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    // Validate all args are numbers
    for a in &raw_args {
        if get_number(a).is_none() {
            return Err(ElispError::WrongTypeArgument("number".to_string()));
        }
    }
    let all_integer = raw_args.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_integer {
        let ints: Vec<i64> = raw_args.iter().map(|a| a.as_integer().unwrap()).collect();
        for &d in &ints[1..] {
            if d == 0 {
                return Err(ElispError::DivisionByZero);
            }
        }
        if ints.len() == 1 {
            if ints[0] == 0 {
                return Err(ElispError::DivisionByZero);
            }
            return Ok(LispObject::integer(1 / ints[0]));
        }
        let result = ints.iter().skip(1).fold(ints[0], |acc, &x| acc / x);
        Ok(LispObject::integer(result))
    } else {
        let numbers: Vec<f64> = raw_args.iter().map(|a| get_number(a).unwrap()).collect();
        for &d in &numbers[1..] {
            if d == 0.0 {
                return Err(ElispError::DivisionByZero);
            }
        }
        if numbers.len() == 1 {
            if numbers[0] == 0.0 {
                return Err(ElispError::DivisionByZero);
            }
            return Ok(LispObject::float(1.0 / numbers[0]));
        }
        let result = numbers.iter().skip(1).fold(numbers[0], |acc, &x| acc / x);
        Ok(LispObject::float(result))
    }
}

fn prim_num_eq(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let first = numbers[0];
    Ok(LispObject::from(numbers.iter().all(|&x| x == first)))
}

fn prim_lt(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if w[0].partial_cmp(&w[1]) != Some(std::cmp::Ordering::Less) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_gt(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if w[0].partial_cmp(&w[1]) != Some(std::cmp::Ordering::Greater) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_le(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if !matches!(
            w[0].partial_cmp(&w[1]),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_ge(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if !matches!(
            w[0].partial_cmp(&w[1]),
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_ne(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let na = get_number(&a).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    let nb = get_number(&b).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::from(na != nb))
}

fn prim_cons(args: &LispObject) -> ElispResult<LispObject> {
    let car = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::cons(car, cdr))
}

fn prim_car(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(cell) => Ok(cell.lock().0.clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

fn prim_cdr(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(cell) => Ok(cell.lock().1.clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

fn prim_list(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.clone())
}

fn prim_length(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Nil => Ok(LispObject::integer(0)),
        LispObject::Cons(_) => {
            let mut count = 0;
            let mut current = arg.clone();
            while let Some((_, rest)) = current.destructure_cons() {
                count += 1;
                current = rest;
            }
            Ok(LispObject::integer(count))
        }
        LispObject::String(s) => Ok(LispObject::integer(s.chars().count() as i64)),
        LispObject::Vector(v) => Ok(LispObject::integer(v.lock().len() as i64)),
        LispObject::HashTable(ht) => Ok(LispObject::integer(ht.lock().data.len() as i64)),
        _ => Err(ElispError::WrongTypeArgument(
            "sequence (list, string, vector)".to_string(),
        )),
    }
}

fn prim_append(args: &LispObject) -> ElispResult<LispObject> {
    // Collect all arguments into a vec
    let mut all_args = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        all_args.push(arg);
        current = rest;
    }
    if all_args.is_empty() {
        return Ok(LispObject::nil());
    }
    // The last argument becomes the tail directly (even if non-list)
    let tail = all_args.pop().unwrap();
    // Collect elements from all but the last arg in reverse order
    let mut items = Vec::new();
    for arg in &all_args {
        let mut cur = arg.clone();
        while let Some((car, cdr)) = cur.destructure_cons() {
            items.push(car);
            cur = cdr;
        }
    }
    // Build result by consing items onto the tail in reverse
    let mut result = tail;
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_reverse(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = arg.clone();
    while let Some((car, cdr)) = current.destructure_cons() {
        result = LispObject::cons(car, result);
        current = cdr;
    }
    Ok(result)
}

fn prim_member(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if obj == car {
            return Ok(current);
        }
        current = cdr;
    }
    Ok(LispObject::nil())
}

fn prim_assoc(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((k, _)) = entry.destructure_cons() {
            if key == k {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn prim_eq(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match (&a, &b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        _ => false,
    };
    Ok(LispObject::from(result))
}

fn prim_equal(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(a == b))
}

fn prim_not(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_nil()))
}

fn prim_null(args: &LispObject) -> ElispResult<LispObject> {
    prim_not(args)
}

fn prim_symbolp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(
        arg.is_symbol() || arg.is_nil() || arg.is_t(),
    ))
}

fn prim_numberp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer() || arg.is_float()))
}

fn prim_listp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_nil() || arg.is_cons()))
}

fn prim_consp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_cons()))
}

fn prim_stringp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_string()))
}

fn prim_princ(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    print!("{}", arg.princ_to_string());
    Ok(arg)
}

fn prim_prin1(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    print!("{}", arg.prin1_to_string());
    Ok(arg)
}

fn prim_string_eq(args: &LispObject) -> ElispResult<LispObject> {
    let (a, b) = match (args.clone().first(), args.clone().nth(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(ElispError::WrongNumberOfArguments),
    };
    match (&a, &b) {
        (LispObject::String(s1), LispObject::String(s2)) => Ok(LispObject::from(s1 == s2)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_lt(args: &LispObject) -> ElispResult<LispObject> {
    let (a, b) = match (args.clone().first(), args.clone().nth(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(ElispError::WrongNumberOfArguments),
    };
    match (&a, &b) {
        (LispObject::String(s1), LispObject::String(s2)) => Ok(LispObject::from(s1 < s2)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_concat(args: &LispObject) -> ElispResult<LispObject> {
    // Emacs `concat` accepts any sequence of character-producing items:
    // - String: text appended verbatim.
    // - Nil: empty list, contributes nothing.
    // - List of integers: each integer pushed as a character codepoint.
    // - Vector of integers: same.
    // This matches help.el's pattern `(concat "[" (mapcar #'car alist) "]")`
    // where the middle arg is a list of character codes.
    let mut result = String::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        match arg {
            LispObject::String(s) => result.push_str(&s),
            LispObject::Nil => {}
            LispObject::Cons(_) => {
                // Treat as a list of character codepoints.
                let mut list_cur = arg;
                while let Some((car, lrest)) = list_cur.destructure_cons() {
                    match car {
                        LispObject::Integer(n) => {
                            let ch = char::from_u32(n as u32).ok_or_else(|| {
                                ElispError::WrongTypeArgument("character".to_string())
                            })?;
                            result.push(ch);
                        }
                        _ => {
                            return Err(ElispError::WrongTypeArgument(
                                "sequence of chars".to_string(),
                            ));
                        }
                    }
                    list_cur = lrest;
                }
            }
            LispObject::Vector(v) => {
                let guard = v.lock();
                for item in guard.iter() {
                    match item {
                        LispObject::Integer(n) => {
                            let ch = char::from_u32(*n as u32).ok_or_else(|| {
                                ElispError::WrongTypeArgument("character".to_string())
                            })?;
                            result.push(ch);
                        }
                        _ => {
                            return Err(ElispError::WrongTypeArgument(
                                "sequence of chars".to_string(),
                            ));
                        }
                    }
                }
            }
            _ => return Err(ElispError::WrongTypeArgument("sequence".to_string())),
        }
        current = rest;
    }
    Ok(LispObject::string(&result))
}

fn prim_substring(args: &LispObject) -> ElispResult<LispObject> {
    let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let start = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args.nth(2);

    let s = match s {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let start = match start {
        LispObject::Integer(i) => i as usize,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let end = match end {
        Some(LispObject::Integer(i)) => Some(i as usize),
        Some(_) => return Err(ElispError::WrongTypeArgument("integer".to_string())),
        None => None,
    };

    let chars: Vec<char> = s.chars().collect();
    let end_idx = end.unwrap_or(chars.len());

    if start > chars.len() || end_idx > chars.len() || start > end_idx {
        return Err(ElispError::WrongNumberOfArguments);
    }

    let result: String = chars[start..end_idx].iter().collect();
    Ok(LispObject::string(&result))
}

// ---------------------------------------------------------------------------
// List operations
// ---------------------------------------------------------------------------

fn prim_nth(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n < 0 {
        return Ok(LispObject::nil());
    }
    let mut current = list;
    for _ in 0..n {
        match current.destructure_cons() {
            Some((_, cdr)) => current = cdr,
            None => return Ok(LispObject::nil()),
        }
    }
    match current.destructure_cons() {
        Some((car, _)) => Ok(car),
        None => Ok(LispObject::nil()),
    }
}

fn prim_nthcdr(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n < 0 {
        return Ok(list);
    }
    let mut current = list;
    for _ in 0..n {
        match current.destructure_cons() {
            Some((_, cdr)) => current = cdr,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(current)
}

fn prim_setcar(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_car = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &cell {
        LispObject::Cons(_) => {
            cell.set_car(new_car.clone());
            Ok(new_car)
        }
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

fn prim_setcdr(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &cell {
        LispObject::Cons(_) => {
            cell.set_cdr(new_cdr.clone());
            Ok(new_cdr)
        }
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

fn prim_nconc(args: &LispObject) -> ElispResult<LispObject> {
    prim_append(args)
}

fn prim_nreverse(args: &LispObject) -> ElispResult<LispObject> {
    prim_reverse(args)
}

fn prim_delq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if !eq_test(&elt, &car) {
            result = LispObject::cons(car, result);
        }
        current = cdr;
    }
    prim_reverse(&LispObject::cons(result, LispObject::nil()))
}

fn prim_memq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    while let Some((car, _cdr)) = current.destructure_cons() {
        if eq_test(&elt, &car) {
            return Ok(current);
        }
        current = _cdr;
    }
    Ok(LispObject::nil())
}

fn prim_assq(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((k, _)) = entry.destructure_cons() {
            if eq_test(&key, &k) {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn prim_last(args: &LispObject) -> ElispResult<LispObject> {
    let list = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args.nth(1).and_then(|a| a.as_integer()).unwrap_or(1);
    if n <= 0 {
        // (last '(a b c) 0) => nil in Emacs
        return Ok(LispObject::nil());
    }
    // Count list length
    let mut len: i64 = 0;
    let mut current = list.clone();
    while let Some((_, cdr)) = current.destructure_cons() {
        len += 1;
        current = cdr;
    }
    let skip = (len - n).max(0);
    let mut current = list;
    for _ in 0..skip {
        if let Some((_, cdr)) = current.destructure_cons() {
            current = cdr;
        }
    }
    Ok(current)
}

fn prim_copy_sequence(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(arg.clone())
}

fn prim_cadr(args: &LispObject) -> ElispResult<LispObject> {
    // (car (cdr x))
    let cdr_args = prim_cdr(args)?;
    let wrapped = LispObject::cons(cdr_args, LispObject::nil());
    prim_car(&wrapped)
}

fn prim_cddr(args: &LispObject) -> ElispResult<LispObject> {
    // (cdr (cdr x))
    let cdr_args = prim_cdr(args)?;
    let wrapped = LispObject::cons(cdr_args, LispObject::nil());
    prim_cdr(&wrapped)
}

fn prim_caar(args: &LispObject) -> ElispResult<LispObject> {
    // (car (car x))
    let car_args = prim_car(args)?;
    let wrapped = LispObject::cons(car_args, LispObject::nil());
    prim_car(&wrapped)
}

fn prim_cdar(args: &LispObject) -> ElispResult<LispObject> {
    // (cdr (car x))
    let car_args = prim_car(args)?;
    let wrapped = LispObject::cons(car_args, LispObject::nil());
    prim_cdr(&wrapped)
}

fn prim_car_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Cons(_) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_car(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_cdr_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Cons(_) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_cdr(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_make_list(args: &LispObject) -> ElispResult<LispObject> {
    let length = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let init = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if length < 0 {
        return Ok(LispObject::nil());
    }
    let mut result = LispObject::nil();
    for _ in 0..length {
        result = LispObject::cons(init.clone(), result);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Type predicates
// ---------------------------------------------------------------------------

fn prim_atom(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(!arg.is_cons()))
}

fn prim_integerp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer()))
}

fn prim_floatp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_float()))
}

fn prim_zerop(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::from(n == 0.0))
}

fn prim_natnump(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Integer(i) => *i >= 0,
        _ => false,
    };
    Ok(LispObject::from(result))
}

fn prim_functionp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Primitive(_) => true,
        LispObject::Cons(cell) => {
            let b = cell.lock();
            if let LispObject::Symbol(id) = &b.0 {
                crate::obarray::symbol_name(*id) == "lambda"
            } else {
                false
            }
        }
        _ => false,
    };
    Ok(LispObject::from(result))
}

fn prim_subrp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_primitive()))
}

// ---------------------------------------------------------------------------
// Numeric
// ---------------------------------------------------------------------------

fn numeric_result(val: f64) -> LispObject {
    if val == val.floor() && val.abs() < 1e15 {
        LispObject::integer(val as i64)
    } else {
        LispObject::float(val)
    }
}

fn prim_1_plus(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Integer(n) => Ok(LispObject::integer(n + 1)),
        LispObject::Float(f) => Ok(LispObject::float(f + 1.0)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_1_minus(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Integer(n) => Ok(LispObject::integer(n - 1)),
        LispObject::Float(f) => Ok(LispObject::float(f - 1.0)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_mod(args: &LispObject) -> ElispResult<LispObject> {
    let x = args
        .first()
        .and_then(|a| get_number(&a))
        .ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    let y = args
        .nth(1)
        .and_then(|a| get_number(&a))
        .ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    if y == 0.0 {
        return Err(ElispError::DivisionByZero);
    }
    // Emacs mod: result has same sign as divisor
    let r = x % y;
    let result = if r != 0.0 && ((r > 0.0) != (y > 0.0)) {
        r + y
    } else {
        r
    };
    Ok(numeric_result(result))
}

fn prim_abs(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(i) => Ok(LispObject::integer(i.abs())),
        LispObject::Float(f) => Ok(LispObject::float(f.abs())),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_max(args: &LispObject) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut max_val =
        get_number(&first).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    let mut max_obj = first;
    let mut current = args.rest().unwrap_or(LispObject::nil());
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
        if n > max_val {
            max_val = n;
            max_obj = arg;
        }
        current = rest;
    }
    Ok(max_obj)
}

fn prim_min(args: &LispObject) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut min_val =
        get_number(&first).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    let mut min_obj = first;
    let mut current = args.rest().unwrap_or(LispObject::nil());
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
        if n < min_val {
            min_val = n;
            min_obj = arg;
        }
        current = rest;
    }
    Ok(min_obj)
}

fn prim_floor(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.floor() as i64))
}

fn prim_ceiling(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.ceil() as i64))
}

fn prim_round(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.round() as i64))
}

fn prim_truncate(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.trunc() as i64))
}

fn prim_float(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::float(n))
}

fn prim_ash(args: &LispObject) -> ElispResult<LispObject> {
    let value = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let count = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let result = if count >= 0 {
        value.wrapping_shl(count as u32)
    } else {
        value.wrapping_shr((-count) as u32)
    };
    Ok(LispObject::integer(result))
}

fn prim_logand(args: &LispObject) -> ElispResult<LispObject> {
    let mut result: i64 = -1; // all bits set
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = arg
            .as_integer()
            .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
        result &= n;
        current = rest;
    }
    Ok(LispObject::integer(result))
}

fn prim_logior(args: &LispObject) -> ElispResult<LispObject> {
    let mut result: i64 = 0;
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = arg
            .as_integer()
            .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
        result |= n;
        current = rest;
    }
    Ok(LispObject::integer(result))
}

fn prim_lognot(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::integer(!n))
}

// ---------------------------------------------------------------------------
// Symbol
// ---------------------------------------------------------------------------

fn prim_symbol_name(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Symbol(id) => Ok(LispObject::string(&crate::obarray::symbol_name(*id))),
        LispObject::Nil => Ok(LispObject::string("nil")),
        LispObject::T => Ok(LispObject::string("t")),
        _ => Err(ElispError::WrongTypeArgument("symbol".to_string())),
    }
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

fn prim_string_to_number(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        // Emacs returns 0 for non-string args; tolerate nil gracefully
        _ if arg.is_nil() => return Ok(LispObject::integer(0)),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    // Try integer first, then float, default to 0
    if let Ok(i) = s.trim().parse::<i64>() {
        Ok(LispObject::integer(i))
    } else if let Ok(f) = s.trim().parse::<f64>() {
        Ok(LispObject::float(f))
    } else {
        Ok(LispObject::integer(0))
    }
}

fn prim_number_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(i) => Ok(LispObject::string(&i.to_string())),
        LispObject::Float(f) => Ok(LispObject::string(&f.to_string())),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_make_string(args: &LispObject) -> ElispResult<LispObject> {
    let length = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let ch = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    if length < 0 {
        return Ok(LispObject::string(""));
    }
    let c = char::from_u32(ch as u32).unwrap_or('?');
    let s: String = std::iter::repeat_n(c, length as usize).collect();
    Ok(LispObject::string(&s))
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

fn prim_prin1_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::string(&arg.prin1_to_string()))
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

fn prim_identity(args: &LispObject) -> ElispResult<LispObject> {
    args.first().ok_or(ElispError::WrongNumberOfArguments)
}

fn prim_ignore(args: &LispObject) -> ElispResult<LispObject> {
    // Consume all args, return nil
    let _ = args;
    Ok(LispObject::nil())
}

fn prim_type_of(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let type_name = match &arg {
        LispObject::Nil => "symbol",
        LispObject::T => "symbol",
        LispObject::Symbol(_) => "symbol",
        LispObject::Integer(_) => "integer",
        LispObject::Float(_) => "float",
        LispObject::String(_) => "string",
        LispObject::Cons(_) => "cons",
        LispObject::Primitive(_) => "subr",
        LispObject::Vector(_) => "vector",
        LispObject::BytecodeFn(_) => "compiled-function",
        LispObject::HashTable(_) => "hash-table",
    };
    Ok(LispObject::symbol(type_name))
}

// ---------------------------------------------------------------------------
// Helper: eq test (identity equality for symbols/integers, pointer-like)
// ---------------------------------------------------------------------------

fn eq_test(a: &LispObject, b: &LispObject) -> bool {
    match (a, b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// String — extended
// ---------------------------------------------------------------------------

fn prim_upcase(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&s.to_uppercase())),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            let upper: String = ch.to_uppercase().collect();
            Ok(LispObject::integer(
                upper.chars().next().unwrap_or('?') as i64
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("string-or-char".to_string())),
    }
}

fn prim_downcase(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&s.to_lowercase())),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            let lower: String = ch.to_lowercase().collect();
            Ok(LispObject::integer(
                lower.chars().next().unwrap_or('?') as i64
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("string-or-char".to_string())),
    }
}

fn prim_capitalize(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => {
            // Emacs `capitalize`: title-case each word. Word boundary =
            // any non-alphanumeric char.
            let mut out = String::with_capacity(s.len());
            let mut start_of_word = true;
            for ch in s.chars() {
                if ch.is_alphanumeric() {
                    if start_of_word {
                        out.extend(ch.to_uppercase());
                        start_of_word = false;
                    } else {
                        out.extend(ch.to_lowercase());
                    }
                } else {
                    out.push(ch);
                    start_of_word = true;
                }
            }
            Ok(LispObject::string(&out))
        }
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            let upper: String = ch.to_uppercase().collect();
            Ok(LispObject::integer(
                upper.chars().next().unwrap_or('?') as i64
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("string-or-char".to_string())),
    }
}

fn prim_safe_length(args: &LispObject) -> ElispResult<LispObject> {
    // Like `length`, but returns the number of cons cells traversed
    // without signalling an error on a cyclic or dotted list. Uses
    // `destructure_cons` which clones the Arc — cheap enough for
    // loader-time use. Caps to stop cycles.
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut count: i64 = 0;
    let mut cur = arg;
    while let Some((_, rest)) = cur.destructure_cons() {
        count += 1;
        cur = rest;
        if count > 1_000_000 {
            break;
        }
    }
    Ok(LispObject::integer(count))
}

fn prim_read(args: &LispObject) -> ElispResult<LispObject> {
    // (read STRING-OR-STREAM) — we only support string input. For
    // buffer/marker streams we'd need editor state; nil args in Emacs
    // read from stdin, which we don't model. Return nil instead of
    // erroring so callers that don't care about the result survive.
    let arg = args.first().unwrap_or(LispObject::nil());
    match arg {
        LispObject::String(s) => {
            crate::reader::read(&s).map_err(|e| ElispError::EvalError(format!("read: {e}")))
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_max_char(args: &LispObject) -> ElispResult<LispObject> {
    // (max-char &optional UNICODE) — max character code in Emacs char
    // space. Emacs 30 returns #x3fffff. Argument selects Unicode-only
    // max (`#x10ffff`) when t. We return the Emacs constant for nil/no
    // arg and the Unicode constant for `t`.
    let unicode_arg = args.first().unwrap_or(LispObject::nil());
    if matches!(unicode_arg, LispObject::T) {
        Ok(LispObject::integer(0x10ffff))
    } else {
        Ok(LispObject::integer(0x3fffff))
    }
}

fn prim_regexp_quote(args: &LispObject) -> ElispResult<LispObject> {
    // (regexp-quote STRING) → STRING with all regex special chars escaped.
    // Emacs's regex engine treats these as specials: . * + ? ^ $ \ [ ]
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => {
            let mut out = String::with_capacity(s.len() + 8);
            for ch in s.chars() {
                if matches!(ch, '.' | '*' | '+' | '?' | '^' | '$' | '\\' | '[' | ']') {
                    out.push('\\');
                }
                out.push(ch);
            }
            Ok(LispObject::string(&out))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string(args: &LispObject) -> ElispResult<LispObject> {
    // (string &rest CHARS) → build a string from character codepoints.
    let mut out = String::new();
    let mut cur = args.clone();
    while let Some((car, rest)) = cur.destructure_cons() {
        match car {
            LispObject::Integer(n) => {
                let ch = char::from_u32(n as u32)
                    .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
                out.push(ch);
            }
            _ => return Err(ElispError::WrongTypeArgument("character".to_string())),
        }
        cur = rest;
    }
    Ok(LispObject::string(&out))
}

fn prim_characterp(args: &LispObject) -> ElispResult<LispObject> {
    // (characterp OBJ) → t if OBJ is a valid character. In Emacs a
    // character is a non-negative integer that is a valid Unicode
    // code point (< 0x3fffff in Emacs's char space).
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(n) if *n >= 0 && *n < 0x3fffff => Ok(LispObject::t()),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_decode_char(args: &LispObject) -> ElispResult<LispObject> {
    // (decode-char CHARSET CODE-POINT &optional RESTRICTION)
    // Proper implementation requires a charset registry. Stub: return
    // CODE-POINT as a character for the `unicode` / `ucs` charsets,
    // nil otherwise (Emacs signals nil for unsupported mappings). This
    // is enough for `characters.el` to advance past the call.
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let code = args.nth(1).unwrap_or(LispObject::nil());
    let charset_name = charset.as_symbol().unwrap_or_default();
    match (charset_name.as_str(), &code) {
        ("unicode" | "ucs", LispObject::Integer(_)) => Ok(code),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_encode_char(args: &LispObject) -> ElispResult<LispObject> {
    // (encode-char CHAR CHARSET) — inverse of decode-char. Same stub
    // strategy: pass-through for unicode/ucs, nil otherwise.
    let ch = args.first().unwrap_or(LispObject::nil());
    let charset = args.nth(1).unwrap_or(LispObject::nil());
    let charset_name = charset.as_symbol().unwrap_or_default();
    match (charset_name.as_str(), &ch) {
        ("unicode" | "ucs", LispObject::Integer(_)) => Ok(ch),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_string_replace(args: &LispObject) -> ElispResult<LispObject> {
    let from = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let to = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let in_str = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&from, &to, &in_str) {
        (LispObject::String(f), LispObject::String(t), LispObject::String(s)) => {
            Ok(LispObject::string(&s.replace(f.as_str(), t.as_str())))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_trim(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(s.trim())),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_prefix_p(args: &LispObject) -> ElispResult<LispObject> {
    let prefix = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&prefix, &s) {
        (LispObject::String(p), LispObject::String(s)) => {
            Ok(LispObject::from(s.starts_with(p.as_str())))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_suffix_p(args: &LispObject) -> ElispResult<LispObject> {
    let suffix = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&suffix, &s) {
        (LispObject::String(sfx), LispObject::String(s)) => {
            Ok(LispObject::from(s.ends_with(sfx.as_str())))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_join(args: &LispObject) -> ElispResult<LispObject> {
    let strings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let separator = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .unwrap_or_default();
    let mut parts = Vec::new();
    let mut current = strings;
    while let Some((car, cdr)) = current.destructure_cons() {
        match &car {
            LispObject::String(s) => parts.push(s.clone()),
            _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
        }
        current = cdr;
    }
    Ok(LispObject::string(&parts.join(&separator)))
}

fn prim_char_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let code = arg
        .as_integer()
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let ch = char::from_u32(code as u32).unwrap_or('?');
    Ok(LispObject::string(&ch.to_string()))
}

fn prim_string_to_char(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => {
            let ch = s.chars().next().unwrap_or('\0');
            Ok(LispObject::integer(ch as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_width(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::integer(s.chars().count() as i64)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_multibyte_string_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(_) => Ok(LispObject::t()),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

// ---------------------------------------------------------------------------
// Sequence — extended
// ---------------------------------------------------------------------------

fn prim_elt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    if n < 0 {
        return Err(ElispError::WrongTypeArgument("natnump".to_string()));
    }
    let idx = n as usize;
    match &seq {
        LispObject::String(s) => {
            let ch = s.chars().nth(idx);
            match ch {
                Some(c) => Ok(LispObject::integer(c as i64)),
                None => Err(ElispError::WrongTypeArgument(
                    "args-out-of-range".to_string(),
                )),
            }
        }
        LispObject::Vector(v) => {
            let v = v.lock();
            v.get(idx).cloned().ok_or(ElispError::WrongTypeArgument(
                "args-out-of-range".to_string(),
            ))
        }
        LispObject::Nil | LispObject::Cons(_) => {
            // Walk the list
            let mut current = seq.clone();
            for _ in 0..idx {
                match current.destructure_cons() {
                    Some((_, cdr)) => current = cdr,
                    None => {
                        return Err(ElispError::WrongTypeArgument(
                            "args-out-of-range".to_string(),
                        ))
                    }
                }
            }
            current.first().ok_or(ElispError::WrongTypeArgument(
                "args-out-of-range".to_string(),
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("sequencep".to_string())),
    }
}

fn prim_copy_alist(args: &LispObject) -> ElispResult<LispObject> {
    let alist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        let copied = if let Some((k, v)) = entry.destructure_cons() {
            LispObject::cons(k, v)
        } else {
            entry
        };
        items.push(copied);
        current = rest;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_plist_get(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = plist;
    while let Some((key, rest)) = current.destructure_cons() {
        if eq_test(&key, &prop) {
            return rest.first().ok_or(ElispError::WrongNumberOfArguments);
        }
        // Skip value
        match rest.destructure_cons() {
            Some((_, next)) => current = next,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(LispObject::nil())
}

fn prim_plist_put(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    // Build a new plist with the property set
    let mut items = Vec::new();
    let mut current = plist.clone();
    let mut found = false;
    while let Some((key, rest)) = current.destructure_cons() {
        if let Some((val, next)) = rest.destructure_cons() {
            if eq_test(&key, &prop) {
                items.push((key, value.clone()));
                found = true;
                current = next;
            } else {
                items.push((key, val));
                current = next;
            }
        } else {
            break;
        }
    }
    if !found {
        items.push((prop, value));
    }
    let mut result = LispObject::nil();
    for (k, v) in items.into_iter().rev() {
        result = LispObject::cons(v, result);
        result = LispObject::cons(k, result);
    }
    Ok(result)
}

fn prim_plist_member(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = plist;
    while let Some((key, _rest)) = current.destructure_cons() {
        if eq_test(&key, &prop) {
            return Ok(current);
        }
        // Skip value, advance to next key
        match _rest.destructure_cons() {
            Some((_, next)) => current = next,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(LispObject::nil())
}

fn prim_remove(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if car != elt {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_remq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if !eq_test(&elt, &car) {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_number_sequence(args: &LispObject) -> ElispResult<LispObject> {
    let from = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let to = args.nth(1).and_then(|a| a.as_integer());
    let step = args.nth(2).and_then(|a| a.as_integer());

    // (number-sequence FROM) => (FROM)
    let to = match to {
        Some(t) => t,
        None => {
            return Ok(LispObject::cons(
                LispObject::integer(from),
                LispObject::nil(),
            ))
        }
    };
    let step = step.unwrap_or(if from <= to { 1 } else { -1 });
    if step == 0 {
        return Err(ElispError::InvalidOperation(
            "number-sequence step must be non-zero".to_string(),
        ));
    }
    let mut items = Vec::new();
    let mut i = from;
    if step > 0 {
        while i <= to {
            items.push(LispObject::integer(i));
            i += step;
        }
    } else {
        while i >= to {
            items.push(LispObject::integer(i));
            i += step;
        }
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Numeric — extended
// ---------------------------------------------------------------------------

/// Simple LCG-based pseudo-random number generator state.
static RANDOM_STATE: AtomicI64 = AtomicI64::new(0);

fn prim_random(args: &LispObject) -> ElispResult<LispObject> {
    let limit = args.first();
    // Advance the LCG state
    let old = RANDOM_STATE.fetch_add(1, Ordering::Relaxed);
    // LCG: a=6364136223846793005, c=1442695040888963407 (Knuth)
    let raw = old
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let raw = raw.unsigned_abs() as i64; // ensure non-negative
    match limit {
        Some(LispObject::Integer(n)) if n > 0 => Ok(LispObject::integer(raw % n)),
        Some(LispObject::Integer(_)) => Err(ElispError::WrongTypeArgument(
            "positive integer".to_string(),
        )),
        Some(LispObject::T) => {
            // (random t) reseeds — use current time nanos
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(42);
            RANDOM_STATE.store(seed, Ordering::Relaxed);
            Ok(LispObject::integer(seed.unsigned_abs() as i64))
        }
        _ => Ok(LispObject::integer(raw)),
    }
}

fn prim_logxor(args: &LispObject) -> ElispResult<LispObject> {
    let mut result: i64 = 0;
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = arg
            .as_integer()
            .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
        result ^= n;
        current = rest;
    }
    Ok(LispObject::integer(result))
}

// ---------------------------------------------------------------------------
// Type — extended
// ---------------------------------------------------------------------------

fn prim_sequencep(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(
        arg,
        LispObject::Nil | LispObject::Cons(_) | LispObject::Vector(_) | LispObject::String(_)
    );
    Ok(LispObject::from(result))
}

fn prim_char_or_string_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::String(_) | LispObject::Integer(_));
    Ok(LispObject::from(result))
}

fn prim_booleanp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::Nil | LispObject::T);
    Ok(LispObject::from(result))
}

fn prim_keywordp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id).starts_with(':'),
        _ => false,
    };
    Ok(LispObject::from(result))
}

// ---------------------------------------------------------------------------
// Misc — extended
// ---------------------------------------------------------------------------

fn prim_error(args: &LispObject) -> ElispResult<LispObject> {
    let msg = args
        .first()
        .map(|a| a.princ_to_string())
        .unwrap_or_default();
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}

fn prim_user_error(args: &LispObject) -> ElispResult<LispObject> {
    let msg = args
        .first()
        .map(|a| a.princ_to_string())
        .unwrap_or_default();
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("user-error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}

fn prim_signal(args: &LispObject) -> ElispResult<LispObject> {
    let symbol = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data = args.nth(1).unwrap_or(LispObject::nil());
    Err(ElispError::Signal(Box::new(SignalData { symbol, data })))
}

impl From<bool> for LispObject {
    fn from(b: bool) -> LispObject {
        if b {
            LispObject::t()
        } else {
            LispObject::nil()
        }
    }
}
