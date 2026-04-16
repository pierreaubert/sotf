use std::fmt;

/// Prefix for all NaN-boxed tagged values: negative quiet NaN.
const NANBOX_PREFIX: u64 = 0xFFF8_0000_0000_0000;

/// Bits 48..51 encode the tag.
const TAG_SHIFT: u64 = 48;

/// 4-bit tag mask (applied after shifting).
const TAG_MASK: u64 = 0xF;

/// Lower 48 bits carry the payload.
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

// Tag values
const TAG_FIXNUM: u64 = 0;
const TAG_GC_PTR: u64 = 1;
const TAG_SYMBOL: u64 = 2;
const TAG_CHAR: u64 = 3;
const TAG_SPECIAL: u64 = 4;
const TAG_SUBR: u64 = 5;

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

    /// Construct a tagged GC pointer.  `ptr` must be 16-byte aligned
    /// (lower 4 bits zero) so it fits in 48 bits on current architectures.
    #[inline]
    pub fn from_ptr(tag: u8, ptr: *const u8) -> Self {
        let addr = ptr as u64;
        debug_assert!(addr & !PAYLOAD_MASK == 0, "pointer does not fit in 48 bits");
        Self(NANBOX_PREFIX | ((tag as u64 & TAG_MASK) << TAG_SHIFT) | (addr & PAYLOAD_MASK))
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

    #[inline]
    pub fn is_ptr(self) -> bool {
        self.has_tag(TAG_GC_PTR)
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

    /// Extract a GC pointer.  Returns `None` unless the tag is `TAG_GC_PTR`.
    #[inline]
    pub fn as_ptr(self) -> Option<*const u8> {
        if !self.is_ptr() {
            return None;
        }
        Some(self.payload() as *const u8)
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
        } else if self.is_ptr() {
            write!(f, "#<ptr {:p}>", self.payload() as *const u8)
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
}

// ---------------------------------------------------------------------------
// Bridge: LispObject ↔ Value conversion
// ---------------------------------------------------------------------------

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
            LispObject::Symbol(s) => {
                // Use a simple hash as symbol ID
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                s.hash(&mut hasher);
                let id = hasher.finish() as u32;
                Value::symbol_id(id)
            }
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
        } else {
            // GC pointers, symbols, subrs can't round-trip without context
            LispObject::Nil
        }
    }
}

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
    fn bridge_symbol_to_value() {
        let v = Value::from_lisp_object(&LispObject::Symbol("foo".to_string()));
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
