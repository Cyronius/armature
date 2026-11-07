/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedLengthListBinding::SVGAnimatedLengthListMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svglengthlist::SVGLengthList;
use crate::dom::svg::svgtypes::SVGLengthDirection;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedLengthList>
#[dom_struct]
pub(crate) struct SVGAnimatedLengthList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    direction: SVGLengthDirection,
    base_val: MutNullableDom<SVGLengthList>,
    anim_val: MutNullableDom<SVGLengthList>,
}

impl SVGAnimatedLengthList {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
        direction: SVGLengthDirection,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                direction,
                base_val: Default::default(),
                anim_val: Default::default(),
            }),
            &*element.owner_window(),
        )
    }
}

impl SVGAnimatedLengthListMethods<crate::DomTypeHolder> for SVGAnimatedLengthList {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedLengthList__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGLengthList> {
        if let Some(list) = self.base_val.get() {
            return list;
        }
        let list = SVGLengthList::new(
            cx,
            &self.element,
            self.attribute.clone(),
            self.direction,
            false,
        );
        self.base_val.set(Some(&list));
        list
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedLengthList__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGLengthList> {
        if let Some(list) = self.anim_val.get() {
            return list;
        }
        let list = SVGLengthList::new(
            cx,
            &self.element,
            self.attribute.clone(),
            self.direction,
            true,
        );
        self.anim_val.set(Some(&list));
        list
    }
}
