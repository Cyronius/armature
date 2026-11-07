/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::cell::DomRefCell;
use script_bindings::codegen::GenericBindings::ElementBinding::ScrollLogicalPosition;
use script_bindings::codegen::GenericBindings::WindowBinding::ScrollBehavior;
use script_bindings::reflector::DomObject;
use script_bindings::str::DOMString;
use style::attr::AttrValue;
use style::parser::ParserContext;
use style::properties::{PropertyDeclaration, PropertyId, longhands};
use style::stylesheets::{CssRuleType, Origin, UrlExtraData};
use style::values::generics::NonNegative;
use style::values::specified;
use style_traits::ParsingMode;
use stylo_dom::ElementState;

use crate::dom::bindings::codegen::Bindings::HTMLOrSVGOrMathMLElementBinding::FocusOptions;
use crate::dom::bindings::codegen::Bindings::SVGElementBinding::SVGElementMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot, LayoutDom, MutNullableDom};
use crate::dom::css::cssstyledeclaration::{
    CSSModificationAccess, CSSStyleDeclaration, CSSStyleOwner,
};
use crate::dom::document::Document;
use crate::dom::document::focus::FocusableArea;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::node::focus::FocusTrigger;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::svg::svganimatedangle::SVGAnimatedAngle;
use crate::dom::svg::svganimatedboolean::SVGAnimatedBoolean;
use crate::dom::svg::svganimatedenumeration::{SVGAnimatedEnumeration, SVGEnumerationKeywords};
use crate::dom::svg::svganimatedinteger::SVGAnimatedInteger;
use crate::dom::svg::svganimatedlength::SVGAnimatedLength;
use crate::dom::svg::svganimatedlengthlist::SVGAnimatedLengthList;
use crate::dom::svg::svganimatednumber::{SVGAnimatedNumber, SVGNumberPart};
use crate::dom::svg::svganimatednumberlist::SVGAnimatedNumberList;
use crate::dom::svg::svganimatedpreserveaspectratio::SVGAnimatedPreserveAspectRatio;
use crate::dom::svg::svganimatedrect::SVGAnimatedRect;
use crate::dom::svg::svganimatedstring::SVGAnimatedString;
use crate::dom::svg::svganimatedtransformlist::SVGAnimatedTransformList;
use crate::dom::svg::svgcircleelement::SVGCircleElement;
use crate::dom::svg::svgellipseelement::SVGEllipseElement;
use crate::dom::svg::svgimageelement::SVGImageElement;
use crate::dom::svg::svgpathelement::SVGPathElement;
use crate::dom::svg::svgpointlist::SVGPointList;
use crate::dom::svg::svgrectelement::SVGRectElement;
use crate::dom::svg::svgstringlist::SVGStringList;
use crate::dom::svg::svgsvgelement::SVGSVGElement;
use crate::dom::svg::svgtypes::{SVGLengthDirection, SVGLengthValue};
use crate::dom::window::scrolling_box::{ScrollAxisState, ScrollRequirement};

/// An object of the SVG DOM that reflects an attribute of an element, like an
/// `SVGAnimatedLength`, which the element keeps so that its IDL attribute always returns
/// the same object.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
enum SVGAnimatedProperty {
    Angle(Dom<SVGAnimatedAngle>),
    Boolean(Dom<SVGAnimatedBoolean>),
    Enumeration(Dom<SVGAnimatedEnumeration>),
    Integer(Dom<SVGAnimatedInteger>),
    Length(Dom<SVGAnimatedLength>),
    LengthList(Dom<SVGAnimatedLengthList>),
    Number(Dom<SVGAnimatedNumber>),
    NumberList(Dom<SVGAnimatedNumberList>),
    PointList(Dom<SVGPointList>),
    PreserveAspectRatio(Dom<SVGAnimatedPreserveAspectRatio>),
    Rect(Dom<SVGAnimatedRect>),
    String(Dom<SVGAnimatedString>),
    StringList(Dom<SVGStringList>),
    TransformList(Dom<SVGAnimatedTransformList>),
}

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct SVGAnimatedPropertyEntry {
    /// The name of the IDL attribute.
    #[ignore_malloc_size_of = "static"]
    name: &'static str,
    property: SVGAnimatedProperty,
}

#[dom_struct]
pub(crate) struct SVGElement {
    element: Element,
    style_decl: MutNullableDom<CSSStyleDeclaration>,
    /// The objects of the SVG DOM that reflect the attributes of this element.
    animated_properties: DomRefCell<Vec<SVGAnimatedPropertyEntry>>,
}

impl SVGElement {
    fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGElement {
        SVGElement::new_inherited_with_state(ElementState::empty(), tag_name, prefix, document)
    }

    pub(crate) fn new_inherited_with_state(
        state: ElementState,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGElement {
        SVGElement {
            element: Element::new_inherited_with_state(state, tag_name, ns!(svg), prefix, document),
            style_decl: Default::default(),
            animated_properties: Default::default(),
        }
    }

    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGElement::new_inherited(tag_name, prefix, document)),
            document,
            proto,
        )
    }

    fn as_element(&self) -> &Element {
        self.upcast::<Element>()
    }

    /// The value of the attribute `name`, which has no namespace. SVG attribute names may
    /// have upper case letters.
    pub(crate) fn svg_attribute(&self, name: &LocalName) -> Option<String> {
        self.as_element()
            .get_attribute_string_value_with_namespace(&ns!(), name)
    }

    /// Set the attribute `name`, which has no namespace, to `value`.
    pub(crate) fn set_svg_attribute(&self, cx: &mut JSContext, name: &LocalName, value: String) {
        let element = self.as_element();
        let value = element.parse_attribute(&ns!(), name, DOMString::from(value));
        element.set_attribute_with_namespace(cx, name.clone(), value, name.clone(), ns!(), None);
    }

    /// The object that the IDL attribute `name` returns, which `create` creates the first
    /// time.
    fn animated_property<T: DomObject>(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        get: impl Fn(&SVGAnimatedProperty) -> Option<DomRoot<T>>,
        create: impl FnOnce(&mut JSContext) -> DomRoot<T>,
        wrap: impl FnOnce(&T) -> SVGAnimatedProperty,
    ) -> DomRoot<T> {
        if let Some(property) = self
            .animated_properties
            .borrow()
            .iter()
            .find(|entry| entry.name == name)
            .and_then(|entry| get(&entry.property))
        {
            return property;
        }
        let object = create(cx);
        self.animated_properties
            .borrow_mut()
            .push(SVGAnimatedPropertyEntry {
                name,
                property: wrap(&object),
            });
        object
    }

    /// The `SVGAnimatedLength` of the IDL attribute `name`, which reflects the attribute
    /// `attribute`.
    pub(crate) fn animated_length(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
        direction: SVGLengthDirection,
        initial: SVGLengthValue,
    ) -> DomRoot<SVGAnimatedLength> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::Length(length) => Some(length.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedLength::new(cx, self, attribute, direction, initial),
            |object| SVGAnimatedProperty::Length(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedNumber` of the IDL attribute `name`.
    pub(crate) fn animated_number(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
        part: SVGNumberPart,
        initial: f32,
    ) -> DomRoot<SVGAnimatedNumber> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::Number(number) => Some(number.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedNumber::new(cx, self, attribute, part, initial),
            |object| SVGAnimatedProperty::Number(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedInteger` of the IDL attribute `name`.
    pub(crate) fn animated_integer(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
        part: SVGNumberPart,
        initial: i32,
    ) -> DomRoot<SVGAnimatedInteger> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::Integer(integer) => Some(integer.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedInteger::new(cx, self, attribute, part, initial),
            |object| SVGAnimatedProperty::Integer(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedEnumeration` of the IDL attribute `name`.
    pub(crate) fn animated_enumeration(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
        keywords: SVGEnumerationKeywords,
        initial: u16,
    ) -> DomRoot<SVGAnimatedEnumeration> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::Enumeration(enumeration) => Some(enumeration.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedEnumeration::new(cx, self, attribute, keywords, initial),
            |object| SVGAnimatedProperty::Enumeration(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedBoolean` of the IDL attribute `name`.
    pub(crate) fn animated_boolean(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
    ) -> DomRoot<SVGAnimatedBoolean> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::Boolean(boolean) => Some(boolean.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedBoolean::new(cx, self, attribute),
            |object| SVGAnimatedProperty::Boolean(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedString` of the IDL attribute `name`.
    pub(crate) fn animated_string(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
    ) -> DomRoot<SVGAnimatedString> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::String(string) => Some(string.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedString::new(cx, self, attribute),
            |object| SVGAnimatedProperty::String(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedNumberList` of the IDL attribute `name`.
    pub(crate) fn animated_number_list(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
    ) -> DomRoot<SVGAnimatedNumberList> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::NumberList(list) => Some(list.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedNumberList::new(cx, self, attribute),
            |object| SVGAnimatedProperty::NumberList(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedLengthList` of the IDL attribute `name`.
    pub(crate) fn animated_length_list(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
        direction: SVGLengthDirection,
    ) -> DomRoot<SVGAnimatedLengthList> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::LengthList(list) => Some(list.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedLengthList::new(cx, self, attribute, direction),
            |object| SVGAnimatedProperty::LengthList(Dom::from_ref(object)),
        )
    }

    /// The `SVGStringList` of the IDL attribute `name`, which reflects the attribute with
    /// the same name.
    pub(crate) fn string_list(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        comma_separated: bool,
    ) -> DomRoot<SVGStringList> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::StringList(list) => Some(list.as_rooted()),
                _ => None,
            },
            |cx| SVGStringList::new(cx, self, LocalName::from(name), comma_separated),
            |object| SVGAnimatedProperty::StringList(Dom::from_ref(object)),
        )
    }

    /// The `points` (or with `read_only`, the `animatedPoints`) of a polyline or polygon.
    ///
    /// <https://svgwg.org/svg2-draft/shapes.html#InterfaceSVGAnimatedPoints>
    pub(crate) fn point_list(&self, cx: &mut JSContext, read_only: bool) -> DomRoot<SVGPointList> {
        let name = if read_only {
            "animatedPoints"
        } else {
            "points"
        };
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::PointList(list) => Some(list.as_rooted()),
                _ => None,
            },
            |cx| SVGPointList::new(cx, self, read_only),
            |object| SVGAnimatedProperty::PointList(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedTransformList` of the IDL attribute `name`.
    pub(crate) fn animated_transform_list(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
    ) -> DomRoot<SVGAnimatedTransformList> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::TransformList(list) => Some(list.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedTransformList::new(cx, self, attribute),
            |object| SVGAnimatedProperty::TransformList(Dom::from_ref(object)),
        )
    }

    /// The `SVGAnimatedAngle` of the IDL attribute `name`.
    pub(crate) fn animated_angle(
        &self,
        cx: &mut JSContext,
        name: &'static str,
        attribute: LocalName,
    ) -> DomRoot<SVGAnimatedAngle> {
        self.animated_property(
            cx,
            name,
            |property| match property {
                SVGAnimatedProperty::Angle(angle) => Some(angle.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedAngle::new(cx, self, attribute),
            |object| SVGAnimatedProperty::Angle(Dom::from_ref(object)),
        )
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGFitToViewBox__viewBox>
    pub(crate) fn view_box(&self, cx: &mut JSContext) -> DomRoot<SVGAnimatedRect> {
        self.animated_property(
            cx,
            "viewBox",
            |property| match property {
                SVGAnimatedProperty::Rect(rect) => Some(rect.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedRect::new(cx, self, local_name!("viewBox")),
            |object| SVGAnimatedProperty::Rect(Dom::from_ref(object)),
        )
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGFitToViewBox__preserveAspectRatio>
    pub(crate) fn preserve_aspect_ratio(
        &self,
        cx: &mut JSContext,
    ) -> DomRoot<SVGAnimatedPreserveAspectRatio> {
        self.animated_property(
            cx,
            "preserveAspectRatio",
            |property| match property {
                SVGAnimatedProperty::PreserveAspectRatio(value) => Some(value.as_rooted()),
                _ => None,
            },
            |cx| SVGAnimatedPreserveAspectRatio::new(cx, self),
            |object| SVGAnimatedProperty::PreserveAspectRatio(Dom::from_ref(object)),
        )
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGURIReference__href>
    pub(crate) fn href(&self, cx: &mut JSContext) -> DomRoot<SVGAnimatedString> {
        self.animated_string(cx, "href", local_name!("href"))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGElement__viewportElement>
    ///
    /// The nearest ancestor `svg` element, which establishes the viewport of this one.
    pub(crate) fn viewport_element(&self) -> Option<DomRoot<SVGSVGElement>> {
        self.upcast::<Node>()
            .ancestors()
            .take_while(|ancestor| ancestor.is::<SVGElement>())
            .find_map(DomRoot::downcast::<SVGSVGElement>)
    }
}

impl VirtualMethods for SVGElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.as_element() as &dyn VirtualMethods)
    }

    fn attribute_mutated(
        &self,
        cx: &mut js::context::JSContext,
        attr: AttrRef<'_>,
        mutation: AttributeMutation,
    ) {
        self.super_type()
            .unwrap()
            .attribute_mutated(cx, attr, mutation);
        let element = self.as_element();
        if let (&local_name!("nonce"), mutation) = (attr.local_name(), mutation) {
            match mutation {
                AttributeMutation::Set(_) => {
                    let nonce = &**attr.value();
                    element.update_nonce_internal_slot(nonce.to_owned(), cx.no_gc());
                },
                AttributeMutation::Removed => {
                    element.update_nonce_internal_slot(String::new(), cx.no_gc());
                },
            }
        }
    }

    fn attribute_affects_presentational_hints(&self, attr: AttrRef<'_>) -> bool {
        matches!(
            attr.local_name(),
            &local_name!("fill") |
                &local_name!("fill-opacity") |
                &local_name!("fill-rule") |
                &local_name!("stroke") |
                &local_name!("stroke-width") |
                &local_name!("stroke-linecap") |
                &local_name!("stroke-linejoin") |
                &local_name!("stroke-dasharray") |
                &local_name!("stroke-dashoffset") |
                &local_name!("stroke-miterlimit") |
                &local_name!("stroke-opacity") |
                &local_name!("clip-rule") |
                &local_name!("color-interpolation") |
                &local_name!("color-interpolation-filters") |
                &local_name!("flood-color") |
                &local_name!("flood-opacity") |
                &local_name!("lighting-color") |
                &local_name!("marker-start") |
                &local_name!("marker-mid") |
                &local_name!("marker-end") |
                &local_name!("paint-order") |
                &local_name!("shape-rendering") |
                &local_name!("stop-color") |
                &local_name!("stop-opacity") |
                &local_name!("text-anchor") |
                &local_name!("vector-effect") |
                &local_name!("display") |
                &local_name!("visibility") |
                &local_name!("opacity") |
                &local_name!("cx") |
                &local_name!("cy") |
                &local_name!("r") |
                &local_name!("rx") |
                &local_name!("ry") |
                &local_name!("x") |
                &local_name!("y") |
                &local_name!("width") |
                &local_name!("height") |
                &local_name!("d")
        ) || self
            .super_type()
            .unwrap()
            .attribute_affects_presentational_hints(attr)
    }
}

impl SVGElementMethods<crate::DomTypeHolder> for SVGElement {
    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGElement__className>
    fn ClassName(&self, cx: &mut JSContext) -> DomRoot<SVGAnimatedString> {
        self.animated_string(cx, "className", local_name!("class"))
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGElement__ownerSVGElement>
    fn GetOwnerSVGElement(&self) -> Option<DomRoot<SVGSVGElement>> {
        self.viewport_element()
    }

    /// <https://svgwg.org/svg2-draft/types.html#__svg__SVGElement__viewportElement>
    fn GetViewportElement(&self) -> Option<DomRoot<SVGElement>> {
        self.viewport_element().map(DomRoot::upcast)
    }

    /// <https://html.spec.whatwg.org/multipage/#the-style-attribute>
    fn Style(&self, cx: &mut JSContext) -> DomRoot<CSSStyleDeclaration> {
        self.style_decl.or_init(|| {
            let global = self.owner_window();
            CSSStyleDeclaration::new(
                cx,
                &global,
                CSSStyleOwner::Element(Dom::from_ref(self.upcast())),
                None,
                CSSModificationAccess::ReadWrite,
            )
        })
    }

    // https://html.spec.whatwg.org/multipage/#globaleventhandlers
    global_event_handlers!();

    /// <https://html.spec.whatwg.org/multipage/#dom-noncedelement-nonce>
    fn Nonce(&self) -> DOMString {
        self.as_element().nonce_value().into()
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-noncedelement-nonce>
    fn SetNonce(&self, cx: &mut JSContext, value: DOMString) {
        self.as_element()
            .update_nonce_internal_slot(String::from(value), cx.no_gc())
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-fe-autofocus>
    fn Autofocus(&self) -> bool {
        self.element.has_attribute(&local_name!("autofocus"))
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-fe-autofocus>
    fn SetAutofocus(&self, cx: &mut JSContext, autofocus: bool) {
        self.element
            .set_bool_attribute(cx, &local_name!("autofocus"), autofocus);
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-focus>
    fn Focus(&self, cx: &mut js::context::JSContext, options: &FocusOptions) {
        // 1. If the allow focus steps given this's node document return false, then return.
        // TODO: Implement this.

        // 2. Run the focusing steps for this.
        if !self
            .upcast::<Node>()
            .run_the_focusing_steps(cx, None, FocusTrigger::Script)
        {
            // The specification seems to imply we should scroll into view even if this element
            // is not a focusable area. No browser does this, so we return early in that case.
            // See https://github.com/whatwg/html/issues/12231.
            return;
        }

        // > 3. If options["focusVisible"] is true, or does not exist but in an
        // >    implementation-defined  way the user agent determines it would be best to do so,
        // >    then indicate focus. TODO: Implement this.
        // TODO: Implement this.

        // > 4. If options["preventScroll"] is false, then scroll a target into view given this,
        // >    "auto", "center", and "center".
        if !options.preventScroll {
            let scroll_axis = ScrollAxisState {
                position: ScrollLogicalPosition::Center,
                requirement: ScrollRequirement::IfNotVisible,
            };
            self.upcast::<Element>().scroll_into_view_with_options(
                cx,
                ScrollBehavior::Smooth,
                scroll_axis,
                scroll_axis,
                None,
                None,
            );
        }
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-blur>
    fn Blur(&self, cx: &mut js::context::JSContext) {
        // TODO: Run the unfocusing steps. Focus the top-level document, not
        //       the current document.
        if !self.as_element().focus_state() {
            return;
        }
        // <https://html.spec.whatwg.org/multipage/#unfocusing-steps>
        self.owner_document().focus_handler().focus(
            cx,
            &FocusableArea::Viewport,
            FocusTrigger::Script,
        );
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-tabindex>
    fn TabIndex(&self) -> i32 {
        self.element.tab_index()
    }

    /// <https://html.spec.whatwg.org/multipage/#dom-tabindex>
    fn SetTabIndex(&self, cx: &mut JSContext, tab_index: i32) {
        self.element
            .set_attribute(cx, &local_name!("tabindex"), tab_index.into());
    }
}

impl<'dom> LayoutDom<'dom, SVGElement> {
    pub(crate) fn synthesize_presentational_hints(
        self,
        document: LayoutDom<'dom, Document>,
        push: &mut impl FnMut(PropertyDeclaration),
    ) {
        let element = self.upcast::<Element>();

        if element.is::<SVGSVGElement>() {
            if let Some(width) = element
                .get_attr_for_layout(&ns!(), &local_name!("width"))
                .and_then(AttrValue::as_length_percentage)
            {
                push(PropertyDeclaration::Width(
                    specified::Size::LengthPercentage(NonNegative(width.clone())),
                ));
            }
            if let Some(height) = element
                .get_attr_for_layout(&ns!(), &local_name!("height"))
                .and_then(AttrValue::as_length_percentage)
            {
                push(PropertyDeclaration::Height(
                    specified::Size::LengthPercentage(NonNegative(height.clone())),
                ));
            }
        }
        let url_data = UrlExtraData(document.url_for_layout().get_arc());
        let parsing_mode =
            ParsingMode::ALLOW_UNITLESS_LENGTH | ParsingMode::ALLOW_ALL_NUMERIC_VALUES;
        let parser_context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            parsing_mode,
            document.quirks_mode(),
            Default::default(),
            None,
            None,
            Default::default(),
        );

        self.parse_svg_attribute(
            &parser_context,
            "fill",
            longhands::fill::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "fill-opacity",
            longhands::fill_opacity::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "fill-rule",
            longhands::fill_rule::parse_declared,
            push,
        );

        self.parse_svg_attribute(
            &parser_context,
            "stroke",
            longhands::stroke::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-width",
            longhands::stroke_width::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-linecap",
            longhands::stroke_linecap::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-linejoin",
            longhands::stroke_linejoin::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-dasharray",
            longhands::stroke_dasharray::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-dashoffset",
            longhands::stroke_dashoffset::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-miterlimit",
            longhands::stroke_miterlimit::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stroke-opacity",
            longhands::stroke_opacity::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "clip-rule",
            longhands::clip_rule::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "color-interpolation",
            longhands::color_interpolation::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "color-interpolation-filters",
            longhands::color_interpolation_filters::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "flood-color",
            longhands::flood_color::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "flood-opacity",
            longhands::flood_opacity::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "lighting-color",
            longhands::lighting_color::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "marker-start",
            longhands::marker_start::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "marker-mid",
            longhands::marker_mid::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "marker-end",
            longhands::marker_end::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "paint-order",
            longhands::paint_order::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "shape-rendering",
            longhands::shape_rendering::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stop-color",
            longhands::stop_color::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "stop-opacity",
            longhands::stop_opacity::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "text-anchor",
            longhands::text_anchor::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "vector-effect",
            longhands::vector_effect::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "display",
            longhands::display::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "visibility",
            longhands::visibility::parse_declared,
            push,
        );
        self.parse_svg_attribute(
            &parser_context,
            "opacity",
            longhands::opacity::parse_declared,
            push,
        );

        // Parse geometry attributes based on element type
        // <circle>: https://svgwg.org/svg2-draft/shapes.html#CircleElement
        if element.downcast::<SVGCircleElement>().is_some() {
            self.parse_svg_attribute(&parser_context, "cx", longhands::cx::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "cy", longhands::cy::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "r", longhands::r::parse_declared, push);
        }
        // <ellipse>: https://svgwg.org/svg2-draft/shapes.html#EllipseElement
        if element.downcast::<SVGEllipseElement>().is_some() {
            self.parse_svg_attribute(&parser_context, "cx", longhands::cx::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "cy", longhands::cy::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "rx", longhands::rx::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "ry", longhands::ry::parse_declared, push);
        }
        // <rect>: https://svgwg.org/svg2-draft/shapes.html#RectElement
        if element.downcast::<SVGRectElement>().is_some() {
            self.parse_svg_attribute(&parser_context, "x", longhands::x::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "y", longhands::y::parse_declared, push);
            self.parse_svg_attribute(
                &parser_context,
                "width",
                longhands::width::parse_declared,
                push,
            );
            self.parse_svg_attribute(
                &parser_context,
                "height",
                longhands::height::parse_declared,
                push,
            );
            self.parse_svg_attribute(&parser_context, "rx", longhands::rx::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "ry", longhands::ry::parse_declared, push);
        }
        // <image>: https://svgwg.org/svg2-draft/embedded.html#ImageElement
        if element.downcast::<SVGImageElement>().is_some() {
            self.parse_svg_attribute(&parser_context, "x", longhands::x::parse_declared, push);
            self.parse_svg_attribute(&parser_context, "y", longhands::y::parse_declared, push);
            self.parse_svg_attribute(
                &parser_context,
                "width",
                longhands::width::parse_declared,
                push,
            );
            self.parse_svg_attribute(
                &parser_context,
                "height",
                longhands::height::parse_declared,
                push,
            );
        }
        // <path>: https://svgwg.org/svg2-draft/paths.html#PathElement
        if element.downcast::<SVGPathElement>().is_some() {
            // The d CSS property only accepts `none` or `path(<string>)`,
            // but the SVG presentation attribute uses raw path data (e.g. "M0,0 L1,1").
            // Wrap the raw path data in `path("...")` so the CSS parser can handle it.
            if let Some(value) = element.get_attr_val_for_layout(&ns!(), &local_name!("d")) {
                if value.eq_ignore_ascii_case("none") {
                    let mut parser = cssparser::Parser::new(value);
                    if let Ok(property) =
                        parser.parse_entirely(|i| longhands::d::parse_declared(&parser_context, i))
                    {
                        push(property);
                    }
                } else {
                    let wrapped = format!("path(\"{}\")", value);
                    let mut parser = cssparser::Parser::new(&wrapped);
                    if let Ok(property) = parser.parse_entirely(|parse_input| {
                        longhands::d::parse_declared(&parser_context, parse_input)
                    }) {
                        push(property);
                    }
                }
            }
        }
    }

    fn parse_svg_attribute<F>(
        self,
        parser_context: &ParserContext,
        attr_name: &str,
        parse: F,
        push: &mut impl FnMut(PropertyDeclaration),
    ) where
        F: for<'i> FnOnce(
            &ParserContext,
            &mut cssparser::Parser<'i>,
        ) -> Result<PropertyDeclaration, style_traits::ParseError>,
    {
        let element = self.upcast::<Element>();
        if let Some(value) = element.get_attr_val_for_layout(&ns!(), &LocalName::from(attr_name)) {
            // A presentation attribute does nothing when its property is disabled by a preference.
            if PropertyId::parse_enabled_for_all_content(attr_name).is_err() {
                return;
            }
            let mut parser = cssparser::Parser::new(value);
            if let Ok(property) =
                parser.parse_entirely(|parse_input| parse(parser_context, parse_input))
            {
                push(property);
            }
        }
    }
}
