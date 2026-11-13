/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::reflect_dom_object;
use servo_arc::Arc;
use style::counter_style::CounterStyleRule;
use style::shared_lock::{Locked, ToCssWithGuard};
use style::stylesheets::CssRuleType;
use style_traits::ToCss;

use super::cssrule::{CSSRule, SpecificCSSRule};
use super::cssstylesheet::CSSStyleSheet;
use crate::dom::bindings::codegen::Bindings::CSSCounterStyleRuleBinding::CSSCounterStyleRuleMethods;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::cssgroupingrule::CSSGroupingRule;
use crate::dom::window::Window;

#[dom_struct]
pub(crate) struct CSSCounterStyleRule {
    css_rule: CSSRule,
    #[ignore_malloc_size_of = "Stylo"]
    #[no_trace]
    counter_style_rule: RefCell<Arc<Locked<CounterStyleRule>>>,
}

impl CSSCounterStyleRule {
    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        parent_rule: Option<&CSSGroupingRule>,
        parent_stylesheet: &CSSStyleSheet,
        rule: Arc<Locked<CounterStyleRule>>,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                css_rule: CSSRule::new_inherited(parent_rule, parent_stylesheet),
                counter_style_rule: RefCell::new(rule),
            }),
            window,
        )
    }

    pub(crate) fn update_rule(&self, rule: Arc<Locked<CounterStyleRule>>) {
        *self.counter_style_rule.borrow_mut() = rule;
    }

    fn descriptor(
        &self,
        get: impl FnOnce(&style::counter_style::Descriptors) -> DOMString,
    ) -> DOMString {
        let guard = self.css_rule.shared_lock().read();
        let rule = self.counter_style_rule.borrow();
        get(rule.read_with(&guard).descriptors())
    }
}

impl SpecificCSSRule for CSSCounterStyleRule {
    fn ty(&self) -> CssRuleType {
        CssRuleType::CounterStyle
    }

    fn get_css(&self) -> DOMString {
        let guard = self.css_rule.shared_lock().read();
        self.counter_style_rule
            .borrow()
            .read_with(&guard)
            .to_css_string(&guard)
            .into()
    }
}

macro_rules! descriptor_accessors {
    ($getter:ident, $setter:ident, $field:ident) => {
        fn $getter(&self) -> DOMString {
            self.descriptor(|descriptors| {
                descriptors
                    .$field
                    .as_ref()
                    .map_or_else(|| DOMString::from(""), |value| value.to_css_string().into())
            })
        }

        fn $setter(&self, _value: DOMString) {
            // TODO: Parse and update the descriptor through Stylo's CSSOM setter.
        }
    };
}

impl CSSCounterStyleRuleMethods<crate::DomTypeHolder> for CSSCounterStyleRule {
    fn Name(&self) -> DOMString {
        let guard = self.css_rule.shared_lock().read();
        let rule = self.counter_style_rule.borrow();
        rule.read_with(&guard).name().to_css_string().into()
    }

    fn SetName(&self, _value: DOMString) {
        // TODO: Parse and update the rule name through Stylo's CSSOM setter.
    }

    descriptor_accessors!(System, SetSystem, system);
    descriptor_accessors!(Symbols, SetSymbols, symbols);
    descriptor_accessors!(AdditiveSymbols, SetAdditiveSymbols, additive_symbols);
    descriptor_accessors!(Negative, SetNegative, negative);
    descriptor_accessors!(Prefix, SetPrefix, prefix);
    descriptor_accessors!(Suffix, SetSuffix, suffix);
    descriptor_accessors!(Range, SetRange, range);
    descriptor_accessors!(Pad, SetPad, pad);
    descriptor_accessors!(SpeakAs, SetSpeakAs, speak_as);
    descriptor_accessors!(Fallback, SetFallback, fallback);
}
