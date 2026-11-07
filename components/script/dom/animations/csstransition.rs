/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::reflect_dom_object;
use script_bindings::root::DomRoot;
use script_bindings::str::DOMString;

use crate::dom::animation::Animation;
use crate::dom::bindings::codegen::Bindings::CSSTransitionBinding::CSSTransitionMethods;
use crate::dom::window::Window;

/// <https://drafts.csswg.org/css-transitions-2/#the-CSSTransition-interface>
#[dom_struct]
pub(crate) struct CSSTransition {
    animation: Animation,

    /// <https://drafts.csswg.org/css-transitions-2/#dom-csstransition-transitionproperty>
    transition_property: DOMString,
}

impl CSSTransition {
    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        transition_property: DOMString,
    ) -> DomRoot<Self> {
        let object = reflect_dom_object(
            cx,
            Box::new(Self {
                animation: Animation::new_inherited(),
                transition_property,
            }),
            window,
        );
        object.animation.initialize_promises(cx);
        object
    }
}

impl CSSTransitionMethods<crate::DomTypeHolder> for CSSTransition {
    /// <https://drafts.csswg.org/css-transitions-2/#dom-csstransition-transitionproperty>
    fn TransitionProperty(&self) -> DOMString {
        self.transition_property.clone()
    }
}
