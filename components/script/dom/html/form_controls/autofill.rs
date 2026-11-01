/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `autocomplete` IDL attribute of `input`, `select` and `textarea` elements.
//! <https://html.spec.whatwg.org/multipage/#autofill>

use crate::dom::bindings::str::DOMString;

/// <https://html.spec.whatwg.org/multipage/#determine-a-field's-category>
#[derive(Clone, Copy, PartialEq)]
enum FieldCategory {
    Off,
    Automatic,
    Normal,
    Contact,
    Credential,
}

const NORMAL_FIELDS: &[&str] = &[
    "name",
    "honorific-prefix",
    "given-name",
    "additional-name",
    "family-name",
    "honorific-suffix",
    "nickname",
    "organization-title",
    "username",
    "new-password",
    "current-password",
    "one-time-code",
    "organization",
    "street-address",
    "address-line1",
    "address-line2",
    "address-line3",
    "address-level4",
    "address-level3",
    "address-level2",
    "address-level1",
    "country",
    "country-name",
    "postal-code",
    "cc-name",
    "cc-given-name",
    "cc-additional-name",
    "cc-family-name",
    "cc-number",
    "cc-exp",
    "cc-exp-month",
    "cc-exp-year",
    "cc-csc",
    "cc-type",
    "transaction-currency",
    "transaction-amount",
    "language",
    "bday",
    "bday-day",
    "bday-month",
    "bday-year",
    "sex",
    "url",
    "photo",
];

const CONTACT_FIELDS: &[&str] = &[
    "tel",
    "tel-country-code",
    "tel-national",
    "tel-area-code",
    "tel-local",
    "tel-local-prefix",
    "tel-local-suffix",
    "tel-extension",
    "email",
    "impp",
];

const CONTACT_KINDS: &[&str] = &["home", "work", "mobile", "fax", "pager"];

const ADDRESS_MODES: &[&str] = &["shipping", "billing"];

fn field_category(field: &str) -> Option<FieldCategory> {
    let matches = |list: &[&str]| list.iter().any(|name| field.eq_ignore_ascii_case(name));
    if field.eq_ignore_ascii_case("off") {
        Some(FieldCategory::Off)
    } else if field.eq_ignore_ascii_case("on") {
        Some(FieldCategory::Automatic)
    } else if matches(NORMAL_FIELDS) {
        Some(FieldCategory::Normal)
    } else if matches(CONTACT_FIELDS) {
        Some(FieldCategory::Contact)
    } else if field.eq_ignore_ascii_case("webauthn") {
        Some(FieldCategory::Credential)
    } else {
        None
    }
}

fn matching_keyword(token: &str, keywords: &[&'static str]) -> Option<&'static str> {
    keywords
        .iter()
        .copied()
        .find(|keyword| token.eq_ignore_ascii_case(keyword))
}

/// Computes the IDL-exposed autofill value from the value of the `autocomplete` content attribute
/// (or `None` if it is absent). `is_anchor` is true when the attribute is wearing the autofill
/// anchor mantle, which is the case for `input` elements in the Hidden state.
///
/// <https://html.spec.whatwg.org/multipage/#attr-fe-autocomplete>
pub(crate) fn idl_exposed_autofill_value(value: Option<&str>, is_anchor: bool) -> DOMString {
    DOMString::from(compute_idl_exposed_autofill_value(value, is_anchor).unwrap_or_default())
}

fn compute_idl_exposed_autofill_value(value: Option<&str>, is_anchor: bool) -> Option<String> {
    // Step 1. If the element has no autocomplete attribute, then jump to the step labeled default.
    // NOTE: Every `None` returned from here corresponds to jumping to the "default" step, which
    // sets the IDL-exposed autofill value to the empty string.
    let value = value?;

    // Step 2. Let tokens be the result of splitting the attribute's value on ASCII whitespace.
    let tokens: Vec<&str> = value.split_ascii_whitespace().collect();

    // Step 3. If tokens is empty, then jump to the step labeled default.
    // Step 4. Let index be the index of the last token in tokens.
    let mut index = tokens.len().checked_sub(1)?;

    // Step 5. Let field be the indexth token in tokens.
    let field = tokens[index];

    // Step 6. Let category be the result of determining a field's category given field.
    // Step 7. If category is null, then jump to the step labeled default.
    let mut category = field_category(field)?;

    // Step 8. If category is Off or Automatic, and either tokens's size is greater than 1 or the
    // element's autocomplete attribute is wearing the autofill anchor mantle, then jump to default.
    if matches!(category, FieldCategory::Off | FieldCategory::Automatic) &&
        (tokens.len() > 1 || is_anchor)
    {
        return None;
    }

    // Step 9. If category is Off, the IDL-exposed autofill value is "off".
    if category == FieldCategory::Off {
        return Some("off".to_owned());
    }

    // Step 10. If category is Automatic, the IDL-exposed autofill value is "on".
    if category == FieldCategory::Automatic {
        return Some("on".to_owned());
    }

    // Step 11. Let IDL value have the same value as field.
    let mut idl_value = field.to_owned();

    'done: {
        // Step 12. If category is Credential and the indexth token is webauthn:
        if category == FieldCategory::Credential {
            if index == 0 {
                break 'done;
            }
            index -= 1;
            category = field_category(tokens[index])?;
            if !matches!(category, FieldCategory::Normal | FieldCategory::Contact) {
                return None;
            }
            idl_value = format!("{} {idl_value}", tokens[index]);
        }

        // Step 13. If the indexth token in tokens is the first entry, then skip to done.
        if index == 0 {
            break 'done;
        }
        index -= 1;

        // Step 14. If category is Contact and the indexth token is a contact kind, ...
        if category == FieldCategory::Contact {
            if let Some(contact) = matching_keyword(tokens[index], CONTACT_KINDS) {
                idl_value = format!("{contact} {idl_value}");
                if index == 0 {
                    break 'done;
                }
                index -= 1;
            }
        }

        // Step 15. If the indexth token in tokens is shipping or billing, ...
        if let Some(mode) = matching_keyword(tokens[index], ADDRESS_MODES) {
            idl_value = format!("{mode} {idl_value}");
            if index == 0 {
                break 'done;
            }
            index -= 1;
        }

        // Step 16. If the indexth entry in tokens is not the first entry, then jump to default.
        if index != 0 {
            return None;
        }

        // Step 17. If the first eight characters of the indexth token are not an ASCII
        // case-insensitive match for "section-", then jump to default.
        let section = tokens[index];
        if !section
            .get(..8)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("section-"))
        {
            return None;
        }

        // Step 18. Let section be the indexth token in tokens, converted to ASCII lowercase.
        // Step 19. Let IDL value be the concatenation of section, a space and the previous value.
        idl_value = format!("{} {idl_value}", section.to_ascii_lowercase());
    }

    // Step 20. Done: the IDL-exposed autofill value is IDL value.
    Some(idl_value)
}
