use std::cell::RefCell;
use std::fmt;

/// Prefix for all NaN-boxed tagged values: negative quiet NaN.
const NANBOX_PREFIX: u64 = 0xFFF8_0000_0000_0000;

/// Bits 48..51 encode the tag.
const TAG_SHIFT: u64 = 48;

/// 4-bit tag mask (applied after shifting).
const TAG_MASK: u64 = 0xF;

/// Lower 48 bits carry the payload.
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Tag values.
//
// Phase 3 reclaimed tag 1 (previously `TAG_GC_PTR`, a legacy
// side-table index used by the VM and pre-Phase-2m main interpreter).
// It's currently unused; reserved for future use (dedicated Bignum
// tag, extension types, etc.).
const TAG_FIXNUM: u64 = 0;
// tag 1 — reserved (was TAG_GC_PTR, retired in Phase 3)
const TAG_SYMBOL: u64 = 2;
const TAG_CHAR: u64 = 3;
const TAG_SPECIAL: u64 = 4;
const TAG_SUBR: u64 = 5;
/// Real GC-heap pointer. The payload is a `*mut GcHeader` address
/// (low 48 bits) returned from `Heap::cons_value` and friends. Objects
/// behind this tag participate in mark-and-sweep tracing.
const TAG_HEAP_PTR: u64 = 6;

// Special-tag payloads
const SPECIAL_NIL: u64 = 0;
const SPECIAL_T: u64 = 1;
const SPECIAL_UNBOUND: u64 = 2;

/// Sign bit for 48-bit integers (bit 47).
const FIXNUM_SIGN_BIT: u64 = 1 << 47;

/// Mask for valid 48-bit fixnum magnitude.
const FIXNUM_MAX: i64 = (1_i64 << 47) - 1;
const FIXNUM_MIN: i64 = -(1_i64 << 47);

/// A NaN-boxed Lisp value. Always 64 bits. Copy, not Clone.
///
/// Encoding: when the raw bits form a valid IEEE 754 double (including
/// positive NaN and infinities), the value IS a float.  When the top 16
/// bits equal `0xFFF8` plus a 4-bit tag, the lower 48 bits are a tagged
/// immediate or pointer.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Value(u64);

impl Value {
    // ---- constructors ----

    /// The canonical nil value.
    #[inline]
    pub fn nil() -> Self {
        Self(NANBOX_PREFIX | (TAG_SPECIAL << TAG_SHIFT) | SPECIAL_NIL)
    }

    /// The canonical t (true) value.
    #[inline]
    pub fn t() -> Self {
        Self(NANBOX_PREFIX | (TAG_SPECIAL << TAG_SHIFT) | SPECIAL_T)
    }

    /// The unbound marker (internal use).
    #[inline]
    pub fn unbound() -> Self {
        Self(NANBOX_PREFIX | (TAG_SPECIAL << TAG_SHIFT) | SPECIAL_UNBOUND)
    }

    /// Pack a signed integer into 48 bits.
    ///
    /// # Panics
    /// Panics if `n` does not fit in 48 bits (outside -2^47 .. 2^47-1).
    #[inline]
    pub fn fixnum(n: i64) -> Self {
        assert!(
            (FIXNUM_MIN..=FIXNUM_MAX).contains(&n),
            "fixnum {n} out of 48-bit range"
        );
        let payload = (n as u64) & PAYLOAD_MASK;
        Self(NANBOX_PREFIX | (TAG_FIXNUM << TAG_SHIFT) | payload)
    }

    /// Store an IEEE 754 double directly. The raw bits become the Value.
    #[inline]
    pub fn float(f: f64) -> Self {
        Self(f.to_bits())
    }

    /// Store a symbol table index (up to 2^48 - 1 symbols).
    #[inline]
    pub fn symbol_id(id: u32) -> Self {
        Self(NANBOX_PREFIX | (TAG_SYMBOL << TAG_SHIFT) | (id as u64))
    }

    /// Store a Unicode scalar value (char, up to 32 bits).
    #[inline]
    pub fn character(ch: char) -> Self {
        Self(NANBOX_PREFIX | (TAG_CHAR << TAG_SHIFT) | (ch as u64))
    }

    /// Store a builtin function (subr) index.
    #[inline]
    pub fn subr(index: u32) -> Self {
        Self(NANBOX_PREFIX | (TAG_SUBR << TAG_SHIFT) | (index as u64))
    }

    /// Construct a real GC-heap pointer Value (TAG_HEAP_PTR).
    ///
    /// `ptr` must fit in 48 bits, which is true for all user-space pointers
    /// on every supported target (x86_64, aarch64).
    #[inline]
    pub fn heap_ptr(ptr: *const u8) -> Self {
        let addr = ptr as u64;
        debug_assert!(
            addr & !PAYLOAD_MASK == 0,
            "heap pointer does not fit in 48 bits"
        );
        Self(NANBOX_PREFIX | (TAG_HEAP_PTR << TAG_SHIFT) | (addr & PAYLOAD_MASK))
    }

    // ---- predicates ----

    /// True when the value is a tagged value with the given tag.
    #[inline]
    fn has_tag(self, tag: u64) -> bool {
        let expected = NANBOX_PREFIX | (tag << TAG_SHIFT);
        let mask = NANBOX_PREFIX | (TAG_MASK << TAG_SHIFT);
        (self.0 & mask) == expected
    }

    #[inline]
    pub fn is_nil(self) -> bool {
        self.0 == Self::nil().0
    }

    #[inline]
    pub fn is_t(self) -> bool {
        self.0 == Self::t().0
    }

    #[inline]
    pub fn is_unbound(self) -> bool {
        self.0 == Self::unbound().0
    }

    #[inline]
    pub fn is_fixnum(self) -> bool {
        self.has_tag(TAG_FIXNUM)
    }

    /// A value is a float when it is NOT one of our NaN-boxed tagged values.
    /// This includes normal doubles, infinities, and positive NaN.
    #[inline]
    pub fn is_float(self) -> bool {
        // Our tagged values all have the prefix 0xFFF8 in the top 16 bits.
        // A value is a float if its top 16 bits do NOT match 0xFFF8..0xFFFF
        // with the tag pattern, i.e. it is not nanboxed.
        !self.is_nanboxed_value()
    }

    /// Internal: strict check that this is one of our tagged values.
    #[inline]
    fn is_nanboxed_value(self) -> bool {
        // Top 16 bits must be 0xFFF8 | (tag << 0) where tag is 0..15.
        // Equivalently: top 13 bits are all 1 (bits 63..51).
        (self.0 >> 51) == 0x1FFF
    }

    /// True when this Value holds a real GC-heap pointer (TAG_HEAP_PTR).
    #[inline]
    pub fn is_heap_ptr(self) -> bool {
        self.has_tag(TAG_HEAP_PTR)
    }

    #[inline]
    pub fn is_symbol(self) -> bool {
        self.has_tag(TAG_SYMBOL)
    }

    #[inline]
    pub fn is_char(self) -> bool {
        self.has_tag(TAG_CHAR)
    }

    #[inline]
    pub fn is_subr(self) -> bool {
        self.has_tag(TAG_SUBR)
    }

    // ---- accessors ----

    /// Extract the tag (0..15).  Returns 0xFF for bare floats.
    ///
    /// The quiet-NaN bit (bit 51) is part of the NaN prefix, not the tag.
    /// The raw nibble at bits 48..51 is `8 | tag`, so we mask with 0x7.
    #[inline]
    pub fn tag(self) -> u8 {
        if self.is_nanboxed_value() {
            ((self.0 >> TAG_SHIFT) & 0x7) as u8
        } else {
            0xFF
        }
    }

    /// Extract the 48-bit payload (only meaningful for tagged values).
    #[inline]
    fn payload(self) -> u64 {
        self.0 & PAYLOAD_MASK
    }

    /// Decode a fixnum, sign-extending from 48 bits.
    #[inline]
    pub fn as_fixnum(self) -> Option<i64> {
        if !self.is_fixnum() {
            return None;
        }
        let raw = self.payload();
        // Sign-extend bit 47 into bits 48..63
        let extended = if raw & FIXNUM_SIGN_BIT != 0 {
            raw | !PAYLOAD_MASK // set upper bits
        } else {
            raw
        };
        Some(extended as i64)
    }

    /// Decode a float.  Returns `None` for tagged values.
    #[inline]
    pub fn as_float(self) -> Option<f64> {
        if self.is_nanboxed_value() {
            None
        } else {
            Some(f64::from_bits(self.0))
        }
    }

    /// Extract the heap pointer payload (TAG_HEAP_PTR only).
    /// Returns `None` for all other tags.
    #[inline]
    pub fn as_heap_ptr(self) -> Option<*mut u8> {
        if !self.is_heap_ptr() {
            return None;
        }
        Some(self.payload() as *mut u8)
    }

    /// Extract a symbol table index.
    #[inline]
    pub fn as_symbol_id(self) -> Option<u32> {
        if !self.is_symbol() {
            return None;
        }
        Some(self.payload() as u32)
    }

    /// Extract a character.
    #[inline]
    pub fn as_char(self) -> Option<char> {
        if !self.is_char() {
            return None;
        }
        char::from_u32(self.payload() as u32)
    }

    /// Extract a subr index.
    #[inline]
    pub fn as_subr(self) -> Option<u32> {
        if !self.is_subr() {
            return None;
        }
        Some(self.payload() as u32)
    }

    /// Bitwise equality (Lisp `eq`).
    #[inline]
    pub fn lisp_eq(self, other: Value) -> bool {
        self.0 == other.0
    }

    /// Raw bits (useful for hashing, debugging, JIT interop).
    #[inline]
    pub fn raw(self) -> u64 {
        self.0
    }

    /// Construct from raw bits (JIT interop).
    #[inline]
    pub fn from_raw(bits: u64) -> Self {
        Self(bits)
    }

    /// Alias for raw() — kept for backward compatibility.
    #[inline]
    pub fn to_bits(self) -> u64 {
        self.0
    }

    /// Construct nil or t from a boolean.
    #[inline]
    pub fn from_bool(b: bool) -> Self {
        if b {
            Self::t()
        } else {
            Self::nil()
        }
    }

    // ---- cons cell access ----

    /// True when this Value is a cons pointer (either `ObjectTag::Cons`
    /// — native Value-based cell — or `ObjectTag::ConsArc` —
    /// Arc-wrapping variant).
    ///
    /// # Safety
    /// This method dereferences the Value's heap pointer to read the
    /// `GcHeader.tag`. The Value MUST be live — i.e. not referring to
    /// a heap object that has been swept. In practice every Value
    /// reachable from the interpreter's stack, env, or root stack
    /// satisfies this invariant. A swept-and-reused Value would
    /// produce a misleading answer (or UB, depending on what replaced
    /// the freed memory). Callers that can't guarantee liveness
    /// should stick to `is_heap_ptr`.
    #[inline]
    pub fn is_cons(&self) -> bool {
        let Some(ptr) = self.as_heap_ptr() else {
            return false;
        };
        // SAFETY: the caller's contract guarantees the Value is live
        // (see method doc). `TAG_HEAP_PTR` always points at a
        // `GcHeader`-prefixed object, so reading `(*header).tag` is
        // well-defined when the object is alive.
        let tag = unsafe { (*(ptr as *const crate::gc::GcHeader)).tag };
        matches!(
            tag,
            crate::gc::ObjectTag::Cons | crate::gc::ObjectTag::ConsArc
        )
    }

    /// True when this Value is a proper list (nil or cons).
    #[inline]
    pub fn is_list(&self) -> bool {
        self.is_nil() || self.is_cons()
    }

    /// Get the car of a real heap-allocated cons cell (TAG_HEAP_PTR).
    ///
    /// # Safety
    /// The Value must be a live TAG_HEAP_PTR pointer returned by
    /// `Heap::cons_value`. The caller must ensure the cell has not been
    /// collected. Returns `None` for any other tag (including side-table
    /// TAG_GC_PTR indices, which are not real pointers).
    #[inline]
    pub unsafe fn cons_car(self) -> Option<Value> {
        let ptr = self.as_heap_ptr()? as *const crate::gc::ConsCell;
        // SAFETY: caller guarantees the pointer is live.
        Some(unsafe { Value::from_raw((*ptr).car) })
    }

    /// Get the cdr of a real heap-allocated cons cell (TAG_HEAP_PTR).
    ///
    /// # Safety
    /// See `cons_car`.
    #[inline]
    pub unsafe fn cons_cdr(self) -> Option<Value> {
        let ptr = self.as_heap_ptr()? as *const crate::gc::ConsCell;
        // SAFETY: caller guarantees the pointer is live.
        Some(unsafe { Value::from_raw((*ptr).cdr) })
    }

    /// Invoke `visit` once for each real heap pointer this Value holds.
    ///
    /// Used by the GC mark phase to enumerate reachable objects.
    /// Immediates (fixnum, float, nil/t, symbol, char, subr) and legacy
    /// TAG_GC_PTR side-table indices visit nothing — only TAG_HEAP_PTR
    /// payloads are real `GcHeader` pointers.
    #[inline]
    pub fn trace(self, mut visit: impl FnMut(*mut crate::gc::GcHeader)) {
        if let Some(ptr) = self.as_heap_ptr() {
            visit(ptr as *mut crate::gc::GcHeader);
        }
    }

    // ---- arithmetic fast path ----
    //
    // These methods operate directly on NaN-boxed values without converting
    // through LispObject. They handle the fixnum x fixnum hot path inline
    // and promote to float when either operand is a float. Returns None
    // for non-numeric operands so the caller can fall back to the full
    // LispObject path.

    /// Extract a numeric value as f64 (works for both fixnum and float).
    #[inline]
    fn as_number(self) -> Option<f64> {
        if let Some(n) = self.as_fixnum() {
            Some(n as f64)
        } else {
            self.as_float()
        }
    }

    /// Add two Values. Returns `Some(result)` for numeric operands.
    #[inline]
    pub fn arith_add(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::fixnum(a.wrapping_add(b)));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::float(a + b))
    }

    /// Subtract `other` from `self`.
    #[inline]
    pub fn arith_sub(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::fixnum(a.wrapping_sub(b)));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::float(a - b))
    }

    /// Multiply two Values.
    #[inline]
    pub fn arith_mul(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::fixnum(a.wrapping_mul(b)));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::float(a * b))
    }

    /// Negate a numeric Value.
    #[inline]
    pub fn negate(self) -> Option<Value> {
        if let Some(n) = self.as_fixnum() {
            return Some(Value::fixnum(-n));
        }
        if let Some(f) = self.as_float() {
            return Some(Value::float(-f));
        }
        None
    }

    /// Increment by 1 (add1).
    #[inline]
    pub fn add1(self) -> Option<Value> {
        if let Some(n) = self.as_fixnum() {
            return Some(Value::fixnum(n.wrapping_add(1)));
        }
        if let Some(f) = self.as_float() {
            return Some(Value::float(f + 1.0));
        }
        None
    }

    /// Decrement by 1 (sub1).
    #[inline]
    pub fn sub1(self) -> Option<Value> {
        if let Some(n) = self.as_fixnum() {
            return Some(Value::fixnum(n.wrapping_sub(1)));
        }
        if let Some(f) = self.as_float() {
            return Some(Value::float(f - 1.0));
        }
        None
    }

    /// Numeric equality (Lisp `=`).
    #[inline]
    pub fn num_eq(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::from_bool(a == b));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::from_bool(a == b))
    }

    /// Less than (Lisp `<`).
    #[inline]
    pub fn lt(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::from_bool(a < b));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::from_bool(a < b))
    }

    /// Greater than (Lisp `>`).
    #[inline]
    pub fn gt(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::from_bool(a > b));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::from_bool(a > b))
    }

    /// Less than or equal (Lisp `<=`).
    #[inline]
    pub fn leq(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::from_bool(a <= b));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::from_bool(a <= b))
    }

    /// Greater than or equal (Lisp `>=`).
    #[inline]
    pub fn geq(self, other: Value) -> Option<Value> {
        if let (Some(a), Some(b)) = (self.as_fixnum(), other.as_fixnum()) {
            return Some(Value::from_bool(a >= b));
        }
        let a = self.as_number()?;
        let b = other.as_number()?;
        Some(Value::from_bool(a >= b))
    }
}

impl PartialEq for Value {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Value {}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_nil() {
            write!(f, "nil")
        } else if self.is_t() {
            write!(f, "t")
        } else if self.is_unbound() {
            write!(f, "#<unbound>")
        } else if self.is_fixnum() {
            write!(f, "{}", self.as_fixnum().unwrap())
        } else if self.is_float() {
            write!(f, "{}", self.as_float().unwrap())
        } else if self.is_symbol() {
            write!(f, "#<symbol {}>", self.as_symbol_id().unwrap())
        } else if self.is_char() {
            write!(f, "?{}", self.as_char().unwrap_or('\u{FFFD}'))
        } else if self.is_heap_ptr() {
            write!(f, "#<heap-ptr {:p}>", self.payload() as *const u8)
        } else if self.is_subr() {
            write!(f, "#<subr {}>", self.as_subr().unwrap())
        } else {
            write!(f, "#<unknown 0x{:016X}>", self.0)
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_of_value_is_8_bytes() {
        assert_eq!(std::mem::size_of::<Value>(), 8);
    }

    #[test]
    fn nil_is_nil_not_fixnum_not_float() {
        let v = Value::nil();
        assert!(v.is_nil());
        assert!(!v.is_fixnum());
        assert!(!v.is_float());
        assert!(!v.is_t());
        assert!(!v.is_symbol());
        assert!(!v.is_char());
    }

    #[test]
    fn t_is_t() {
        let v = Value::t();
        assert!(v.is_t());
        assert!(!v.is_nil());
        assert!(!v.is_fixnum());
        assert!(!v.is_float());
    }

    #[test]
    fn unbound_is_unbound() {
        let v = Value::unbound();
        assert!(v.is_unbound());
        assert!(!v.is_nil());
        assert!(!v.is_t());
    }

    #[test]
    fn fixnum_roundtrip_zero() {
        let v = Value::fixnum(0);
        assert!(v.is_fixnum());
        assert!(!v.is_float());
        assert_eq!(v.as_fixnum(), Some(0));
    }

    #[test]
    fn fixnum_roundtrip_positive() {
        let v = Value::fixnum(1);
        assert_eq!(v.as_fixnum(), Some(1));

        let v = Value::fixnum(i32::MAX as i64);
        assert_eq!(v.as_fixnum(), Some(i32::MAX as i64));
    }

    #[test]
    fn fixnum_roundtrip_negative() {
        let v = Value::fixnum(-1);
        assert!(v.is_fixnum());
        assert_eq!(v.as_fixnum(), Some(-1));

        let v = Value::fixnum(i32::MIN as i64);
        assert_eq!(v.as_fixnum(), Some(i32::MIN as i64));
    }

    #[test]
    fn fixnum_large_values() {
        // Near the 48-bit boundary
        let max = FIXNUM_MAX;
        let min = FIXNUM_MIN;

        let v = Value::fixnum(max);
        assert_eq!(v.as_fixnum(), Some(max));

        let v = Value::fixnum(min);
        assert_eq!(v.as_fixnum(), Some(min));

        // 1 trillion
        let v = Value::fixnum(1_000_000_000_000);
        assert_eq!(v.as_fixnum(), Some(1_000_000_000_000));

        let v = Value::fixnum(-1_000_000_000_000);
        assert_eq!(v.as_fixnum(), Some(-1_000_000_000_000));
    }

    #[test]
    #[should_panic(expected = "out of 48-bit range")]
    fn fixnum_overflow_panics() {
        Value::fixnum(FIXNUM_MAX + 1);
    }

    #[test]
    #[should_panic(expected = "out of 48-bit range")]
    fn fixnum_underflow_panics() {
        Value::fixnum(FIXNUM_MIN - 1);
    }

    #[test]
    fn float_roundtrip_normal() {
        for &f in &[0.0_f64, 1.5, -3.14, 1e100, -1e-100] {
            let v = Value::float(f);
            assert!(v.is_float(), "expected float for {f}");
            assert!(!v.is_fixnum());
            assert_eq!(v.as_float(), Some(f));
        }
    }

    #[test]
    fn float_infinity() {
        let v = Value::float(f64::INFINITY);
        assert!(v.is_float());
        assert_eq!(v.as_float(), Some(f64::INFINITY));

        let v = Value::float(f64::NEG_INFINITY);
        assert!(v.is_float());
        assert_eq!(v.as_float(), Some(f64::NEG_INFINITY));
    }

    #[test]
    fn float_nan_is_still_float() {
        // A standard (positive) NaN should NOT be confused with our tagged values.
        // Our tagged values use the NEGATIVE quiet NaN space (0xFFF8...).
        // Standard NaN is 0x7FF8... (positive quiet NaN).
        let v = Value::float(f64::NAN);
        assert!(v.is_float(), "positive NaN must be recognized as float");
        assert!(!v.is_fixnum());
        assert!(!v.is_nil());
        let extracted = v.as_float().unwrap();
        assert!(extracted.is_nan());
    }

    #[test]
    fn eq_works() {
        assert!(Value::nil().lisp_eq(Value::nil()));
        assert!(Value::t().lisp_eq(Value::t()));
        assert!(Value::fixnum(42).lisp_eq(Value::fixnum(42)));
        assert!(!Value::fixnum(1).lisp_eq(Value::fixnum(2)));
        assert!(!Value::nil().lisp_eq(Value::t()));
        assert!(!Value::fixnum(0).lisp_eq(Value::nil()));
        assert!(Value::float(3.14).lisp_eq(Value::float(3.14)));
    }

    #[test]
    fn character_roundtrip() {
        let v = Value::character('A');
        assert!(v.is_char());
        assert!(!v.is_fixnum());
        assert!(!v.is_float());
        assert_eq!(v.as_char(), Some('A'));

        // Unicode
        let v = Value::character('\u{1F600}'); // grinning face emoji
        assert!(v.is_char());
        assert_eq!(v.as_char(), Some('\u{1F600}'));

        // Null char
        let v = Value::character('\0');
        assert_eq!(v.as_char(), Some('\0'));
    }

    #[test]
    fn symbol_id_roundtrip() {
        let v = Value::symbol_id(0);
        assert!(v.is_symbol());
        assert_eq!(v.as_symbol_id(), Some(0));

        let v = Value::symbol_id(12345);
        assert!(v.is_symbol());
        assert_eq!(v.as_symbol_id(), Some(12345));

        let v = Value::symbol_id(u32::MAX);
        assert!(v.is_symbol());
        assert_eq!(v.as_symbol_id(), Some(u32::MAX));
    }

    #[test]
    fn subr_roundtrip() {
        let v = Value::subr(0);
        assert!(v.is_subr());
        assert_eq!(v.as_subr(), Some(0));

        let v = Value::subr(42);
        assert!(v.is_subr());
        assert_eq!(v.as_subr(), Some(42));
    }

    #[test]
    fn tag_values() {
        assert_eq!(Value::fixnum(0).tag(), TAG_FIXNUM as u8);
        assert_eq!(Value::symbol_id(0).tag(), TAG_SYMBOL as u8);
        assert_eq!(Value::character('x').tag(), TAG_CHAR as u8);
        assert_eq!(Value::nil().tag(), TAG_SPECIAL as u8);
        assert_eq!(Value::t().tag(), TAG_SPECIAL as u8);
        assert_eq!(Value::subr(0).tag(), TAG_SUBR as u8);
        assert_eq!(Value::float(1.0).tag(), 0xFF); // not tagged
    }

    #[test]
    fn cross_type_not_equal() {
        // Values of different types should never be equal
        let values = [
            Value::nil(),
            Value::t(),
            Value::fixnum(0),
            Value::float(0.0),
            Value::character('\0'),
            Value::symbol_id(0),
            Value::subr(0),
        ];
        for (i, a) in values.iter().enumerate() {
            for (j, b) in values.iter().enumerate() {
                if i != j {
                    assert!(a != b, "values[{i}] should != values[{j}]");
                }
            }
        }
    }

    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", Value::nil()), "nil");
        assert_eq!(format!("{:?}", Value::t()), "t");
        assert_eq!(format!("{:?}", Value::fixnum(42)), "42");
        assert_eq!(format!("{:?}", Value::fixnum(-1)), "-1");
        assert_eq!(format!("{:?}", Value::character('A')), "?A");
        // Float should display the number
        let s = format!("{:?}", Value::float(3.14));
        assert!(s.contains("3.14"), "got: {s}");
    }

    #[test]
    fn negative_nan_region_not_float() {
        // Manually craft a value in the 0xFFF8 region -- it should NOT be a float
        let raw = NANBOX_PREFIX | (TAG_FIXNUM << TAG_SHIFT) | 42;
        let v = Value(raw);
        assert!(!v.is_float());
        assert!(v.is_fixnum());
        assert_eq!(v.as_fixnum(), Some(42));
    }

    #[test]
    fn value_is_copy() {
        let a = Value::fixnum(7);
        let b = a; // copy
        let c = a; // still valid
        assert!(b.lisp_eq(c));
    }

    // -- Arithmetic fast path tests --

    #[test]
    fn add_fixnums() {
        let r = Value::fixnum(3).arith_add(Value::fixnum(4));
        assert_eq!(r, Some(Value::fixnum(7)));
    }

    #[test]
    fn add_fixnum_negative() {
        let r = Value::fixnum(10).arith_add(Value::fixnum(-3));
        assert_eq!(r, Some(Value::fixnum(7)));
    }

    #[test]
    fn add_floats() {
        let r = Value::float(1.5).arith_add(Value::float(2.5));
        assert_eq!(r.unwrap().as_float(), Some(4.0));
    }

    #[test]
    fn add_fixnum_float_promotes() {
        let r = Value::fixnum(1).arith_add(Value::float(2.5));
        assert_eq!(r.unwrap().as_float(), Some(3.5));
    }

    #[test]
    fn add_non_numeric_returns_none() {
        assert!(Value::nil().arith_add(Value::fixnum(1)).is_none());
        assert!(Value::fixnum(1).arith_add(Value::t()).is_none());
    }

    #[test]
    fn sub_fixnums() {
        let r = Value::fixnum(10).arith_sub(Value::fixnum(3));
        assert_eq!(r, Some(Value::fixnum(7)));
    }

    #[test]
    fn mul_fixnums() {
        let r = Value::fixnum(6).arith_mul(Value::fixnum(7));
        assert_eq!(r, Some(Value::fixnum(42)));
    }

    #[test]
    fn mul_fixnum_zero() {
        let r = Value::fixnum(12345).arith_mul(Value::fixnum(0));
        assert_eq!(r, Some(Value::fixnum(0)));
    }

    #[test]
    fn negate_fixnum() {
        assert_eq!(Value::fixnum(5).negate(), Some(Value::fixnum(-5)));
        assert_eq!(Value::fixnum(-5).negate(), Some(Value::fixnum(5)));
        assert_eq!(Value::fixnum(0).negate(), Some(Value::fixnum(0)));
    }

    #[test]
    fn negate_float() {
        assert_eq!(Value::float(3.14).negate().unwrap().as_float(), Some(-3.14));
    }

    #[test]
    fn negate_non_numeric() {
        assert!(Value::nil().negate().is_none());
    }

    #[test]
    fn add1_fixnum() {
        assert_eq!(Value::fixnum(41).add1(), Some(Value::fixnum(42)));
        assert_eq!(Value::fixnum(-1).add1(), Some(Value::fixnum(0)));
    }

    #[test]
    fn add1_float() {
        assert_eq!(Value::float(1.5).add1().unwrap().as_float(), Some(2.5));
    }

    #[test]
    fn sub1_fixnum() {
        assert_eq!(Value::fixnum(42).sub1(), Some(Value::fixnum(41)));
        assert_eq!(Value::fixnum(0).sub1(), Some(Value::fixnum(-1)));
    }

    #[test]
    fn num_eq_fixnums() {
        assert_eq!(Value::fixnum(5).num_eq(Value::fixnum(5)), Some(Value::t()));
        assert_eq!(
            Value::fixnum(5).num_eq(Value::fixnum(6)),
            Some(Value::nil())
        );
    }

    #[test]
    fn num_eq_mixed() {
        // 5 == 5.0 should be true
        assert_eq!(Value::fixnum(5).num_eq(Value::float(5.0)), Some(Value::t()));
    }

    #[test]
    fn lt_fixnums() {
        assert_eq!(Value::fixnum(3).lt(Value::fixnum(5)), Some(Value::t()));
        assert_eq!(Value::fixnum(5).lt(Value::fixnum(3)), Some(Value::nil()));
        assert_eq!(Value::fixnum(5).lt(Value::fixnum(5)), Some(Value::nil()));
    }

    #[test]
    fn gt_fixnums() {
        assert_eq!(Value::fixnum(5).gt(Value::fixnum(3)), Some(Value::t()));
        assert_eq!(Value::fixnum(3).gt(Value::fixnum(5)), Some(Value::nil()));
    }

    #[test]
    fn leq_fixnums() {
        assert_eq!(Value::fixnum(3).leq(Value::fixnum(5)), Some(Value::t()));
        assert_eq!(Value::fixnum(5).leq(Value::fixnum(5)), Some(Value::t()));
        assert_eq!(Value::fixnum(6).leq(Value::fixnum(5)), Some(Value::nil()));
    }

    #[test]
    fn geq_fixnums() {
        assert_eq!(Value::fixnum(5).geq(Value::fixnum(3)), Some(Value::t()));
        assert_eq!(Value::fixnum(5).geq(Value::fixnum(5)), Some(Value::t()));
        assert_eq!(Value::fixnum(4).geq(Value::fixnum(5)), Some(Value::nil()));
    }

    #[test]
    fn from_bool_values() {
        assert!(Value::from_bool(true).is_t());
        assert!(Value::from_bool(false).is_nil());
    }

    #[test]
    fn comparison_non_numeric_returns_none() {
        assert!(Value::nil().lt(Value::fixnum(1)).is_none());
        assert!(Value::fixnum(1).gt(Value::nil()).is_none());
        assert!(Value::nil().num_eq(Value::nil()).is_none());
    }

    #[test]
    fn is_list_nil_and_fixnum() {
        assert!(Value::nil().is_list());
        assert!(!Value::fixnum(42).is_list());
        assert!(!Value::t().is_list());
        assert!(!Value::float(1.0).is_list());
    }
}

// ---------------------------------------------------------------------------
// Bridge: LispObject ↔ Value conversion via thread-local heap side-table
// ---------------------------------------------------------------------------

thread_local! {
    /// The real GC heap that `obj_to_value` routes heap-typed
    /// `LispObject`s through. `None` means no scope is installed —
    /// heap-typed conversions that need a heap will return `nil`
    /// (see `obj_to_value`). Installed by `HeapScope::enter`.
    static CURRENT_HEAP: RefCell<Option<std::sync::Arc<parking_lot::Mutex<crate::gc::Heap>>>> =
        const { RefCell::new(None) };
}

/// RAII guard that installs `heap` as the current thread's active heap for
/// the lifetime of the scope. Nested scopes stack (the previous value is
/// restored on drop) so reentrant `Interpreter::eval` calls are safe.
///
/// Construction intentionally doesn't assert uniqueness: the same heap can
/// be installed repeatedly in practice (a hook running under the
/// interpreter's own eval re-enters with the same `Arc<Mutex<Heap>>`),
/// and the LIFO restore keeps the behaviour correct either way.
pub struct HeapScope {
    previous: Option<std::sync::Arc<parking_lot::Mutex<crate::gc::Heap>>>,
}

impl HeapScope {
    /// Install `heap` as the current scope's active heap. The returned
    /// guard restores the previous heap on drop.
    pub fn enter(heap: std::sync::Arc<parking_lot::Mutex<crate::gc::Heap>>) -> Self {
        let previous = CURRENT_HEAP.with(|h| h.borrow_mut().replace(heap));
        HeapScope { previous }
    }
}

impl Drop for HeapScope {
    fn drop(&mut self) {
        CURRENT_HEAP.with(|h| *h.borrow_mut() = self.previous.take());
    }
}

/// Run `f` against the current thread's active heap, if one is installed.
/// Returns `Some(f(heap))` when a scope is active, `None` otherwise —
/// callers use the `None` case to fall back to the side-table.
///
/// Implementation detail: the Arc is cloned out of the RefCell before the
/// Mutex lock is taken so neither borrow is held across the lock. This
/// also means `f` is free to call back into any `obj_to_value` /
/// `value_to_obj` path that reads `CURRENT_HEAP`, as long as it doesn't
/// re-acquire the same Mutex (which would deadlock — `parking_lot::Mutex`
/// is not reentrant).
fn with_current_heap<R>(f: impl FnOnce(&mut crate::gc::Heap) -> R) -> Option<R> {
    let heap_arc = CURRENT_HEAP.with(|h| h.borrow().as_ref().cloned());
    heap_arc.map(|arc| f(&mut arc.lock()))
}

/// Convert a LispObject into a NaN-boxed Value.
///
/// Immediate types (nil, t, fixnum, float, symbol) are encoded directly.
/// All heap types go through the current `HeapScope`'s `Heap` — this is
/// the only path after Phase 2o removed the legacy `HEAP_OBJECTS`
/// side-table. Callers that evaluate outside of any `HeapScope` get a
/// `Value::nil()` fallback (with a debug assertion). In practice every
/// interpreter entry point (`Interpreter::eval`, `eval_value`,
/// `eval_source_value`) installs a `HeapScope` first, so the fallback
/// only fires in standalone `obj_to_value` calls from tests that don't
/// bother setting up a heap.
///
/// Identity is preserved across `obj_to_value`/`value_to_obj`
/// round-trips: Cons/Vector/HashTable heap objects wrap the same `Arc`
/// the caller provided, so `setcar`/`setcdr`/`puthash`/mutation
/// primitives see a consistent view.
pub fn obj_to_value(obj: LispObject) -> Value {
    use crate::object::LispObject;
    match &obj {
        LispObject::Nil => Value::nil(),
        LispObject::T => Value::t(),
        LispObject::Integer(n) => {
            if *n >= FIXNUM_MIN && *n <= FIXNUM_MAX {
                Value::fixnum(*n)
            } else {
                // Out-of-range integer → Bignum object on the real heap.
                with_current_heap(|h| h.bignum_value(*n)).unwrap_or_else(no_heap_nil_fallback)
            }
        }
        LispObject::Float(f) => Value::float(*f),
        LispObject::Symbol(id) => Value::symbol_id(id.0),
        LispObject::String(s) => {
            with_current_heap(|h| h.string_value(s)).unwrap_or_else(no_heap_nil_fallback)
        }
        LispObject::Vector(arc) => {
            with_current_heap(|h| h.vector_value(arc.clone())).unwrap_or_else(no_heap_nil_fallback)
        }
        LispObject::HashTable(arc) => with_current_heap(|h| h.hashtable_value(arc.clone()))
            .unwrap_or_else(no_heap_nil_fallback),
        LispObject::BytecodeFn(func) => with_current_heap(|h| h.bytecode_value(func.clone()))
            .unwrap_or_else(no_heap_nil_fallback),
        LispObject::Cons(arc) => with_current_heap(|h| h.cons_arc_value(arc.clone()))
            .unwrap_or_else(no_heap_nil_fallback),
        LispObject::Primitive(name) => {
            with_current_heap(|h| h.primitive_value(name)).unwrap_or_else(no_heap_nil_fallback)
        }
    }
}

/// Fallback used by `obj_to_value` when no `HeapScope` is installed.
/// Returns `Value::nil()` so callers don't panic in release; a debug
/// assertion fires so tests that forget to install a scope are caught.
#[cold]
#[inline]
fn no_heap_nil_fallback() -> Value {
    debug_assert!(
        false,
        "obj_to_value called on a heap type without an active HeapScope; \
         install one via `HeapScope::enter(heap)` or call through \
         `Interpreter::eval` / `eval_value` / `eval_source_value`."
    );
    Value::nil()
}

/// Recover a LispObject from a NaN-boxed Value.
///
/// Immediate types are reconstructed directly; heap types are looked up
/// in the thread-local side-table.
pub fn value_to_obj(val: Value) -> LispObject {
    use crate::object::LispObject;
    if val.is_nil() {
        return LispObject::Nil;
    }
    if val.is_t() {
        return LispObject::T;
    }
    if let Some(n) = val.as_fixnum() {
        return LispObject::Integer(n);
    }
    if let Some(f) = val.as_float() {
        return LispObject::Float(f);
    }
    if let Some(id) = val.as_symbol_id() {
        return LispObject::Symbol(crate::obarray::SymbolId(id));
    }
    // Real heap pointer: dispatch on the pointed-to `GcHeader.tag`.
    // Each case recursively decodes the heap object back into a legacy
    // `LispObject` so existing call sites keep working. Object types
    // whose heap representation hasn't been added yet fall through to
    // Nil for forward compatibility.
    if val.is_heap_ptr() {
        let ptr = match val.as_heap_ptr() {
            Some(p) => p,
            None => return LispObject::Nil,
        };
        // SAFETY: the Value carries a TAG_HEAP_PTR, which by construction
        // points at a live GcHeader prefix of an object allocated by
        // `Heap`. Callers are responsible for keeping the object rooted.
        unsafe {
            let header = &*(ptr as *const crate::gc::GcHeader);
            match header.tag {
                crate::gc::ObjectTag::Cons => {
                    let cell = ptr as *const crate::gc::ConsCell;
                    let car = Value::from_raw((*cell).car);
                    let cdr = Value::from_raw((*cell).cdr);
                    return LispObject::cons(value_to_obj(car), value_to_obj(cdr));
                }
                crate::gc::ObjectTag::String => {
                    // SAFETY: `header.tag == String` means the object is
                    // a `StringObject` whose `data: Box<str>` is fully
                    // initialised at offset `size_of::<GcHeader>()`. Take
                    // an explicit `&StringObject` first so the subsequent
                    // `.data.to_string()` call uses the sanctioned autoref
                    // path rather than an implicit reborrow.
                    let obj: &crate::gc::StringObject = &*(ptr as *const crate::gc::StringObject);
                    return LispObject::String(obj.data.to_string());
                }
                crate::gc::ObjectTag::Vector => {
                    // Phase 2n: the heap object wraps the same
                    // `SharedVec` the LispObject references. Cloning
                    // the Arc preserves identity: `(eq x x)` on a
                    // heap-allocated vector stays true across
                    // obj_to_value/value_to_obj round-trips.
                    let obj: &crate::gc::VectorObject = &*(ptr as *const crate::gc::VectorObject);
                    return LispObject::Vector(obj.v.clone());
                }
                crate::gc::ObjectTag::HashTable => {
                    // Phase 2n: identity-preserving via Arc::clone.
                    let obj: &crate::gc::HashTableObject =
                        &*(ptr as *const crate::gc::HashTableObject);
                    return LispObject::HashTable(obj.table.clone());
                }
                crate::gc::ObjectTag::ByteCode => {
                    let obj: &crate::gc::BytecodeFnObject =
                        &*(ptr as *const crate::gc::BytecodeFnObject);
                    return LispObject::BytecodeFn(obj.func.clone());
                }
                crate::gc::ObjectTag::Bignum => {
                    let obj: &crate::gc::BignumObject = &*(ptr as *const crate::gc::BignumObject);
                    return LispObject::Integer(obj.value);
                }
                crate::gc::ObjectTag::Symbol => {
                    // Symbols aren't heap-allocated via Heap; live in
                    // the process-global obarray. Defensive fallthrough.
                }
                crate::gc::ObjectTag::ConsArc => {
                    // Phase 2n-cons: identity-preserving. Arc::clone
                    // shares the same inner Mutex as the original
                    // `LispObject::Cons(arc)`; `setcar`/`setcdr`
                    // mutations propagate across round-trips.
                    let obj: &crate::gc::ConsArcCell = &*(ptr as *const crate::gc::ConsArcCell);
                    return LispObject::Cons(obj.arc.clone());
                }
                crate::gc::ObjectTag::Primitive => {
                    // Phase 2o: primitives live on the real heap as
                    // owned-name wrappers. Decode clones the name
                    // into a fresh LispObject — primitives are
                    // immutable (dispatch is by name), so there's no
                    // identity story to preserve.
                    let obj: &crate::gc::PrimitiveObject =
                        &*(ptr as *const crate::gc::PrimitiveObject);
                    return LispObject::Primitive(obj.name.clone());
                }
            }
        }
        return LispObject::Nil;
    }
    LispObject::Nil
}

impl Value {
    /// Convert a LispObject to a Value (lossy for heap objects).
    /// Fixnums, floats, nil, t map exactly. Symbols use a hash.
    /// Cons/String/Vector/etc. cannot be represented without a Heap.
    pub fn from_lisp_object(obj: &crate::object::LispObject) -> Self {
        use crate::object::LispObject;
        match obj {
            LispObject::Nil => Value::nil(),
            LispObject::T => Value::t(),
            LispObject::Integer(n) => {
                if *n >= FIXNUM_MIN && *n <= FIXNUM_MAX {
                    Value::fixnum(*n)
                } else {
                    // Overflow: store as float (lossy for very large ints)
                    Value::float(*n as f64)
                }
            }
            LispObject::Float(f) => Value::float(*f),
            LispObject::Symbol(id) => Value::symbol_id(id.0),
            // Heap objects can't be converted without allocation
            _ => Value::nil(),
        }
    }

    /// Convert a Value back to a LispObject (partial — no heap object support).
    pub fn to_lisp_object(self) -> crate::object::LispObject {
        use crate::object::LispObject;
        if self.is_nil() {
            LispObject::Nil
        } else if self.is_t() {
            LispObject::T
        } else if let Some(n) = self.as_fixnum() {
            LispObject::Integer(n)
        } else if let Some(f) = self.as_float() {
            LispObject::Float(f)
        } else if let Some(ch) = self.as_char() {
            LispObject::Integer(ch as i64)
        } else if let Some(id) = self.as_symbol_id() {
            LispObject::Symbol(crate::obarray::SymbolId(id))
        } else {
            // GC pointers, subrs can't round-trip without context
            LispObject::Nil
        }
    }
}

use crate::object::LispObject;

#[cfg(test)]
mod bridge_tests {
    use super::*;
    use crate::object::LispObject;

    #[test]
    fn bridge_nil_roundtrip() {
        let v = Value::from_lisp_object(&LispObject::Nil);
        assert!(v.is_nil());
        assert_eq!(v.to_lisp_object(), LispObject::Nil);
    }

    #[test]
    fn bridge_t_roundtrip() {
        let v = Value::from_lisp_object(&LispObject::T);
        assert!(v.is_t());
        assert_eq!(v.to_lisp_object(), LispObject::T);
    }

    #[test]
    fn bridge_integer_roundtrip() {
        for n in [
            0i64,
            1,
            -1,
            42,
            -999,
            1_000_000,
            i32::MAX as i64,
            i32::MIN as i64,
        ] {
            let v = Value::from_lisp_object(&LispObject::Integer(n));
            assert!(v.is_fixnum());
            assert_eq!(v.to_lisp_object(), LispObject::Integer(n));
        }
    }

    #[test]
    fn bridge_float_roundtrip() {
        for f in [0.0, 1.5, -3.14, f64::INFINITY] {
            let v = Value::from_lisp_object(&LispObject::Float(f));
            assert!(v.is_float());
            assert_eq!(v.to_lisp_object(), LispObject::Float(f));
        }
    }

    #[test]
    fn bridge_string_routes_to_heap_when_scope_active() {
        // Phase 2m + 2o: when a HeapScope is installed, obj_to_value for
        // String produces a TAG_HEAP_PTR Value and the string lives on
        // the real GC heap.
        let heap = std::sync::Arc::new(parking_lot::Mutex::new(crate::gc::Heap::new()));
        heap.lock().set_gc_mode(crate::gc::GcMode::Manual);
        let _scope = crate::value::HeapScope::enter(heap.clone());

        let v = obj_to_value(LispObject::string("phase-2m"));
        assert!(
            v.is_heap_ptr(),
            "String must land on the real heap under scope"
        );

        // Round-trip back produces the same content.
        assert_eq!(value_to_obj(v), LispObject::string("phase-2m"));
    }

    #[test]
    fn bridge_string_without_scope_returns_nil() {
        // Phase 2o: the legacy side-table is gone. `obj_to_value` for a
        // heap type without an active HeapScope returns `Value::nil()`
        // (and trips a `debug_assert` — harmless in release tests when
        // we explicitly assert the fallback).
        //
        // Test is `#[cfg(not(debug_assertions))]` because the debug
        // assertion would panic in debug builds. The assertion is a
        // safety rail for real interpreter callers, not the standalone
        // out-of-scope path exercised here.
        #[cfg(not(debug_assertions))]
        {
            let v = obj_to_value(LispObject::string("no-scope"));
            assert!(
                v.is_nil(),
                "out-of-scope heap-typed obj_to_value returns nil"
            );
        }
    }

    #[test]
    fn bridge_bignum_routes_to_heap_when_scope_active() {
        // Phase 2m: oversized integers route to heap.bignum_value under scope.
        let heap = std::sync::Arc::new(parking_lot::Mutex::new(crate::gc::Heap::new()));
        heap.lock().set_gc_mode(crate::gc::GcMode::Manual);
        let _scope = crate::value::HeapScope::enter(heap.clone());

        let big = 1_i64 << 50;
        let v = obj_to_value(LispObject::Integer(big));
        assert!(v.is_heap_ptr(), "oversized Integer must land on the heap");
        assert_eq!(value_to_obj(v), LispObject::Integer(big));
    }

    #[test]
    fn bridge_heap_string_decodes_via_value_to_obj() {
        // Phase 2f: a Value produced by `Heap::string_value` decodes
        // through `value_to_obj` into a legacy `LispObject::String`.
        // The side-table and the real heap coexist; this exercises the
        // heap path end-to-end.
        let mut heap = crate::gc::Heap::new();
        heap.set_gc_mode(crate::gc::GcMode::Manual);
        let v = heap.string_value("phase-2f");
        assert!(v.is_heap_ptr());
        let obj = value_to_obj(v);
        assert_eq!(obj, LispObject::string("phase-2f"));

        // Sweeping the heap reclaims the string, but `obj` is a fully
        // owned `LispObject::String` so the assertion above held even
        // though the heap cell is about to be freed.
        heap.collect();
        assert_eq!(heap.bytes_allocated(), 0);
    }

    #[test]
    fn bridge_symbol_to_value() {
        let v = Value::from_lisp_object(&LispObject::symbol("foo"));
        assert!(v.is_symbol());
    }

    #[test]
    fn bridge_cons_to_nil() {
        // Cons can't be represented as Value without heap — returns nil
        let cons = LispObject::cons(LispObject::Integer(1), LispObject::Nil);
        let v = Value::from_lisp_object(&cons);
        assert!(v.is_nil());
    }
}
