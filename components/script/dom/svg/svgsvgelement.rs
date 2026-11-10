/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use atomic_refcell::AtomicRefCell;
use base64::Engine as _;
use cssparser::Parser;
use dom_struct::dom_struct;
use euclid::default::Transform3D;
use html5ever::{LocalName, Prefix, local_name, ns};
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use layout_api::SVGElementData;
use servo_url::ServoUrl;
use style::attr::AttrValue;
use style::parser::ParserContext;
use style::properties::{PropertyDeclarationId, PropertyId};
use style::stylesheets::Origin;
use style::values::specified::LengthPercentage;
use style_traits::ParsingMode;
use stylo_atoms::Atom;
use uuid::Uuid;

use crate::dom::bindings::codegen::Bindings::DOMMatrixBinding::DOMMatrix2DInit;
use crate::dom::bindings::codegen::Bindings::DOMRectReadOnlyBinding::DOMRectReadOnlyMethods;
use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::ElementBinding::ElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::codegen::Bindings::SVGSVGElementBinding::SVGSVGElementMethods;
use crate::dom::bindings::codegen::Bindings::SelectionBinding::SelectionMethods;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{DomRoot, LayoutDom, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::dommatrix::DOMMatrix;
use crate::dom::dommatrixreadonly::dommatrix2dinit_to_matrix;
use crate::dom::dompoint::DOMPoint;
use crate::dom::domrect::DOMRect;
use crate::dom::domrectreadonly::DOMRectReadOnly;
use crate::dom::element::attributes::storage::AttrRef;
use crate::dom::element::{AttributeMutation, Element};
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::nodelist::NodeList;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{
    ChildrenMutation, CloneChildrenFlag, Node, NodeDamage, NodeTraits, UnbindContext,
};
use crate::dom::svg::svgangle::SVGAngle;
use crate::dom::svg::svganimatedlength::SVGAnimatedLength;
use crate::dom::svg::svgelement::{PRESENTATION_ATTRIBUTES, SVGElement};
use crate::dom::svg::svggeometry;
use crate::dom::svg::svggraphicselement::SVGGraphicsElement;
use crate::dom::svg::svgimageelement::SVGImageRequestSuppression;
use crate::dom::svg::svglength::SVGLength;
use crate::dom::svg::svgnumber::SVGNumber;
use crate::dom::svg::svgtransform::{SVGTransform, SVGTransformValue};
use crate::dom::svg::svgtypes::{
    SVGLengthDirection, SVGLengthValue, parse_number, split_list, svg_animated_length,
    svg_fit_to_view_box,
};

/// Document CSS lies outside an inline SVG's serialized rendering copy. Transfer the resolved
/// fill while keeping other paint and text properties in the rasterizer's existing path.
const RASTERIZED_SVG_STYLE_PROPERTIES: &[&str] = &["fill"];

#[dom_struct]
pub(crate) struct SVGSVGElement {
    svggraphicselement: SVGGraphicsElement,
    #[no_trace]
    uuid: Uuid,
    // The XML source of subtree rooted at this SVG element, serialized into
    // a base64 encoded `data:` url. This is cached to avoid recomputation
    // on each layout and must be invalidated when the subtree changes.
    #[no_trace]
    cached_serialized_data_url: AtomicRefCell<Option<Result<ServoUrl, ()>>>,
    /// Same-document IDs imported from another SVG root by this rendering copy.
    #[no_trace]
    cached_external_reference_ids: AtomicRefCell<Vec<String>>,
    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__currentScale>
    current_scale: Cell<f32>,
    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__currentTranslate>
    current_translate: MutNullableDom<DOMPoint>,
    /// SVG document time when this root's clock was last started or resumed.
    animation_time_origin: Cell<f64>,
    /// Clock value at `animation_time_origin`, or the held value while paused.
    animation_time: Cell<f64>,
    animations_paused: Cell<bool>,
}

impl SVGSVGElement {
    /// The outermost SVG root in this fragment, if this element is nested in one.
    fn outermost_svg(&self) -> Option<DomRoot<SVGSVGElement>> {
        self.upcast::<Node>()
            .ancestors()
            .take_while(|ancestor| ancestor.is::<SVGElement>())
            .filter_map(DomRoot::downcast::<SVGSVGElement>)
            .last()
    }

    /// The time in this SVG fragment's independent animation clock, in seconds.
    pub(crate) fn animation_time(&self) -> f64 {
        if let Some(root) = self.outermost_svg() {
            return root.animation_time();
        }
        if self.animations_paused.get() {
            self.animation_time.get()
        } else {
            self.animation_time.get() + self.owner_document().current_animation_timeline_value() -
                self.animation_time_origin.get()
        }
    }

    pub(crate) fn animations_are_paused(&self) -> bool {
        self.outermost_svg().map_or_else(
            || self.animations_paused.get(),
            |root| root.animations_paused.get(),
        )
    }

    fn new_inherited(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGSVGElement {
        SVGSVGElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(local_name, prefix, document),
            uuid: Uuid::new_v4(),
            cached_serialized_data_url: Default::default(),
            cached_external_reference_ids: Default::default(),
            current_scale: Cell::new(1.0),
            current_translate: Default::default(),
            animation_time_origin: Cell::new(document.current_animation_timeline_value()),
            animation_time: Cell::new(0.0),
            animations_paused: Cell::new(false),
        }
    }

    #[cfg_attr(crown, allow(crown::unrooted_must_root))]
    pub(crate) fn new(
        cx: &mut js::context::JSContext,
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<SVGSVGElement> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(SVGSVGElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
        )
    }

    pub(crate) fn serialize_and_cache_subtree(&self, cx: &mut js::context::JSContext) {
        let _suppress_image_requests = SVGImageRequestSuppression::new();
        self.cached_external_reference_ids.borrow_mut().clear();
        let document_fragment = self.owner_document().CreateDocumentFragment(cx);
        let cloned_node = Node::clone(
            cx,
            self.upcast(),
            None,
            CloneChildrenFlag::CloneChildren,
            None,
        );
        if document_fragment
            .upcast::<Node>()
            .AppendChild(cx, &cloned_node)
            .is_err()
        {
            error!("Unable to clone SVG tree");
            *self.cached_serialized_data_url.borrow_mut() = Some(Err(()));
            return;
        }

        self.apply_sampled_effects_to_copy(cx, self.upcast(), &cloned_node);

        self.process_use_elements(cx, &cloned_node);

        let Ok(xml_source) = cloned_node.xml_serialize(false) else {
            *self.cached_serialized_data_url.borrow_mut() = Some(Err(()));
            return;
        };

        let xml_source: String = xml_source.into();
        let base64_encoded_source = base64::engine::general_purpose::STANDARD.encode(xml_source);
        let data_url = format!("data:image/svg+xml;base64,{base64_encoded_source}");
        match ServoUrl::parse(&data_url) {
            Ok(url) => *self.cached_serialized_data_url.borrow_mut() = Some(Ok(url)),
            Err(error) => error!("Unable to parse serialized SVG data url: {error}"),
        };
    }

    /// The image renderer does not sample SMIL. Apply sampled effects to each
    /// rendering copy, including definitions imported through `<use>`.
    fn apply_sampled_effects_to_copy(
        &self,
        cx: &mut JSContext,
        source_root: &Node,
        clone_root: &Node,
    ) {
        let document = self.owner_document();
        for (source, clone) in source_root
            .traverse_preorder(ShadowIncluding::No)
            .zip(clone_root.traverse_preorder(ShadowIncluding::No))
        {
            let (Some(source), Some(clone)) = (
                source.downcast::<SVGElement>(),
                clone.downcast::<SVGElement>(),
            ) else {
                continue;
            };
            if svggeometry::motion_transform(cx, &source).is_some() {
                let [a, b, c, d, e, f] = svggeometry::element_transform(cx, &source).as_coeffs();
                clone.set_svg_attribute(
                    cx,
                    &local_name!("transform"),
                    format!("matrix({a} {b} {c} {d} {e} {f})"),
                );
            }

            let mut style = source
                .svg_attribute(&local_name!("style"))
                .unwrap_or_default();
            let original_style_len = style.len();
            if document.is_html_document() &&
                let Some(computed_style) = source
                    .upcast::<Node>()
                    .owner_window()
                    .computed_style_for_svg(source.upcast::<Node>().to_trusted_node_address())
            {
                let parent_style = source
                    .upcast::<Node>()
                    .GetParentElement()
                    .and_then(DomRoot::downcast::<SVGElement>)
                    .and_then(|parent| {
                        parent
                            .upcast::<Node>()
                            .owner_window()
                            .computed_style_for_svg(
                                parent.upcast::<Node>().to_trusted_node_address(),
                            )
                    });
                for property in RASTERIZED_SVG_STYLE_PROPERTIES {
                    let Ok(PropertyId::NonCustom(id)) =
                        PropertyId::parse_enabled_for_all_content(property)
                    else {
                        continue;
                    };
                    let Ok(id) = id.longhand_or_shorthand() else {
                        continue;
                    };
                    let value = computed_style
                        .computed_value_to_string(PropertyDeclarationId::Longhand(id));
                    // A referenced subtree inherits from its `<use>` element. Copying an
                    // inherited value that came only from the definition's ancestors would
                    // incorrectly override the value at the use site.
                    if id.inherited() &&
                        parent_style.as_ref().is_some_and(|parent| {
                            parent.computed_value_to_string(PropertyDeclarationId::Longhand(id)) ==
                                value
                        })
                    {
                        continue;
                    }
                    // URLs are relative to the original document, not to the rendering copy's
                    // data URL. Keep those references in the original attributes and styles.
                    if value.is_empty() || value.contains("url(") {
                        continue;
                    }
                    if !style.trim().is_empty() && !style.trim_end().ends_with(';') {
                        style.push(';');
                    }
                    style.push_str(property);
                    style.push(':');
                    style.push_str(&value);
                }
            }
            let mut serialized_properties = Vec::new();
            for animation in document.smil_animations_targeting(cx, &source) {
                if !animation.passes_conditional_processing() {
                    continue;
                }
                let kind = animation.upcast::<Element>().local_name();
                let active = if kind == &local_name!("set") {
                    animation.set_is_active()
                } else if kind == &local_name!("animate") {
                    animation.sample_iteration().is_some()
                } else {
                    false
                };
                if !active {
                    continue;
                }
                let Some(property) = animation
                    .upcast::<SVGElement>()
                    .svg_attribute(&local_name!("attributeName"))
                else {
                    continue;
                };
                let is_geometry_property = matches!(
                    property.as_str(),
                    "x" | "y" |
                        "width" |
                        "height" |
                        "cx" |
                        "cy" |
                        "r" |
                        "rx" |
                        "ry" |
                        "x1" |
                        "y1" |
                        "x2" |
                        "y2" |
                        "fx" |
                        "fy" |
                        "fr"
                );
                let is_animated_property = PRESENTATION_ATTRIBUTES.contains(&property.as_str()) ||
                    is_geometry_property ||
                    property == "d" &&
                        source.upcast::<Element>().local_name() == &local_name!("path");
                if !is_animated_property || serialized_properties.contains(&property) {
                    continue;
                }
                if is_geometry_property {
                    let direction = match property.as_str() {
                        "x" | "width" | "cx" | "rx" | "x1" | "x2" | "fx" => {
                            SVGLengthDirection::Horizontal
                        },
                        "y" | "height" | "cy" | "ry" | "y1" | "y2" | "fy" => {
                            SVGLengthDirection::Vertical
                        },
                        _ => SVGLengthDirection::Other,
                    };
                    let value = SVGAnimatedLength::sample_value(
                        cx,
                        &source,
                        &LocalName::from(property.as_str()),
                        direction,
                        SVGLengthValue::number(0.0),
                    );
                    // resvg reads geometry from SVG attributes. Keep the live
                    // attribute unchanged and write its sampled value to the copy.
                    clone.set_svg_attribute(
                        cx,
                        &LocalName::from(property.as_str()),
                        value.to_string(),
                    );
                    serialized_properties.push(property);
                    continue;
                }
                let value = svggeometry::computed_value(&source, &property);
                if value.is_empty() {
                    continue;
                }
                if !style.trim().is_empty() && !style.trim_end().ends_with(';') {
                    style.push(';');
                }
                style.push_str(&property);
                style.push(':');
                style.push_str(&value);
                serialized_properties.push(property);
            }
            if style.len() != original_style_len {
                clone.set_svg_attribute(cx, &local_name!("style"), style);
            }
        }
    }

    fn process_use_elements(&self, cx: &mut JSContext, root_node: &Node) {
        for node in root_node.traverse_preorder(ShadowIncluding::No) {
            if let Some(element) = node.downcast::<Element>() &&
                element.local_name() == &local_name!("use")
            {
                self.process_single_use_element(cx, element, root_node)
            }
        }
    }

    fn process_single_use_element(
        &self,
        cx: &mut JSContext,
        use_element: &Element,
        root_node: &Node,
    ) {
        // `href` takes precedence over the deprecated `xlink:href`.
        // <https://svgwg.org/svg2-draft/linking.html#XLinkRefAttrs>
        let Some(href) = use_element
            .get_attribute_string_value(&local_name!("href"))
            .or_else(|| {
                use_element
                    .get_attribute_string_value_with_namespace(&ns!(xlink), &local_name!("href"))
            })
        else {
            return;
        };
        let Some(id_string) = href.strip_prefix("#").map(DOMString::from) else {
            return;
        };

        let document = self.upcast::<Node>().owner_doc();
        let Some(referenced_element) = document.GetElementById(cx, id_string.clone()) else {
            return;
        };
        let referenced_node = referenced_element.upcast::<Node>();

        // Don't use this node if it doesn't have an `<svg>` ancestor.
        if !referenced_node
            .inclusive_ancestors_unrooted(cx.no_gc(), ShadowIncluding::No)
            .any(|ancestor| ancestor.is::<SVGSVGElement>())
        {
            return;
        };

        // Don't use this node if it already exists within the same `<svg>` element.
        if referenced_node
            .inclusive_ancestors_unrooted(cx.no_gc(), ShadowIncluding::No)
            .any(|ancestor| *ancestor == self.upcast())
        {
            return;
        };

        self.cached_external_reference_ids
            .borrow_mut()
            .push(String::from(id_string));
        let cloned_node = Node::clone(
            cx,
            referenced_node,
            None,
            CloneChildrenFlag::CloneChildren,
            None,
        );
        self.apply_sampled_effects_to_copy(cx, referenced_node, &cloned_node);
        let _ = root_node.AppendChild(cx, &cloned_node);
    }

    pub(crate) fn depends_on_external_ids(&self, ids: &[String]) -> bool {
        self.cached_external_reference_ids
            .borrow()
            .iter()
            .any(|reference| ids.contains(reference))
    }

    pub(crate) fn has_external_references(&self) -> bool {
        !self.cached_external_reference_ids.borrow().is_empty()
    }

    pub(crate) fn invalidate_cached_serialized_subtree_and_rasterization_result(
        &self,
        no_gc: &NoGC,
    ) {
        if self.cached_serialized_data_url.borrow().is_none() {
            return;
        }
        let owner_window = self.owner_window();
        owner_window
            .image_cache()
            .evict_rasterized_image(&self.uuid);
        if let Some(Ok(url)) = &*self.cached_serialized_data_url.borrow() {
            owner_window.layout_mut().remove_cached_image(url);
            owner_window.image_cache().evict_completed_image(
                url,
                owner_window.origin().immutable(),
                &None,
            );
        }

        *self.cached_serialized_data_url.borrow_mut() = None;
        self.cached_external_reference_ids.borrow_mut().clear();
        self.upcast::<Node>().dirty(no_gc, NodeDamage::Other);
    }
}

impl SVGSVGElement {
    /// The size of the viewport that this element establishes, in user units: the size of
    /// its `viewBox`, or else the size of its box.
    pub(crate) fn viewport_size(&self, cx: &mut JSContext) -> Option<(f32, f32)> {
        let view_box = self
            .upcast::<SVGElement>()
            .svg_attribute(&local_name!("viewBox"))
            .and_then(|value| {
                let numbers = split_list(&value)?
                    .into_iter()
                    .map(parse_number)
                    .collect::<Option<Vec<f32>>>()?;
                match numbers.as_slice() {
                    [_, _, width, height] if *width >= 0.0 && *height >= 0.0 => {
                        Some((*width, *height))
                    },
                    _ => None,
                }
            });
        if view_box.is_some() {
            return view_box;
        }
        let (width, height) = svggeometry::svg_viewport_size(cx, self.upcast());
        Some((width as f32, height as f32))
    }
}

impl SVGSVGElement {
    /// Whether the bounding box of `element`, in the user space of this element, intersects
    /// `rect`, or with `enclosure`, lies inside it.
    fn check_element(
        &self,
        cx: &mut JSContext,
        element: &SVGElement,
        rect: &DOMRectReadOnly,
        enclosure: bool,
    ) -> bool {
        let Some(bbox) = svggeometry::bounding_box(cx, element) else {
            return false;
        };
        let transform = svggeometry::transform_to_user_space_of(cx, element, self.upcast());
        let bbox = transform.transform_rect_bbox(bbox);
        let rect = kurbo::Rect::new(
            rect.X(),
            rect.Y(),
            rect.X() + rect.Width(),
            rect.Y() + rect.Height(),
        );
        if enclosure {
            rect.contains_rect(bbox)
        } else {
            rect.intersect(bbox).area() > 0.0 ||
                (bbox.area() == 0.0 && rect.intersect(bbox) == bbox)
        }
    }

    /// The graphics elements below this one, or below `reference_element`, that intersect
    /// or lie inside `rect`.
    fn element_list(
        &self,
        cx: &mut JSContext,
        rect: &DOMRectReadOnly,
        reference_element: Option<&SVGElement>,
        enclosure: bool,
    ) -> DomRoot<NodeList> {
        let root = reference_element.map_or(self.upcast::<Node>(), |element| element.upcast());
        let candidates: Vec<DomRoot<SVGElement>> = root
            .traverse_preorder(ShadowIncluding::No)
            .skip(1)
            .filter_map(DomRoot::downcast::<SVGElement>)
            .filter(|element| {
                svggeometry::shape_geometry_kind(element) ||
                    matches!(
                        element.upcast::<Element>().local_name(),
                        &local_name!("text") | &local_name!("image") | &local_name!("use")
                    )
            })
            .collect();
        let mut matching = Vec::new();
        for candidate in candidates {
            if self.check_element(cx, &candidate, rect, enclosure) {
                matching.push(DomRoot::upcast::<Node>(candidate));
            }
        }
        NodeList::new_simple_list(cx, &self.owner_window(), matching.into_iter())
    }
}

impl<'dom> LayoutDom<'dom, SVGSVGElement> {
    pub(crate) fn data(self) -> SVGElementData<'dom> {
        let svg_id = self.unsafe_get().uuid;
        let element = self.upcast::<Element>();
        let width = element.get_attr_for_layout(&ns!(), &local_name!("width"));
        let height = element.get_attr_for_layout(&ns!(), &local_name!("height"));
        let view_box = element.get_attr_for_layout(&ns!(), &local_name!("viewBox"));
        SVGElementData {
            source: self
                .unsafe_get()
                .cached_serialized_data_url
                .borrow()
                .clone(),
            width,
            height,
            view_box,
            svg_id,
        }
    }
}

impl VirtualMethods for SVGSVGElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
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

        self.invalidate_cached_serialized_subtree_and_rasterization_result(cx.no_gc());
    }

    fn attribute_affects_presentational_hints(&self, attr: AttrRef<'_>) -> bool {
        match attr.local_name() {
            &local_name!("width") | &local_name!("height") => true,
            _ => self
                .super_type()
                .unwrap()
                .attribute_affects_presentational_hints(attr),
        }
    }

    fn parse_plain_attribute(&self, name: &LocalName, value: DOMString) -> AttrValue {
        match *name {
            local_name!("width") | local_name!("height") => {
                let value = &value.str();
                let parser = &mut Parser::new(value);
                let doc = self.owner_document();
                let url = doc.url().into_url().into();
                let context = ParserContext::new(
                    Origin::Author,
                    &url,
                    None,
                    ParsingMode::ALLOW_UNITLESS_LENGTH,
                    doc.quirks_mode(),
                    /* namespaces = */ Default::default(),
                    None,
                    None,
                    /* attr_taint = */ Default::default(),
                );
                let val = LengthPercentage::parse_quirky(
                    &context,
                    parser,
                    style::values::specified::AllowQuirks::Always,
                );
                AttrValue::LengthPercentage(value.to_string(), val.ok())
            },
            _ => self
                .super_type()
                .unwrap()
                .parse_plain_attribute(name, value),
        }
    }

    fn children_changed(&self, cx: &mut JSContext, mutation: &ChildrenMutation) {
        if let Some(super_type) = self.super_type() {
            super_type.children_changed(cx, mutation);
        }

        self.invalidate_cached_serialized_subtree_and_rasterization_result(cx.no_gc());
    }

    fn unbind_from_tree(&self, cx: &mut js::context::JSContext, context: &UnbindContext<'_>) {
        if let Some(s) = self.super_type() {
            s.unbind_from_tree(cx, context);
        }

        self.invalidate_cached_serialized_subtree_and_rasterization_result(cx.no_gc());
    }

    /// Fire a `load` event at an `svg` element once the parser has parsed it and its
    /// descendants.
    ///
    /// SVG 1.1 fires its `SVGLoad` event "at the point at which the user agent has fully parsed
    /// the element and its descendants" (<https://www.w3.org/TR/SVG11/interact.html#LoadEvent>),
    /// and browsers still fire it, as `load`, at the `svg` elements of both SVG and HTML
    /// documents. SVG 2 only describes the `load` event of the `Window` and of elements that
    /// load external resources (<https://svgwg.org/svg2-draft/interact.html#LoadEvent>), so this
    /// follows the browsers. Parser-blocking scripts inside the element have run by the time the
    /// parser pops it. Elements in documents without a browsing context, such as `template`
    /// contents and documents from `DOMParser`, fire no event.
    fn pop(&self, cx: &mut JSContext) {
        if let Some(s) = self.super_type() {
            s.pop(cx);
        }

        if !self.owner_document().has_browsing_context() {
            return;
        }
        self.owner_global()
            .task_manager()
            .dom_manipulation_task_source()
            .queue_simple_event(self.upcast(), atom!("load"));
    }
}

impl SVGSVGElementMethods<crate::DomTypeHolder> for SVGSVGElement {
    svg_fit_to_view_box!();

    /// <https://www.w3.org/TR/SVG11/animate.html#PauseAnimations>
    fn PauseAnimations(&self) {
        if self.outermost_svg().is_some() {
            return;
        }
        if !self.animations_paused.get() {
            self.animation_time.set(self.animation_time());
            self.animations_paused.set(true);
        }
    }

    /// <https://www.w3.org/TR/SVG11/animate.html#UnpauseAnimations>
    fn UnpauseAnimations(&self) {
        if self.outermost_svg().is_some() {
            return;
        }
        if self.animations_paused.replace(false) {
            self.animation_time_origin
                .set(self.owner_document().current_animation_timeline_value());
        }
    }

    /// <https://www.w3.org/TR/SVG11/animate.html#AnimationsPaused>
    fn AnimationsPaused(&self) -> bool {
        self.animations_are_paused()
    }

    /// <https://www.w3.org/TR/SVG11/animate.html#GetCurrentTime>
    fn GetCurrentTime(&self) -> Finite<f32> {
        Finite::wrap(self.animation_time() as f32)
    }

    /// <https://www.w3.org/TR/SVG11/animate.html#SetCurrentTime>
    fn SetCurrentTime(&self, cx: &mut JSContext, seconds: Finite<f32>) {
        if self.outermost_svg().is_some() {
            return;
        }
        let previous_time = self.animation_time();
        self.animation_time.set(*seconds as f64);
        self.animation_time_origin
            .set(self.owner_document().current_animation_timeline_value());
        self.owner_document()
            .advance_smil_timeline_to(self, *seconds as f64);
        self.owner_document()
            .queue_smil_seek_events(self, previous_time, *seconds as f64);
        self.owner_document().update_smil_style_effects(cx);
        self.invalidate_cached_serialized_subtree_and_rasterization_result(cx.no_gc());
    }

    svg_animated_length!(X, "x", Horizontal, SVGLengthValue::number(0.0));
    svg_animated_length!(Y, "y", Vertical, SVGLengthValue::number(0.0));
    svg_animated_length!(
        Width,
        "width",
        Horizontal,
        SVGLengthValue::percentage(100.0)
    );
    svg_animated_length!(
        Height,
        "height",
        Vertical,
        SVGLengthValue::percentage(100.0)
    );

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGNumber>
    fn CreateSVGNumber(&self, cx: &mut JSContext) -> DomRoot<SVGNumber> {
        SVGNumber::new_detached(cx, &self.owner_global())
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__currentScale>
    fn CurrentScale(&self) -> Finite<f32> {
        Finite::wrap(self.current_scale.get())
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__currentScale>
    fn SetCurrentScale(&self, _cx: &mut JSContext, value: Finite<f32>) {
        self.current_scale.set(*value);
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__currentTranslate>
    fn CurrentTranslate(&self, cx: &mut JSContext) -> DomRoot<DOMPoint> {
        if let Some(point) = self.current_translate.get() {
            return point;
        }
        let point = DOMPoint::new_svg(cx, &self.owner_global(), 0.0, 0.0);
        self.current_translate.set(Some(&point));
        point
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__getIntersectionList>
    fn GetIntersectionList(
        &self,
        cx: &mut JSContext,
        rect: &DOMRectReadOnly,
        reference_element: Option<&SVGElement>,
    ) -> DomRoot<NodeList> {
        self.element_list(cx, rect, reference_element, false)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__getEnclosureList>
    fn GetEnclosureList(
        &self,
        cx: &mut JSContext,
        rect: &DOMRectReadOnly,
        reference_element: Option<&SVGElement>,
    ) -> DomRoot<NodeList> {
        self.element_list(cx, rect, reference_element, true)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__checkIntersection>
    fn CheckIntersection(
        &self,
        cx: &mut JSContext,
        element: &SVGElement,
        rect: &DOMRectReadOnly,
    ) -> bool {
        self.check_element(cx, element, rect, false)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__checkEnclosure>
    fn CheckEnclosure(
        &self,
        cx: &mut JSContext,
        element: &SVGElement,
        rect: &DOMRectReadOnly,
    ) -> bool {
        self.check_element(cx, element, rect, true)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__deselectAll>
    fn DeselectAll(&self, cx: &mut JSContext) {
        if let Some(selection) = self.owner_document().GetSelection(cx) {
            selection.RemoveAllRanges(cx.no_gc());
        }
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGMatrix>
    fn CreateSVGMatrix(&self, cx: &mut JSContext) -> DomRoot<DOMMatrix> {
        DOMMatrix::new(cx, &self.owner_global(), true, Transform3D::identity())
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGRect>
    fn CreateSVGRect(&self, cx: &mut JSContext) -> DomRoot<DOMRect> {
        DOMRect::new_svg(cx, &self.owner_global(), 0.0, 0.0, 0.0, 0.0)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__getElementById>
    fn GetElementById(&self, element_id: DOMString) -> Option<DomRoot<Element>> {
        let id = Atom::from(element_id.str().as_ref());
        self.upcast::<Node>()
            .traverse_preorder(ShadowIncluding::No)
            .skip(1)
            .filter_map(DomRoot::downcast::<Element>)
            .find(|element| element.Id().str().as_ref() == &*id)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__suspendRedraw>
    fn SuspendRedraw(&self, _max_wait_milliseconds: u32) -> u32 {
        1
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__unsuspendRedraw>
    fn UnsuspendRedraw(&self, _suspend_handle_id: u32) {}

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__unsuspendRedrawAll>
    fn UnsuspendRedrawAll(&self) {}

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__forceRedraw>
    fn ForceRedraw(&self) {}

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGAngle>
    fn CreateSVGAngle(&self, cx: &mut JSContext) -> DomRoot<SVGAngle> {
        SVGAngle::new_detached(cx, &self.owner_global())
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGPoint>
    fn CreateSVGPoint(&self, cx: &mut JSContext) -> DomRoot<DOMPoint> {
        DOMPoint::new_svg(cx, &self.owner_global(), 0.0, 0.0)
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGTransform>
    fn CreateSVGTransform(&self, cx: &mut JSContext) -> DomRoot<SVGTransform> {
        SVGTransform::new(
            cx,
            &self.owner_global(),
            SVGTransformValue::matrix(kurbo::Affine::IDENTITY),
            None,
            false,
        )
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGTransformFromMatrix>
    fn CreateSVGTransformFromMatrix(
        &self,
        cx: &mut JSContext,
        matrix: &DOMMatrix2DInit,
    ) -> Fallible<DomRoot<SVGTransform>> {
        let matrix = dommatrix2dinit_to_matrix(matrix)?;
        Ok(SVGTransform::new(
            cx,
            &self.owner_global(),
            SVGTransformValue::matrix(kurbo::Affine::new([
                matrix.m11, matrix.m12, matrix.m21, matrix.m22, matrix.m31, matrix.m32,
            ])),
            None,
            false,
        ))
    }

    /// <https://svgwg.org/svg2-draft/struct.html#__svg__SVGSVGElement__createSVGLength>
    fn CreateSVGLength(&self, cx: &mut JSContext) -> DomRoot<SVGLength> {
        SVGLength::new_detached(cx, &self.owner_global())
    }
}
