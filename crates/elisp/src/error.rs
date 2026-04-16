use crate::object::LispObject;

#[derive(Debug, Clone)]
pub struct ThrowData {
    pub tag: LispObject,
    pub value: LispObject,
}

#[derive(Debug, Clone)]
pub struct SignalData {
    pub symbol: LispObject,
    pub data: LispObject,
}

#[derive(Debug, Clone)]
pub enum ElispError {
    VoidFunction(String),
    VoidVariable(String),
    WrongTypeArgument(String),
    WrongNumberOfArguments,
    SyntaxError(String),
    ReaderError(String),
    UnboundValue(String),
    InvalidOperation(String),
    FileError {
        operation: String,
        path: String,
        message: String,
    },
    DivisionByZero,
    StackOverflow,
    EvalError(String),
    /// Non-local exit via (throw TAG VALUE)
    Throw(Box<ThrowData>),
    /// Emacs-style error signal via (signal ERROR-SYMBOL DATA)
    Signal(Box<SignalData>),
}

pub type ElispResult<T> = Result<T, ElispError>;

impl ElispError {
    /// Convert a Rust-side error into an Emacs signal with proper error symbol.
    pub fn to_signal(&self) -> ElispError {
        match self {
            ElispError::Signal(..) | ElispError::Throw(..) => self.clone(),
            ElispError::VoidFunction(name) => ElispError::Signal(Box::new(SignalData {
                symbol: LispObject::symbol("void-function"),
                data: LispObject::cons(LispObject::symbol(name), LispObject::nil()),
            })),
            ElispError::VoidVariable(name) => ElispError::Signal(Box::new(SignalData {
                symbol: LispObject::symbol("void-variable"),
                data: LispObject::cons(LispObject::symbol(name), LispObject::nil()),
            })),
            ElispError::WrongTypeArgument(expected) => ElispError::Signal(Box::new(SignalData {
                symbol: LispObject::symbol("wrong-type-argument"),
                data: LispObject::cons(LispObject::string(expected), LispObject::nil()),
            })),
            ElispError::WrongNumberOfArguments => ElispError::Signal(Box::new(SignalData {
                symbol: LispObject::symbol("wrong-number-of-arguments"),
                data: LispObject::nil(),
            })),
            ElispError::DivisionByZero => ElispError::Signal(Box::new(SignalData {
                symbol: LispObject::symbol("arith-error"),
                data: LispObject::nil(),
            })),
            _ => ElispError::Signal(Box::new(SignalData {
                symbol: LispObject::symbol("error"),
                data: LispObject::cons(LispObject::string(&self.to_string()), LispObject::nil()),
            })),
        }
    }

    /// Check if this error matches a condition name for condition-case.
    pub fn matches_condition(&self, condition: &LispObject) -> bool {
        let sym = match condition.as_symbol() {
            Some(s) => s,
            None => return false,
        };
        // 'error' matches everything (except throw)
        if sym == "error" {
            return !matches!(self, ElispError::Throw(..));
        }
        // Match specific error symbols
        let signal = self.to_signal();
        if let ElispError::Signal(sig) = &signal {
            if let Some(s) = sig.symbol.as_symbol() {
                return s == sym;
            }
        }
        false
    }
}

impl std::fmt::Display for ElispError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElispError::VoidFunction(name) => write!(f, "void function: {}", name),
            ElispError::VoidVariable(name) => write!(f, "void variable: {}", name),
            ElispError::WrongTypeArgument(expected) => {
                write!(f, "wrong type argument: expected {}", expected)
            }
            ElispError::WrongNumberOfArguments => {
                write!(f, "wrong number of arguments")
            }
            ElispError::SyntaxError(msg) => write!(f, "syntax error: {}", msg),
            ElispError::ReaderError(msg) => write!(f, "reader error: {}", msg),
            ElispError::UnboundValue(name) => write!(f, "unbound value: {}", name),
            ElispError::InvalidOperation(msg) => write!(f, "invalid operation: {}", msg),
            ElispError::FileError {
                operation,
                path,
                message,
            } => {
                write!(f, "file error: {} '{}' - {}", operation, path, message)
            }
            ElispError::DivisionByZero => write!(f, "division by zero"),
            ElispError::StackOverflow => write!(f, "stack overflow (possible infinite recursion)"),
            ElispError::EvalError(msg) => write!(f, "evaluation error: {}", msg),
            ElispError::Throw(throw_data) => {
                write!(
                    f,
                    "no catch for tag: {} with value: {}",
                    throw_data.tag.prin1_to_string(),
                    throw_data.value.prin1_to_string()
                )
            }
            ElispError::Signal(signal_data) => {
                write!(
                    f,
                    "{}: {}",
                    signal_data.symbol.prin1_to_string(),
                    signal_data.data.prin1_to_string()
                )
            }
        }
    }
}

impl std::error::Error for ElispError {}
