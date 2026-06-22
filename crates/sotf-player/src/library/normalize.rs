use super::consts::TRACK_WAVEFORM_SAMPLES;
use super::types::TrackWaveform;

/// Normalize an artist or album name for consistent grouping
/// Converts to lowercase, trims whitespace, removes diacritics and special characters
/// Keeps ASCII letters, numbers, periods, and UTF-8 letters/numbers
/// Examples:
/// - "2Cellos", "2CELLOS", "2 Cellos " -> "2cellos"
/// - "Café" -> "cafe"
/// - "The Beatles!" -> "thebeatles"
/// - "AC/DC" -> "acdc"
pub(super) fn normalize_album_key(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    name.trim()
        .nfd() // Normalize to NFD (decomposed form) to separate diacritics
        .filter(|c| {
            // Keep ASCII letters, numbers, and periods
            if c.is_ascii_alphanumeric() || *c == '.' {
                return true;
            }
            // Keep UTF-8 letters and numbers (non-ASCII)
            if !c.is_ascii() && (c.is_alphabetic() || c.is_numeric()) {
                return true;
            }
            // Filter out everything else (diacritics, punctuation, etc.)
            false
        })
        .collect::<String>()
        .to_lowercase()
}

pub fn normalize_waveform_samples(samples: &[u8]) -> Option<TrackWaveform> {
    if samples.is_empty() {
        return None;
    }

    let mut normalized = [0_u8; TRACK_WAVEFORM_SAMPLES];
    if samples.len() == TRACK_WAVEFORM_SAMPLES {
        normalized.copy_from_slice(samples);
    } else {
        for (idx, sample) in normalized.iter_mut().enumerate() {
            let src_idx = (idx * samples.len()) / TRACK_WAVEFORM_SAMPLES;
            *sample = samples[src_idx];
        }
    }
    Some(Box::new(normalized))
}

pub(super) fn tagged_album_key(title: &str, edition: Option<&str>) -> String {
    let normalized = normalize_album_key(title);
    let edition = edition.map(normalize_album_key).unwrap_or_default();
    format!("{}|{}", normalized, edition)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_album_key_corpus() {
        let cases = [
            ("2Cellos", "2cellos"),
            ("2CELLOS", "2cellos"),
            ("2 Cellos ", "2cellos"),
            ("Café", "cafe"),
            ("The Beatles!", "thebeatles"),
            ("AC/DC", "acdc"),
            ("R.E.M.", "r.e.m."),
            ("  spaces  ", "spaces"),
            ("", ""),
            ("Beyoncé", "beyonce"),
            // Non-ASCII letters/numbers are preserved.
            ("日本語", "日本語"),
            ("ÄÖÜ", "aou"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                normalize_album_key(input),
                expected,
                "normalize_album_key({input:?})"
            );
        }
    }

    #[test]
    fn normalize_album_key_idempotent() {
        let inputs = ["Café!", "AC/DC (Remaster)", "  2Cellos  "];
        for input in inputs {
            let once = normalize_album_key(input);
            let twice = normalize_album_key(&once);
            assert_eq!(
                once, twice,
                "normalize_album_key not idempotent for {input:?}"
            );
        }
    }

    #[test]
    fn normalize_album_key_removes_punctuation_and_diacritics() {
        let key = normalize_album_key("¡Hola, Señor!");
        assert!(!key.contains(','));
        assert!(!key.contains('¡'));
        assert!(!key.contains('ñ'));
        assert_eq!(key, "holasenor");
    }

    mod property_tests {
        use super::super::normalize_album_key;
        use proptest::prelude::*;
        use unicode_normalization::UnicodeNormalization;

        proptest! {
            #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

            /// INVARIANT: normalizing an already-normalized key is a no-op.
            #[test]
            fn normalize_album_key_is_idempotent(input in "[[:ascii:][:alpha:][:digit:][:space:][:punct:]]{0,64}") {
                let once = normalize_album_key(&input);
                let twice = normalize_album_key(&once);
                prop_assert_eq!(
                    once, twice,
                    "normalize_album_key must be idempotent for input {:?}",
                    input
                );
            }

            /// INVARIANT: Unicode normalization before or after `normalize_album_key`
            /// does not change the result, because diacritics are removed by filtering.
            #[test]
            fn normalize_album_key_stable_over_unicode_normalization(
                chars in prop::collection::vec(any::<char>(), 0..64),
            ) {
                let input: String = chars.into_iter().collect();
                let direct = normalize_album_key(&input);
                let nfc = normalize_album_key(&input.nfc().collect::<String>());
                let nfd = normalize_album_key(&input.nfd().collect::<String>());
                prop_assert_eq!(
                    direct.clone(), nfc,
                    "NFC pre-normalization changed the result for {:?}",
                    input
                );
                prop_assert_eq!(
                    direct, nfd,
                    "NFD pre-normalization changed the result for {:?}",
                    input
                );
            }

            /// INVARIANT: the normalized output is always lowercase and trimmed.
            #[test]
            fn normalize_album_key_lowercase_and_trimmed(
                input in "[[:alpha:][:space:][:punct:]]{0,64}",
            ) {
                let key = normalize_album_key(&input);
                prop_assert_eq!(
                    key.clone(),
                    key.to_lowercase(),
                    "output must be lowercase for input {:?}",
                    input
                );
                prop_assert_eq!(
                    key.clone(),
                    key.trim(),
                    "output must be trimmed for input {:?}",
                    input
                );
            }
        }
    }
}
