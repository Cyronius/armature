/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGAnimatedNumberListBinding::SVGAnimatedNumberListMethods;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::node::NodeTraits;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svgnumberlist::SVGNumberList;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGAnimatedNumberList>
#[dom_struct]
pub(crate) struct SVGAnimatedNumberList {
    reflector_: Reflector,
    element: Dom<SVGElement>,
    #[no_trace]
    attribute: LocalName,
    base_val: MutNullableDom<SVGNumberList>,
    anim_val: MutNullableDom<SVGNumberList>,
}

impl SVGAnimatedNumberList {
    pub(crate) fn new(
        cx: &mut JSContext,
        element: &SVGElement,
        attribute: LocalName,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                element: Dom::from_ref(element),
                attribute,
                base_val: Default::default(),
                anim_val: Default::default(),
            }),
            &*element.owner_window(),
        )
    }
}

impl SVGAnimatedNumberListMethods<crate::DomTypeHolder> for SVGAnimatedNumberList {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedNumberList__baseVal>
    fn BaseVal(&self, cx: &mut JSContext) -> DomRoot<SVGNumberList> {
        if let Some(list) = self.base_val.get() {
            return list;
        }
        let list = SVGNumberList::new(cx, &self.element, self.attribute.clone(), false);
        self.base_val.set(Some(&list));
        list
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGAnimatedNumberList__animVal>
    fn AnimVal(&self, cx: &mut JSContext) -> DomRoot<SVGNumberList> {
        if let Some(list) = self.anim_val.get() {
            return list;
        }
        let list = SVGNumberList::new(cx, &self.element, self.attribute.clone(), true);
        self.anim_val.set(Some(&list));
        list
    }
}
