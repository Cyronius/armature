/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::SVGNumberBinding::SVGNumberMethods;
use crate::dom::bindings::error::{Error, ErrorResult};
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::globalscope::GlobalScope;
use crate::dom::svg::svgnumberlist::SVGNumberList;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGNumber>
#[dom_struct]
pub(crate) struct SVGNumber {
    reflector_: Reflector,
    value: Cell<f32>,
    /// The list that this number is an element of, which reflects an attribute.
    list: MutNullableDom<SVGNumberList>,
    /// <https://svgwg.org/svg2-draft/types.html#ReadOnlyNumber>
    read_only: Cell<bool>,
}

impl SVGNumber {
    /// Create a number that is not reflected, like one from `createSVGNumber()`.
    pub(crate) fn new_detached(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Self> {
        Self::new(cx, global, 0.0, None, false)
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: f32,
        list: Option<&SVGNumberList>,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                value: Cell::new(value),
                list: MutNullableDom::new(list),
                read_only: Cell::new(read_only),
            }),
            global,
        )
    }

    /// The value, without synchronizing the list with its attribute first.
    pub(crate) fn raw_value(&self) -> f32 {
        self.value.get()
    }

    pub(crate) fn set_raw_value(&self, value: f32) {
        self.value.set(value);
    }

    /// Whether this number is an element of a list, or read-only, in which case a list that
    /// it is inserted into gets a copy.
    pub(crate) fn is_attached(&self) -> bool {
        self.list.get().is_some() || self.read_only.get()
    }

    pub(crate) fn attach(&self, list: &SVGNumberList, read_only: bool) {
        self.list.set(Some(list));
        self.read_only.set(read_only);
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermDetach>
    pub(crate) fn detach(&self) {
        self.list.set(None);
        self.read_only.set(false);
    }
}

impl SVGNumberMethods<crate::DomTypeHolder> for SVGNumber {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNumber__value>
    fn Value(&self, cx: &mut JSContext) -> Finite<f32> {
        if let Some(list) = self.list.get() {
            list.synchronize(cx);
        }
        Finite::wrap(self.value.get())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGNumber__value>
    fn SetValue(&self, cx: &mut JSContext, value: Finite<f32>) -> ErrorResult {
        if self.read_only.get() {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGNumber is read-only".into(),
            )));
        }
        if let Some(list) = self.list.get() {
            list.synchronize(cx);
        }
        self.value.set(*value);
        if let Some(list) = self.list.get() {
            list.commit(cx);
        }
        Ok(())
    }
}
