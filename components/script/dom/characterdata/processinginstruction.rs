/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use net_traits::ReferrerPolicy;
use script_bindings::cell::DomRefCell;
use script_bindings::domstring::TracedDOMString;
use servo_arc::Arc;
use servo_config::pref;
use servo_url::ServoUrl;
use style::stylesheets::Stylesheet;

use crate::css::stylesheet_loader::{
    ElementStylesheetLoader, StylesheetContextSource, StylesheetOwner,
};
use crate::dom::bindings::codegen::Bindings::CharacterDataBinding::CharacterDataMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::codegen::Bindings::ProcessingInstructionBinding::ProcessingInstructionMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::WindowMethods;
use crate::dom::bindings::domname::is_valid_attribute_local_name;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::xmlname::matches_name_production;
use crate::dom::characterdata::CharacterData;
use crate::dom::characterdata::pi_attributes::{self, PseudoAttribute};
use crate::dom::css::cssstylesheet::CSSStyleSheet;
use crate::dom::css::stylesheet::StyleSheet as DOMStyleSheet;
use crate::dom::document::Document;
use crate::dom::documentorshadowroot::StylesheetSource;
use crate::dom::element::Element;
use crate::dom::html::htmllinkelement::RequestGenerationId;
use crate::dom::medialist::MediaList;
use crate::dom::node::virtualmethods::VirtualMethods;
use crate::dom::node::{BindContext, Node, NodeTraits, UnbindContext};
use crate::dom::window::Window;

/// An HTML processing instruction node.
#[dom_struct]
pub(crate) struct ProcessingInstruction {
    characterdata: CharacterData,
    target: TracedDOMString,

    /// Whether this processing instruction was created by a parser.
    parser_inserted: Cell<bool>,

    /// The style sheet of an `xml-stylesheet` processing instruction.
    #[conditional_malloc_size_of]
    #[no_trace]
    stylesheet: DomRefCell<Option<Arc<Stylesheet>>>,
    cssom_stylesheet: MutNullableDom<CSSStyleSheet>,

    /// The URL of the style sheet, which is the `href` of its `CSSStyleSheet`.
    #[no_trace]
    stylesheet_url: DomRefCell<Option<ServoUrl>>,

    /// Changed whenever the style sheet is fetched again or removed, so that the result
    /// of an earlier fetch is ignored.
    request_generation_id: Cell<RequestGenerationId>,

    /// The number of loads of the style sheet and its imports that have not finished.
    pending_loads: Cell<u32>,
}

impl ProcessingInstruction {
    fn new_inherited(
        target: DOMString,
        data: DOMString,
        document: &Document,
    ) -> ProcessingInstruction {
        ProcessingInstruction {
            characterdata: CharacterData::new_inherited(data, document),
            target: target.traced(),
            parser_inserted: Cell::new(false),
            stylesheet: DomRefCell::new(None),
            cssom_stylesheet: MutNullableDom::new(None),
            stylesheet_url: DomRefCell::new(None),
            request_generation_id: Cell::new(RequestGenerationId::default()),
            pending_loads: Cell::new(0),
        }
    }

    /// Note that this processing instruction was created by a parser.
    pub(crate) fn set_parser_inserted(&self) {
        self.parser_inserted.set(true);
    }

    pub(crate) fn get_request_generation_id(&self) -> RequestGenerationId {
        self.request_generation_id.get()
    }

    fn is_xml_stylesheet(&self) -> bool {
        *self.target.str() == *"xml-stylesheet"
    }

    /// The pseudo-attributes of an `xml-stylesheet` processing instruction, or `None` if
    /// this is not one, or its data does not match the pseudo-attribute grammar.
    fn xml_stylesheet_attributes(&self) -> Option<Vec<PseudoAttribute>> {
        if !self.is_xml_stylesheet() {
            return None;
        }
        pi_attributes::parse_xml_stylesheet(&self.upcast::<CharacterData>().data())
    }

    fn xml_stylesheet_attribute(&self, name: &str) -> Option<String> {
        self.xml_stylesheet_attributes()?
            .into_iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, value)| value)
    }

    /// The `charset` pseudo-attribute of this processing instruction, which gives the
    /// environment encoding of its style sheet.
    pub(crate) fn stylesheet_charset(&self) -> Option<String> {
        self.xml_stylesheet_attribute("charset")
    }

    /// The title of this processing instruction's style sheet: its `title` pseudo-attribute.
    pub(crate) fn stylesheet_title(&self) -> Option<String> {
        self.xml_stylesheet_attribute("title")
    }

    /// <https://drafts.csswg.org/cssom/#prolog>
    ///
    /// > The prolog refers to nodes that are children of the document and are not following
    /// > the Element child of the document, if any.
    fn is_in_prolog(&self) -> bool {
        let node = self.upcast::<Node>();
        node.GetParentNode()
            .is_some_and(|parent| parent.is::<Document>()) &&
            !node
                .preceding_siblings()
                .any(|sibling| sibling.is::<Element>())
    }

    /// The steps that run when an `xml-stylesheet` processing instruction becomes part of
    /// the prolog, is no longer part of the prolog, or has its data changed:
    /// <https://drafts.csswg.org/cssom/#requirements-on-user-agents-implementing-the-xml-stylesheet-processing-instruction>
    ///
    /// TODO: This does not yet run when the prolog changes because the document element
    /// is inserted or removed.
    fn update_stylesheet(&self, cx: &mut JSContext) {
        // Step 1. If an instance of this algorithm is currently running for node, abort that
        // instance, and stop the associated fetching if applicable.
        self.request_generation_id
            .set(self.request_generation_id.get().increment());

        // Step 2. If node has an associated CSS style sheet, remove it.
        self.remove_stylesheet(cx.no_gc());

        // The remaining steps only apply while the node is part of the prolog.
        if !self.upcast::<Node>().is_connected() || !self.is_in_prolog() {
            return;
        }

        // Step 3. If node is not an xml-stylesheet processing instruction, then return.
        // (The pseudo-attributes must also match their grammar.)
        let Some(attributes) = self.xml_stylesheet_attributes() else {
            return;
        };
        let attribute = |name: &str| {
            attributes
                .iter()
                .find(|(candidate, _)| candidate == name)
                .map(|(_, value)| value.as_str())
        };

        // Step 4. If node does not have an href pseudo-attribute, then return.
        let Some(href) = attribute("href") else {
            return;
        };

        // Step 5. Let title be the value of the title pseudo-attribute or the empty string if
        // the title pseudo-attribute is not specified.
        let title = attribute("title").unwrap_or_default();

        // Step 6. If there is an alternate pseudo-attribute with the value "yes" and title is
        // the empty string, then return.
        if attribute("alternate") == Some("yes") && title.is_empty() {
            return;
        }

        // Step 7. If there is a type pseudo-attribute whose value is not a supported styling
        // language the user agent may ignore the processing instruction.
        if attribute("type").is_some_and(|type_| !type_.eq_ignore_ascii_case("text/css")) {
            return;
        }

        let document = self.owner_document();
        if document.browsing_context().is_none() {
            return;
        }

        // Step 8. Let input URL be the value specified by the href pseudo-attribute.
        // Step 9. Let document be node's node document.
        // Step 10. Let base URL be document's document base URL.
        // Step 11. Let referrer be document's address.
        // Step 12. Let parsed URL be the return value of invoking the URL parser with the
        // string input URL and the base URL base URL.
        // Step 13. If parsed URL is failure, then return.
        let Ok(url) = document.base_url().join(href) else {
            return;
        };

        let media =
            MediaList::parse_media_list(attribute("media").unwrap_or_default(), document.window());
        let media = Arc::new(document.style_shared_author_lock().wrap(media));

        *self.stylesheet_url.borrow_mut() = Some(url.clone());
        self.pending_loads.set(0);

        // Step 14. Let request be a new request, whose URL is parsed URL, destination is
        // "style", initiator is "", ... and referrer is referrer.
        // Step 15. Fetch request, and create a CSS style sheet from the response.
        ElementStylesheetLoader::load_with_element(
            cx,
            self.upcast(),
            StylesheetContextSource::ProcessingInstruction,
            media,
            url,
            None,
            String::new(),
        );
    }

    /// Called when the data of this node changes.
    pub(crate) fn data_changed(&self, cx: &mut JSContext) {
        if self.is_xml_stylesheet() && (self.stylesheet.borrow().is_some() || self.is_in_prolog()) {
            self.update_stylesheet(cx);
        }
    }

    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    pub(crate) fn set_stylesheet(&self, no_gc: &NoGC, stylesheet: Arc<Stylesheet>) {
        // > alternate flag: Set if the alternate pseudo-attribute value is a case-sensitive
        // > match for "yes", or unset otherwise.
        //
        // Alternative style sheets are disabled.
        if self.xml_stylesheet_attribute("alternate").as_deref() == Some("yes") {
            stylesheet.set_disabled(true);
        }

        let owner = self.stylesheet_list_owner();
        if let Some(old_stylesheet) = self
            .stylesheet
            .safe_borrow_mut(no_gc)
            .replace(stylesheet.clone())
        {
            owner.remove_stylesheet(
                StylesheetSource::Node(Dom::from_ref(self.upcast())),
                &old_stylesheet,
            );
        }
        owner.add_owned_stylesheet(no_gc, self.upcast(), stylesheet);
    }

    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    fn remove_stylesheet(&self, no_gc: &NoGC) {
        if let Some(stylesheet) = self.stylesheet.safe_borrow_mut(no_gc).take() {
            let owner = self.stylesheet_list_owner();
            owner.remove_stylesheet(
                StylesheetSource::Node(Dom::from_ref(self.upcast())),
                &stylesheet,
            );
            if let Some(cssom_stylesheet) = self.cssom_stylesheet.get() {
                cssom_stylesheet.set_owner_node(None);
            }
            self.cssom_stylesheet.set(None);
            owner.invalidate_stylesheets(no_gc);
        }
    }

    pub(crate) fn get_cssom_stylesheet(
        &self,
        cx: &mut JSContext,
    ) -> Option<DomRoot<CSSStyleSheet>> {
        let stylesheet = self.stylesheet.borrow().clone()?;
        Some(self.cssom_stylesheet.or_init(|| {
            CSSStyleSheet::new(
                cx,
                &self.owner_window(),
                Some(self.upcast::<Node>()),
                DOMString::from_static("text/css"),
                self.stylesheet_url
                    .borrow()
                    .as_ref()
                    .map(|url| DOMString::from(url.as_str())),
                stylesheet,
                None, // constructor_document
            )
        }))
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        target: DOMString,
        data: DOMString,
        document: &Document,
    ) -> DomRoot<ProcessingInstruction> {
        Self::new_with_proto(cx, target, data, document, None)
    }

    fn new_with_proto(
        cx: &mut JSContext,
        target: DOMString,
        data: DOMString,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> DomRoot<ProcessingInstruction> {
        Node::reflect_node_with_proto(
            cx,
            Box::new(ProcessingInstruction::new_inherited(target, data, document)),
            document,
            proto,
        )
    }

    /// Create a processing instruction after validating `target` and `data`, as
    /// <https://dom.spec.whatwg.org/#dom-document-createprocessinginstruction> does.
    pub(crate) fn new_validated(
        cx: &mut JSContext,
        target: DOMString,
        data: DOMString,
        document: &Document,
        proto: Option<HandleObject>,
    ) -> Fallible<DomRoot<ProcessingInstruction>> {
        // Step 1. If target does not match the Name production, then throw an "InvalidCharacterError" DOMException.
        if !matches_name_production(&target.str()) {
            return Err(Error::InvalidCharacter(Some(
                "Target name provided is invalid".into(),
            )));
        }

        // Step 2. If data contains the string "?>", then throw an "InvalidCharacterError" DOMException.
        if data.contains("?>") {
            return Err(Error::InvalidCharacter(Some(
                "Processing instruction's data cannot contain `>?`".into(),
            )));
        }

        // Step 3.
        Ok(Self::new_with_proto(cx, target, data, document, proto))
    }

    pub(crate) fn target(&self) -> &TracedDOMString {
        &self.target
    }

    /// The pseudo-attributes in this node's data.
    fn attributes(&self) -> Vec<PseudoAttribute> {
        pi_attributes::parse(&self.upcast::<CharacterData>().data())
    }

    /// Replace the data of this node with the serialisation of `attributes`.
    fn set_attributes(&self, cx: &mut JSContext, attributes: &[PseudoAttribute]) {
        let character_data = self.upcast::<CharacterData>();
        let length = character_data.Length();
        // The offset is zero, so this cannot fail.
        let _ = character_data.ReplaceData(
            cx,
            0,
            length,
            DOMString::from(pi_attributes::serialize(attributes)),
        );
    }

    fn validate_attribute_name(name: &DOMString) -> Fallible<()> {
        if is_valid_attribute_local_name(&name.str()) {
            Ok(())
        } else {
            Err(Error::InvalidCharacter(Some(
                "Invalid attribute name".into(),
            )))
        }
    }
}

impl ProcessingInstructionMethods<crate::DomTypeHolder> for ProcessingInstruction {
    /// <https://dom.spec.whatwg.org/#dom-processinginstruction-processinginstruction>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        target: DOMString,
        data: DOMString,
    ) -> Fallible<DomRoot<ProcessingInstruction>> {
        // This constructor is only part of the tentative attributes proposal.
        if !pref!(dom_processing_instruction_attributes_enabled) {
            return Err(Error::Type(c"Illegal constructor.".to_owned()));
        }
        let document = window.Document();
        Self::new_validated(cx, target, data, &document, proto)
    }

    /// <https://dom.spec.whatwg.org/#dom-processinginstruction-target>
    fn Target(&self) -> DOMString {
        self.target.clone().root()
    }

    fn HasAttributes(&self) -> bool {
        !self.attributes().is_empty()
    }

    fn GetAttributeNames(&self) -> Vec<DOMString> {
        self.attributes()
            .into_iter()
            .map(|(name, _)| DOMString::from(name))
            .collect()
    }

    fn GetAttribute(&self, name: DOMString) -> Option<DOMString> {
        let name = name.str();
        self.attributes()
            .into_iter()
            .find(|(candidate, _)| *candidate == *name)
            .map(|(_, value)| DOMString::from(value))
    }

    fn SetAttribute(&self, cx: &mut JSContext, name: DOMString, value: DOMString) -> Fallible<()> {
        Self::validate_attribute_name(&name)?;
        let mut attributes = self.attributes();
        let value = String::from(value);
        match attributes
            .iter_mut()
            .find(|(candidate, _)| *candidate == *name.str())
        {
            Some((_, existing)) => *existing = value,
            None => attributes.push((String::from(name), value)),
        }
        self.set_attributes(cx, &attributes);
        Ok(())
    }

    fn RemoveAttribute(&self, cx: &mut JSContext, name: DOMString) {
        let mut attributes = self.attributes();
        let length = attributes.len();
        attributes.retain(|(candidate, _)| *candidate != *name.str());
        if attributes.len() != length {
            self.set_attributes(cx, &attributes);
        }
    }

    fn ToggleAttribute(
        &self,
        cx: &mut JSContext,
        name: DOMString,
        force: Option<bool>,
    ) -> Fallible<bool> {
        Self::validate_attribute_name(&name)?;
        let present = self.HasAttribute(name.clone());
        match (present, force) {
            (true, Some(true)) => Ok(true),
            (true, _) => {
                self.RemoveAttribute(cx, name);
                Ok(false)
            },
            (false, Some(false)) => Ok(false),
            (false, _) => {
                self.SetAttribute(cx, name, DOMString::new())?;
                Ok(true)
            },
        }
    }

    fn HasAttribute(&self, name: DOMString) -> bool {
        let name = name.str();
        self.attributes()
            .iter()
            .any(|(candidate, _)| *candidate == *name)
    }

    /// <https://drafts.csswg.org/cssom/#dom-linkstyle-sheet>
    fn GetSheet(&self, cx: &mut JSContext) -> Option<DomRoot<DOMStyleSheet>> {
        self.get_cssom_stylesheet(cx).map(DomRoot::upcast)
    }
}

impl StylesheetOwner for ProcessingInstruction {
    fn parser_inserted(&self) -> bool {
        self.parser_inserted.get()
    }

    fn potentially_render_blocking(&self) -> bool {
        self.parser_inserted()
    }

    fn referrer_policy(&self, _cx: &mut JSContext) -> ReferrerPolicy {
        ReferrerPolicy::EmptyString
    }

    fn increment_pending_loads_count(&self) {
        self.pending_loads.set(self.pending_loads.get() + 1)
    }

    fn load_finished(&self, _successful: bool) -> Option<bool> {
        // No load or error events are fired at processing instructions.
        self.pending_loads
            .set(self.pending_loads.get().saturating_sub(1));
        None
    }

    fn set_origin_clean(&self, cx: &mut JSContext, origin_clean: bool) {
        if let Some(stylesheet) = self.get_cssom_stylesheet(cx) {
            stylesheet.set_origin_clean(origin_clean);
        }
    }

    fn media_matches_environment(&self) -> bool {
        self.xml_stylesheet_attribute("media")
            .is_none_or(|media| MediaList::matches_environment(&self.owner_document(), &media))
    }
}

impl VirtualMethods for ProcessingInstruction {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<Node>() as &dyn VirtualMethods)
    }

    fn bind_to_tree(&self, cx: &mut JSContext, context: &BindContext) {
        if let Some(super_type) = self.super_type() {
            super_type.bind_to_tree(cx, context);
        }

        if context.tree_connected && self.is_xml_stylesheet() && self.is_in_prolog() {
            self.update_stylesheet(cx);
        }
    }

    fn unbind_from_tree(&self, cx: &mut JSContext, context: &UnbindContext) {
        if let Some(super_type) = self.super_type() {
            super_type.unbind_from_tree(cx, context);
        }

        if self.is_xml_stylesheet() {
            self.request_generation_id
                .set(self.request_generation_id.get().increment());
            self.remove_stylesheet(cx.no_gc());
        }
    }
}
