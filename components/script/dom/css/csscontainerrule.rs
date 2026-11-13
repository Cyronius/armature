/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::MutableHandleValue;
use script_bindings::reflector::reflect_dom_object;
use servo_arc::Arc;
use style::shared_lock::{SharedRwLockReadGuard, ToCssWithGuard};
use style::stylesheets::{ContainerRule, CssRuleType};
use style_traits::ToCss;

use super::cssconditionrule::CSSConditionRule;
use super::cssgroupingrule::CSSGroupingRule;
use super::cssrule::SpecificCSSRule;
use super::cssstylesheet::CSSStyleSheet;
use crate::dom::bindings::codegen::Bindings::CSSContainerRuleBinding::{
    CSSContainerCondition, CSSContainerRuleMethods,
};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::utils::to_frozen_array;
use crate::dom::window::Window;

#[dom_struct]
pub(crate) struct CSSContainerRule {
    css_condition_rule: CSSConditionRule,
    #[ignore_malloc_size_of = "Stylo"]
    #[no_trace]
    container_rule: RefCell<Arc<ContainerRule>>,
}

impl CSSContainerRule {
    fn new_inherited(
        parent_rule: Option<&CSSGroupingRule>,
        parent_stylesheet: &CSSStyleSheet,
        rule: Arc<ContainerRule>,
    ) -> CSSContainerRule {
        CSSContainerRule {
            css_condition_rule: CSSConditionRule::new_inherited(
                parent_rule,
                parent_stylesheet,
                rule.rules.clone(),
            ),
            container_rule: RefCell::new(rule),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        parent_rule: Option<&CSSGroupingRule>,
        parent_stylesheet: &CSSStyleSheet,
        rule: Arc<ContainerRule>,
    ) -> DomRoot<CSSContainerRule> {
        reflect_dom_object(
            cx,
            Box::new(CSSContainerRule::new_inherited(
                parent_rule,
                parent_stylesheet,
                rule,
            )),
            window,
        )
    }

    fn serialized_conditions(&self) -> Vec<CSSContainerCondition> {
        self.container_rule
            .borrow()
            .conditions
            .0
            .iter()
            .map(|condition| CSSContainerCondition {
                name: if condition.name().is_none() {
                    DOMString::new()
                } else {
                    condition.name().to_css_string().into()
                },
                query: condition
                    .query_condition()
                    .map(|query| query.to_css_string().into())
                    .unwrap_or_else(DOMString::new),
            })
            .collect()
    }

    pub(crate) fn get_condition_text(&self) -> DOMString {
        self.container_rule
            .borrow()
            .conditions
            .to_css_string()
            .into()
    }

    pub(crate) fn update_rule(&self, rule: Arc<ContainerRule>, guard: &SharedRwLockReadGuard) {
        self.css_condition_rule
            .update_rules(rule.rules.clone(), guard);
        *self.container_rule.borrow_mut() = rule;
    }
}

impl SpecificCSSRule for CSSContainerRule {
    fn ty(&self) -> CssRuleType {
        CssRuleType::Container
    }

    fn get_css(&self) -> DOMString {
        let guard = self.css_condition_rule.shared_lock().read();
        self.container_rule.borrow().to_css_string(&guard).into()
    }
}

impl CSSContainerRuleMethods<crate::DomTypeHolder> for CSSContainerRule {
    fn ContainerName(&self) -> DOMString {
        let conditions = self.serialized_conditions();
        if conditions.len() == 1 {
            conditions.into_iter().next().unwrap().name
        } else {
            DOMString::new()
        }
    }

    fn ContainerQuery(&self) -> DOMString {
        let conditions = self.serialized_conditions();
        if conditions.len() == 1 {
            conditions.into_iter().next().unwrap().query
        } else {
            DOMString::new()
        }
    }

    fn Conditions(&self, cx: &mut JSContext, retval: MutableHandleValue) {
        to_frozen_array(cx, &self.serialized_conditions(), retval);
    }
}
