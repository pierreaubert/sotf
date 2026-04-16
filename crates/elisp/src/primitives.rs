use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

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
    interp.define("boundp", LispObject::primitive("boundp"));
    interp.define("fboundp", LispObject::primitive("fboundp"));
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
    interp.define("symbol-function", LispObject::primitive("symbol-function"));

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
    interp.define("string-match", LispObject::primitive("string-match"));

    // New primitives — I/O
    interp.define("prin1-to-string", LispObject::primitive("prin1-to-string"));

    // New primitives — misc
    interp.define("identity", LispObject::primitive("identity"));
    interp.define("ignore", LispObject::primitive("ignore"));
    interp.define("type-of", LispObject::primitive("type-of"));
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
        "boundp" => prim_boundp(args),
        "fboundp" => prim_fboundp(args),
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
        "symbol-function" => prim_symbol_function(args),

        // String
        "string-to-number" => prim_string_to_number(args),
        "number-to-string" => prim_number_to_string(args),
        "make-string" => prim_make_string(args),
        "string-match" => prim_string_match(args),

        // I/O
        "prin1-to-string" => prim_prin1_to_string(args),

        // Misc
        "identity" => prim_identity(args),
        "ignore" => prim_ignore(args),
        "type-of" => prim_type_of(args),

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
    Ok(LispObject::from(
        numbers.iter().all(|&x| (x - first).abs() < 1e-10),
    ))
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
    match arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(car, _) => Ok((*car).clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

fn prim_cdr(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(_, cdr) => Ok((*cdr).clone()),
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
        LispObject::Cons(_, _) => {
            let mut count = 0;
            let mut current = arg.clone();
            while let Some((_, rest)) = current.destructure_cons() {
                count += 1;
                current = rest;
            }
            Ok(LispObject::integer(count))
        }
        LispObject::String(s) => Ok(LispObject::integer(s.len() as i64)),
        _ => Err(ElispError::WrongTypeArgument("list or string".to_string())),
    }
}

fn prim_append(args: &LispObject) -> ElispResult<LispObject> {
    let mut result = LispObject::nil();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let mut arg_list = arg;
        while let Some((car, cdr)) = arg_list.destructure_cons() {
            result = LispObject::cons(car, result);
            arg_list = cdr;
        }
        current = rest;
    }
    prim_reverse(&LispObject::cons(result, LispObject::nil()))
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
    let (obj, list) = args.clone().destructure();
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
    let (key, alist) = args.clone().destructure();
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
    let mut result = String::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        match arg {
            LispObject::String(s) => result.push_str(&s),
            LispObject::Integer(i) => result.push_str(&i.to_string()),
            LispObject::Symbol(s) => result.push_str(&s),
            _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
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
    match cell {
        LispObject::Cons(_, cdr) => Ok(LispObject::cons(new_car, (*cdr).clone())),
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

fn prim_setcdr(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match cell {
        LispObject::Cons(car, _) => Ok(LispObject::cons((*car).clone(), new_cdr)),
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
    match arg {
        LispObject::Cons(_, _) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_car(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_cdr_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Cons(_, _) => {
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

fn prim_boundp(args: &LispObject) -> ElispResult<LispObject> {
    let _arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    // Stub: always true (we don't have void distinction yet)
    Ok(LispObject::t())
}

fn prim_fboundp(args: &LispObject) -> ElispResult<LispObject> {
    let _arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    // Stub: always true
    Ok(LispObject::t())
}

fn prim_functionp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Primitive(_) => true,
        LispObject::Cons(car, _) => matches!(car.as_ref(), LispObject::Symbol(s) if s == "lambda"),
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
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(numeric_result(n + 1.0))
}

fn prim_1_minus(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(numeric_result(n - 1.0))
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
        LispObject::Symbol(s) => Ok(LispObject::string(s)),
        LispObject::Nil => Ok(LispObject::string("nil")),
        LispObject::T => Ok(LispObject::string("t")),
        _ => Err(ElispError::WrongTypeArgument("symbol".to_string())),
    }
}

fn prim_symbol_function(args: &LispObject) -> ElispResult<LispObject> {
    // Stub: in our Lisp-1, just return the argument itself.
    // A real Lisp-2 would look up the function cell.
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(arg)
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

fn prim_string_to_number(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
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
    let s: String = std::iter::repeat(c).take(length as usize).collect();
    Ok(LispObject::string(&s))
}

fn prim_string_match(args: &LispObject) -> ElispResult<LispObject> {
    // Stub: always returns nil
    let _regexp = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let _string = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::nil())
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
        LispObject::Cons(_, _) => "cons",
        LispObject::Primitive(_) => "subr",
        LispObject::Vector(_) => "vector",
        LispObject::BytecodeFn(_) => "compiled-function",
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

impl From<bool> for LispObject {
    fn from(b: bool) -> LispObject {
        if b {
            LispObject::t()
        } else {
            LispObject::nil()
        }
    }
}
