use super::misc::is_subsequence;
use super::plugin_preset::PluginPreset;
use super::plugin_preset::searchable_fields;
use super::types::PresetSearchResult;
use super::types::push_unique_field;

pub(super) fn score_preset<'a>(
    preset: &'a PluginPreset,
    terms: &[String],
) -> Option<PresetSearchResult<'a>> {
    let fields = searchable_fields(preset);
    let mut score = 0u32;
    let mut matched_fields = Vec::new();

    for term in terms {
        let mut term_matched = false;
        for (field, value, exact_weight, contains_weight, fuzzy_weight) in &fields {
            let Some(field_score) =
                score_field(value, term, *exact_weight, *contains_weight, *fuzzy_weight)
            else {
                continue;
            };
            score = score.saturating_add(field_score);
            push_unique_field(&mut matched_fields, *field);
            term_matched = true;
        }
        if !term_matched {
            return None;
        }
    }

    Some(PresetSearchResult {
        preset,
        score,
        matched_fields,
    })
}

pub(super) fn score_field(
    field_value: &str,
    term: &str,
    exact_weight: u32,
    contains_weight: u32,
    fuzzy_weight: u32,
) -> Option<u32> {
    if field_value.is_empty() || term.is_empty() {
        return None;
    }
    if field_value == term {
        return Some(exact_weight);
    }
    if field_value.split_whitespace().any(|word| word == term) {
        return Some(exact_weight.saturating_sub(5));
    }
    if field_value.contains(term) {
        return Some(contains_weight);
    }
    if is_subsequence(term, field_value) {
        return Some(fuzzy_weight);
    }
    None
}
