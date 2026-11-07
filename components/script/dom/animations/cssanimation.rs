/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::reflect_dom_object;
use script_bindings::root::DomRoot;
use script_bindings::str::DOMString;

use crate::dom::animation::Animation;
use crate::dom::bindings::codegen::Bindings::CSSAnimationBinding::CSSAnimationMethods;
use crate::dom::window::Window;

/// <https://drafts.csswg.org/css-animations-2/#the-CSSAnimation-interface>
#[dom_struct]
pub(crate) struct CSSAnimation {
    animation: Animation,

    /// <https://drafts.csswg.org/css-animations-2/#dom-cssanimation-animationname>
    animation_name: DOMString,
}

impl CSSAnimation {
    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        animation_name: DOMString,
    ) -> DomRoot<Self> {
        let object = reflect_dom_object(
            cx,
            Box::new(Self {
                animation: Animation::new_inherited(),
                animation_name,
            }),
            window,
        );
        object.animation.initialize_promises(cx);
        object
    }
}

impl CSSAnimationMethods<crate::DomTypeHolder> for CSSAnimation {
    /// <https://drafts.csswg.org/css-animations-2/#dom-cssanimation-animationname>
    fn AnimationName(&self) -> DOMString {
        self.animation_name.clone()
    }
}
