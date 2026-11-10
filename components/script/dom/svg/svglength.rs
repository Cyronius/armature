/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use html5ever::LocalName;
use js::context::JSContext;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use script_bindings::str::DOMString;
use style::properties::ComputedValues;
use style::values::computed::LineHeight;

use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::SVGLengthBinding::SVGLengthMethods;
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::globalscope::GlobalScope;
use crate::dom::node::NodeTraits;
use crate::dom::svg::svganimatedlength::SVGAnimatedLength;
use crate::dom::svg::svgelement::SVGElement;
use crate::dom::svg::svglengthlist::SVGLengthList;
use crate::dom::svg::svgtypes::{
    INITIAL_FONT_SIZE, NoLengthContext, SVGFontMetric, SVGLengthContext, SVGLengthDirection,
    SVGLengthValue,
};

/// Where the value of an [`SVGLength`] lives.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
enum SVGLengthStorage {
    /// A length that is not reflected, like one from `createSVGLength()`, or an element of
    /// a list, which reflects an attribute.
    Own {
        value: Cell<SVGLengthValue>,
        list: MutNullableDom<SVGLengthList>,
    },
    /// The base or animated value of an `SVGAnimatedLength`, which reflects a content
    /// attribute.
    Reflected(Dom<SVGAnimatedLength>),
}

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGLength>
#[dom_struct]
pub(crate) struct SVGLength {
    reflector_: Reflector,
    storage: SVGLengthStorage,
    /// <https://svgwg.org/svg2-draft/types.html#ReadOnlyLength>
    read_only: Cell<bool>,
}

impl SVGLength {
    /// Create a length that is not reflected, with the value `0`.
    pub(crate) fn new_detached(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Self> {
        Self::new_own(cx, global, SVGLengthValue::number(0.0), None, false)
    }

    /// Create a length that is not reflected, or with `list` an element of `list`.
    pub(crate) fn new_own(
        cx: &mut JSContext,
        global: &GlobalScope,
        value: SVGLengthValue,
        list: Option<&SVGLengthList>,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                storage: SVGLengthStorage::Own {
                    value: Cell::new(value),
                    list: MutNullableDom::new(list),
                },
                read_only: Cell::new(read_only),
            }),
            global,
        )
    }

    /// The list that this length is an element of.
    fn list(&self) -> Option<DomRoot<SVGLengthList>> {
        match &self.storage {
            SVGLengthStorage::Own { list, .. } => list.get(),
            SVGLengthStorage::Reflected(_) => None,
        }
    }

    /// Whether this length belongs to a list or reflects an attribute, or is read-only, in
    /// which case a list that it is inserted into gets a copy.
    pub(crate) fn is_attached(&self) -> bool {
        self.read_only.get() ||
            match &self.storage {
                SVGLengthStorage::Own { list, .. } => list.get().is_some(),
                SVGLengthStorage::Reflected(_) => true,
            }
    }

    /// Make this length, which is not attached, an element of `list`.
    pub(crate) fn attach(&self, list: &SVGLengthList, read_only: bool) {
        if let SVGLengthStorage::Own { list: own_list, .. } = &self.storage {
            own_list.set(Some(list));
            self.read_only.set(read_only);
        }
    }

    /// <https://svgwg.org/svg2-draft/types.html#TermDetach>
    pub(crate) fn detach(&self) {
        if let SVGLengthStorage::Own { list, .. } = &self.storage {
            list.set(None);
            self.read_only.set(false);
        }
    }

    /// Set the value of a length that is an element of a list, without serializing the list.
    pub(crate) fn set_raw_length(&self, length: SVGLengthValue) {
        if let SVGLengthStorage::Own { value, .. } = &self.storage {
            value.set(length);
        }
    }

    /// Synchronize the list that this length is an element of with its attribute.
    fn synchronize(&self, cx: &mut JSContext) {
        if let Some(list) = self.list() {
            list.synchronize(cx);
        }
    }

    /// Create the base value, or with `read_only` the animated value, of `animated`.
    pub(crate) fn new_reflected(
        cx: &mut JSContext,
        animated: &SVGAnimatedLength,
        read_only: bool,
    ) -> DomRoot<Self> {
        reflect_dom_object(
            cx,
            Box::new(Self {
                reflector_: Reflector::new(),
                storage: SVGLengthStorage::Reflected(Dom::from_ref(animated)),
                read_only: Cell::new(read_only),
            }),
            &*animated.element().owner_window(),
        )
    }

    /// The value, without synchronizing a list that this length is an element of first.
    pub(crate) fn length(&self, cx: &mut JSContext) -> SVGLengthValue {
        match &self.storage {
            SVGLengthStorage::Own { value, .. } => value.get(),
            SVGLengthStorage::Reflected(animated) => {
                if self.read_only.get() {
                    animated.animated_value(cx)
                } else {
                    animated.base_value()
                }
            },
        }
    }

    fn set_length(&self, cx: &mut JSContext, length: SVGLengthValue) -> ErrorResult {
        self.check_writable()?;
        match &self.storage {
            SVGLengthStorage::Own { value, list } => {
                // The other elements of the list must be up to date before it is serialized.
                if let Some(list) = list.get() {
                    list.synchronize(cx);
                }
                value.set(length);
                if let Some(list) = list.get() {
                    list.commit(cx);
                }
            },
            SVGLengthStorage::Reflected(animated) => animated.set_base_value(cx, length),
        }
        Ok(())
    }

    fn check_writable(&self) -> ErrorResult {
        if self.read_only.get() {
            return Err(Error::NoModificationAllowed(Some(
                "This SVGLength is read-only".into(),
            )));
        }
        Ok(())
    }

    /// Run `f` with the context that relative lengths are resolved in.
    fn with_context<R>(
        &self,
        cx: &mut JSContext,
        f: impl FnOnce(&mut JSContext, &dyn SVGLengthContext) -> R,
    ) -> R {
        match &self.storage {
            SVGLengthStorage::Own { list, .. } => match list.get() {
                Some(list) => {
                    let context = ElementLengthContext {
                        element: list.element(),
                        direction: list.direction(),
                    };
                    f(cx, &context)
                },
                None => f(cx, &NoLengthContext),
            },
            SVGLengthStorage::Reflected(animated) => {
                let context = ElementLengthContext {
                    element: animated.element(),
                    direction: animated.direction(),
                };
                f(cx, &context)
            },
        }
    }
}

fn not_supported() -> Error {
    Error::NotSupported(Some(
        "The length cannot be converted to the requested unit".into(),
    ))
}

impl SVGLengthMethods<crate::DomTypeHolder> for SVGLength {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__unitType>
    fn UnitType(&self, cx: &mut JSContext) -> u16 {
        self.synchronize(cx);
        self.length(cx).dom_unit_type()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__value>
    fn GetValue(&self, cx: &mut JSContext) -> Fallible<Finite<f32>> {
        self.synchronize(cx);
        let length = self.length(cx);
        let value = self
            .with_context(cx, |cx, context| length.to_user_units(cx, context))
            .ok_or_else(not_supported)?;
        Ok(Finite::wrap(value))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__value>
    fn SetValue(&self, cx: &mut JSContext, value: Finite<f32>) -> ErrorResult {
        self.synchronize(cx);
        self.check_writable()?;
        let unit_type = self.length(cx).unit_type;
        let length = self
            .with_context(cx, |cx, context| {
                SVGLengthValue::from_user_units(cx, *value, unit_type, context)
            })
            .ok_or_else(not_supported)?;
        self.set_length(cx, length)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__valueInSpecifiedUnits>
    fn ValueInSpecifiedUnits(&self, cx: &mut JSContext) -> Finite<f32> {
        self.synchronize(cx);
        Finite::wrap(self.length(cx).value)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__valueInSpecifiedUnits>
    fn SetValueInSpecifiedUnits(&self, cx: &mut JSContext, value: Finite<f32>) -> ErrorResult {
        self.synchronize(cx);
        self.check_writable()?;
        let unit_type = self.length(cx).unit_type;
        self.set_length(cx, SVGLengthValue::new(*value, unit_type))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__valueAsString>
    fn ValueAsString(&self, cx: &mut JSContext) -> DOMString {
        self.synchronize(cx);
        DOMString::from(self.length(cx).to_string())
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__valueAsString>
    fn SetValueAsString(&self, cx: &mut JSContext, value: DOMString) -> ErrorResult {
        self.synchronize(cx);
        self.check_writable()?;
        let length = SVGLengthValue::parse(&value.str())
            .ok_or_else(|| Error::Syntax(Some("The value is not a valid length".into())))?;
        self.set_length(cx, length)
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__newValueSpecifiedUnits>
    fn NewValueSpecifiedUnits(
        &self,
        cx: &mut JSContext,
        unit_type: u16,
        value: Finite<f32>,
    ) -> ErrorResult {
        self.check_writable()?;
        if !SVGLengthValue::is_valid_unit_type(unit_type) {
            return Err(not_supported());
        }
        self.set_length(cx, SVGLengthValue::new(*value, unit_type))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGLength__convertToSpecifiedUnits>
    fn ConvertToSpecifiedUnits(&self, cx: &mut JSContext, unit_type: u16) -> ErrorResult {
        self.synchronize(cx);
        self.check_writable()?;
        if !SVGLengthValue::is_valid_unit_type(unit_type) {
            return Err(not_supported());
        }
        let length = self.length(cx);
        let converted = self
            .with_context(cx, |cx, context| {
                let user_units = length.to_user_units(cx, context)?;
                SVGLengthValue::from_user_units(cx, user_units, unit_type, context)
            })
            .ok_or_else(not_supported)?;
        self.set_length(cx, converted)
    }
}

/// The value of the length attribute `attribute` of `element` in user units, or `initial`
/// when it is missing or invalid.
pub(crate) fn resolve_length(
    cx: &mut JSContext,
    element: &SVGElement,
    attribute: &LocalName,
    direction: SVGLengthDirection,
    initial: SVGLengthValue,
) -> f32 {
    let value = SVGAnimatedLength::sample_value(cx, element, attribute, direction, initial);
    let context = ElementLengthContext { element, direction };
    value.to_user_units(cx, &context).unwrap_or(0.0)
}

/// `value`, a length of an attribute of `element`, in user units.
pub(crate) fn resolve_value(
    cx: &mut JSContext,
    element: &SVGElement,
    value: SVGLengthValue,
    direction: SVGLengthDirection,
) -> f32 {
    let context = ElementLengthContext { element, direction };
    value.to_user_units(cx, &context).unwrap_or(0.0)
}

/// The context of a length that reflects an attribute of `element`.
pub(crate) struct ElementLengthContext<'a> {
    pub(crate) element: &'a SVGElement,
    pub(crate) direction: SVGLengthDirection,
}

impl ElementLengthContext<'_> {
    fn line_height_for_style(&self, style: &ComputedValues) -> f32 {
        let font = style.get_font();
        let font_size = font.font_size.computed_size().px();
        match font.line_height {
            LineHeight::Normal => {
                let window = self.element.owner_window();
                let font_context = window.font_context();
                let font_group = font_context.font_group_with_size(
                    style.clone_font(),
                    font.font_size.computed_size().into(),
                );
                font_group
                    .first(&font_context)
                    .map_or(font_size * 1.2, |font| font.metrics().line_gap.to_f32_px())
            },
            LineHeight::Number(number) => font_size * number.0,
            LineHeight::Length(length) => length.0.px(),
        }
    }
}

impl SVGLengthContext for ElementLengthContext<'_> {
    fn font_size(&self, _: &mut JSContext) -> Option<f32> {
        // An element that is not rendered has no style; use the initial font size then.
        Some(
            self.element
                .inherited_style()
                .map_or(INITIAL_FONT_SIZE, |style| {
                    style.get_font().font_size.computed_size().px()
                }),
        )
    }

    fn font_metric(&self, _: &mut JSContext, metric: SVGFontMetric) -> Option<f32> {
        let Some(style) = self.element.inherited_style() else {
            return Some(metric.fallback(INITIAL_FONT_SIZE));
        };
        let font = style.clone_font();
        let font_size = font.font_size.computed_size();
        let window = self.element.owner_window();
        let font_context = window.font_context();
        let font_group = font_context.font_group_with_size(font, font_size.into());
        let metrics = font_group
            .first(&font_context)
            .map(|font| font.metrics().clone());
        let value = metrics.and_then(|metrics| match metric {
            SVGFontMetric::XHeight => Some(metrics.x_height).filter(|value| value.0 != 0),
            SVGFontMetric::ZeroAdvance => metrics.zero_horizontal_advance,
            SVGFontMetric::IdeographicAdvance => metrics.ic_horizontal_advance,
            SVGFontMetric::CapHeight => Some(metrics.ascent),
        });
        Some(value.map_or_else(
            || metric.fallback(font_size.px()),
            |value| value.to_f32_px(),
        ))
    }

    fn root_font_size(&self, _: &mut JSContext) -> Option<f32> {
        let root = self.element.owner_document().GetDocumentElement()?;
        Some(root.style().map_or(INITIAL_FONT_SIZE, |style| {
            style.get_font().font_size.computed_size().px()
        }))
    }

    fn line_height(&self, _: &mut JSContext) -> Option<f32> {
        Some(
            self.element
                .inherited_style()
                .map_or(INITIAL_FONT_SIZE * 1.2, |style| {
                    self.line_height_for_style(&style)
                }),
        )
    }

    fn root_line_height(&self, _: &mut JSContext) -> Option<f32> {
        let root = self.element.owner_document().GetDocumentElement()?;
        Some(root.style().map_or(INITIAL_FONT_SIZE * 1.2, |style| {
            self.line_height_for_style(&style)
        }))
    }

    fn window_viewport_size(&self, _: &mut JSContext) -> Option<(f32, f32)> {
        let size = self.element.owner_window().viewport_details().size;
        Some((size.width, size.height))
    }

    fn viewport_length(&self, cx: &mut JSContext) -> Option<f32> {
        let viewport = self.element.viewport_element()?;
        let (width, height) = viewport.viewport_size(cx)?;
        Some(match self.direction {
            SVGLengthDirection::Horizontal => width,
            SVGLengthDirection::Vertical => height,
            SVGLengthDirection::Other => ((width * width + height * height) / 2.0).sqrt(),
        })
    }
}
