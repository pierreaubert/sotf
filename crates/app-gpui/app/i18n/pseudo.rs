use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::language::Language;

const EXPANSION_NUMERATOR: usize = 14;
const EXPANSION_DENOMINATOR: usize = 10;

pub fn pseudo_expand(source: &str) -> String {
    if source.is_empty() {
        return String::new();
    }

    let mut output = String::with_capacity(source.len() * EXPANSION_NUMERATOR / 10 + 4);
    let visible = visible_character_count(source);
    let bracketed = visible >= 4;
    if bracketed {
        output.push('⟦');
    }

    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            output.push(ch);
            for placeholder in chars.by_ref() {
                output.push(placeholder);
                if placeholder == '}' {
                    break;
                }
            }
        } else {
            output.push(accent(ch));
        }
    }

    let target = visible
        .saturating_mul(EXPANSION_NUMERATOR)
        .div_ceil(EXPANSION_DENOMINATOR);
    let decoration = usize::from(bracketed) * 2;
    let current = visible + decoration;
    for _ in current..target.max(current) {
        output.push('~');
    }
    if bracketed {
        output.push('⟧');
    }
    output
}

pub fn pseudo_static(source: &'static str) -> &'static str {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, &'static str>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut cache) = cache.lock() else {
        return source;
    };
    if let Some(value) = cache.get(source) {
        return value;
    }
    let value: &'static str = Box::leak(pseudo_expand(source).into_boxed_str());
    cache.insert(source, value);
    value
}

pub fn pseudo_owned(language: Language, value: String) -> String {
    if language.is_pseudo() {
        pseudo_expand(&value)
    } else {
        value
    }
}

pub fn pseudo_borrowed(language: Language, value: &'static str) -> &'static str {
    if language.is_pseudo() {
        pseudo_static(value)
    } else {
        value
    }
}

fn visible_character_count(source: &str) -> usize {
    let mut count = 0;
    let mut in_placeholder = false;
    for ch in source.chars() {
        match ch {
            '{' => in_placeholder = true,
            '}' if in_placeholder => in_placeholder = false,
            _ if !in_placeholder => count += 1,
            _ => {}
        }
    }
    count
}

fn accent(ch: char) -> char {
    match ch {
        'A' => 'Å',
        'B' => 'Ɓ',
        'C' => 'Ç',
        'D' => 'Ð',
        'E' => 'É',
        'F' => 'Ƒ',
        'G' => 'Ĝ',
        'H' => 'Ĥ',
        'I' => 'Ï',
        'J' => 'Ĵ',
        'K' => 'Ķ',
        'L' => 'Ŀ',
        'M' => 'Ṁ',
        'N' => 'Ñ',
        'O' => 'Ö',
        'P' => 'Þ',
        'Q' => 'Ǫ',
        'R' => 'Ř',
        'S' => 'Š',
        'T' => 'Ť',
        'U' => 'Ü',
        'V' => 'Ṽ',
        'W' => 'Ŵ',
        'X' => 'Ẍ',
        'Y' => 'Ý',
        'Z' => 'Ž',
        'a' => 'å',
        'b' => 'ɓ',
        'c' => 'ç',
        'd' => 'ð',
        'e' => 'é',
        'f' => 'ƒ',
        'g' => 'ĝ',
        'h' => 'ĥ',
        'i' => 'ï',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ŀ',
        'm' => 'ṁ',
        'n' => 'ñ',
        'o' => 'ö',
        'p' => 'þ',
        'q' => 'ǫ',
        'r' => 'ř',
        's' => 'š',
        't' => 'ť',
        'u' => 'ü',
        'v' => 'ṽ',
        'w' => 'ŵ',
        'x' => 'ẍ',
        'y' => 'ý',
        'z' => 'ž',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_is_visible_bounded_and_preserves_placeholders() {
        let expanded = pseudo_expand("Found {count} speakers");
        assert!(expanded.starts_with('⟦'));
        assert!(expanded.ends_with('⟧'));
        assert!(expanded.contains("{count}"));
        assert!(expanded.contains("šþéåķéřš"));

        let source_visible = visible_character_count("Found {count} speakers");
        let expanded_visible = visible_character_count(&expanded);
        let ratio = expanded_visible as f32 / source_visible as f32;
        assert!((1.3..=1.5).contains(&ratio), "ratio={ratio}");
    }
}
