/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::BarPropBinding::BarPropMethods;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::window::Window;

/// <https://html.spec.whatwg.org/multipage/#barprop>
#[dom_struct]
pub(crate) struct BarProp {
    reflector_: Reflector,
    window: Dom<Window>,
}

impl BarProp {
    fn new_inherited(window: &Window) -> BarProp {
        BarProp {
            reflector_: Reflector::new(),
            window: Dom::from_ref(window),
        }
    }

    pub(crate) fn new(cx: &mut JSContext, window: &Window) -> DomRoot<BarProp> {
        reflect_dom_object(cx, Box::new(BarProp::new_inherited(window)), window)
    }
}

impl BarPropMethods<crate::DomTypeHolder> for BarProp {
    /// <https://html.spec.whatwg.org/multipage/#dom-barprop-visible>
    fn Visible(&self) -> bool {
        // There are no browser UI bars, so they are all shown except for the ones that a popup
        // window asked to leave out.
        !self.window.is_popup()
    }
}
