/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use cssparser::ToCss;
use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::reflect_dom_object;
use servo_arc::Arc;
use style::shared_lock::{Locked, SharedRwLockReadGuard, ToCssWithGuard};
use style::stylesheets::{CssRuleType, CssRules, ScopeRule};

use super::cssgroupingrule::CSSGroupingRule;
use super::cssrule::SpecificCSSRule;
use super::cssstylesheet::CSSStyleSheet;
use crate::dom::bindings::codegen::Bindings::CSSScopeRuleBinding::CSSScopeRuleMethods;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::window::Window;

#[dom_struct]
pub(crate) struct CSSScopeRule {
    css_grouping_rule: CSSGroupingRule,
    #[ignore_malloc_size_of = "Stylo"]
    #[no_trace]
    scope_rule: RefCell<Arc<ScopeRule>>,
}

impl CSSScopeRule {
    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        parent_rule: Option<&CSSGroupingRule>,
        parent_stylesheet: &CSSStyleSheet,
        rule: Arc<ScopeRule>,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                css_grouping_rule: CSSGroupingRule::new_inherited(parent_rule, parent_stylesheet),
                scope_rule: RefCell::new(rule),
            }),
            window,
        )
    }

    pub(crate) fn clone_rules(&self) -> Arc<Locked<CssRules>> {
        self.scope_rule.borrow().rules.clone()
    }

    pub(crate) fn update_rule(&self, rule: Arc<ScopeRule>, guard: &SharedRwLockReadGuard) {
        self.css_grouping_rule.update_rules(&rule.rules, guard);
        *self.scope_rule.borrow_mut() = rule;
    }
}

impl SpecificCSSRule for CSSScopeRule {
    fn ty(&self) -> CssRuleType {
        CssRuleType::Scope
    }

    fn get_css(&self) -> DOMString {
        let guard = self.css_grouping_rule.shared_lock().read();
        self.scope_rule.borrow().to_css_string(&guard).into()
    }
}

impl CSSScopeRuleMethods<crate::DomTypeHolder> for CSSScopeRule {
    fn GetStart(&self) -> Option<DOMString> {
        self.scope_rule
            .borrow()
            .bounds
            .start
            .as_ref()
            .map(|start| start.to_css_string().into())
    }

    fn GetEnd(&self) -> Option<DOMString> {
        self.scope_rule
            .borrow()
            .bounds
            .end
            .as_ref()
            .map(|end| end.to_css_string().into())
    }
}
