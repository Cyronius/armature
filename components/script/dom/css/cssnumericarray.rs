/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use super::cssnumericvalue::CSSNumericValue;
use crate::dom::bindings::codegen::Bindings::CSSNumericArrayBinding::CSSNumericArrayMethods;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct CSSNumericArray {
    reflector_: Reflector,
    values: DomRefCell<Vec<Dom<CSSNumericValue>>>,
}

impl CSSNumericArray {
    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        values: Vec<DomRoot<CSSNumericValue>>,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                values: DomRefCell::new(
                    values.into_iter().map(|value| value.as_traced()).collect(),
                ),
            }),
            global,
        )
    }

    pub(crate) fn values(&self) -> Vec<DomRoot<CSSNumericValue>> {
        self.values.borrow().iter().map(Dom::as_rooted).collect()
    }
}

impl CSSNumericArrayMethods<crate::DomTypeHolder> for CSSNumericArray {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericarray-length>
    fn Length(&self) -> u32 {
        self.values.borrow().len() as u32
    }

    /// <https://drafts.css-houdini.org/css-typed-om-1/#dom-cssnumericarray-indexed-getter>
    fn IndexedGetter(&self, index: u32) -> Option<DomRoot<CSSNumericValue>> {
        self.values.borrow().get(index as usize).map(Dom::as_rooted)
    }
}
