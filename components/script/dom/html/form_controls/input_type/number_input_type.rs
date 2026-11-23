/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::domstring::parse_floating_point_number;

use crate::dom::bindings::str::DOMString;
use crate::dom::html::form_controls::htmlinputelement::HTMLInputElement;
use crate::dom::html::form_controls::input_type::SpecificInputType;
use crate::dom::html::form_controls::input_type::text_input_widget::TextInputWidget;

#[derive(Default, JSTraceable, MallocSizeOf, PartialEq)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct NumberInputType {
    text_input_widget: DomRefCell<TextInputWidget>,
}

/// Reads user-typed number text more leniently than the spec's floating-point number grammar
/// (accepting a trailing '.' or a leading '+'), as Gecko does with its localized parser.
fn lenient_number(text: &str) -> Option<f64> {
    let text = text.trim_matches(|c: char| c.is_ascii_whitespace());
    if text.is_empty() ||
        !text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'))
    {
        return None;
    }
    text.parse::<f64>().ok().filter(|number| number.is_finite())
}

impl SpecificInputType for NumberInputType {
    fn text_input_widget(&self) -> Option<&DomRefCell<TextInputWidget>> {
        Some(&self.text_input_widget)
    }

    fn sanitize_value(&self, _input: &HTMLInputElement, value: &mut DOMString) {
        if !value.is_valid_floating_point_number_string() {
            value.clear();
        }
        // Spec says that user agent "may" round the value
        // when it's suffering a step mismatch, but WPT tests
        // want it unrounded, and this matches other browser
        // behavior (typing an unrounded number into an
        // integer field box and pressing enter generally keeps
        // the number intact but makes the input box :invalid)
    }

    /// Text typed by the user that is not a valid floating-point number may still parse
    /// leniently (for example "1."); the getter then returns the number's best
    /// representation, and anything else becomes the empty string. This mirrors Gecko's
    /// `SanitizationKind::ForValueGetter` for number inputs.
    fn sanitize_value_for_getter(&self, _input: &HTMLInputElement, value: &mut DOMString) {
        if value.is_valid_floating_point_number_string() {
            return;
        }
        let lenient = lenient_number(&value.str());
        value.clear();
        if let Some(number) = lenient &&
            let Some(string) = self.convert_number_to_string(number)
        {
            value.push_str(&string.str());
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#number-state-(type=number):concept-input-value-string-number>
    fn convert_string_to_number(&self, input: &str) -> Option<f64> {
        parse_floating_point_number(input)
    }

    /// <https://html.spec.whatwg.org/multipage/#number-state-(type=number):concept-input-value-string-number>
    fn convert_number_to_string(&self, input: f64) -> Option<DOMString> {
        let mut value = DOMString::from(input.to_string());
        value.set_best_representation_of_the_floating_point_number();
        Some(value)
    }

    /// <https://html.spec.whatwg.org/multipage/#number-state-(type=number):suffering-from-bad-input>
    /// `value` is the text as the user typed it; only text that cannot be read as a number
    /// at all is bad input (Gecko's `NumberInputType::HasBadInput`).
    fn suffers_from_bad_input(&self, value: &DOMString) -> bool {
        !value.is_valid_floating_point_number_string() && lenient_number(&value.str()).is_none()
    }

    fn update_shadow_tree(&self, cx: &mut JSContext, input: &HTMLInputElement) {
        self.text_input_widget
            .borrow()
            .update_shadow_tree(cx, input)
    }

    fn update_placeholder_contents(&self, cx: &mut JSContext, input: &HTMLInputElement) {
        self.text_input_widget
            .borrow()
            .update_placeholder_contents(cx, input)
    }
}
