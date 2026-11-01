/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::rust::HandleObject;
use script_bindings::domstring::TracedDOMString;
use servo_config::pref;

use crate::dom::bindings::codegen::Bindings::CharacterDataBinding::CharacterDataMethods;
use crate::dom::bindings::codegen::Bindings::ProcessingInstructionBinding::ProcessingInstructionMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::WindowMethods;
use crate::dom::bindings::domname::is_valid_attribute_local_name;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::xmlname::matches_name_production;
use crate::dom::characterdata::CharacterData;
use crate::dom::characterdata::pi_attributes::{self, PseudoAttribute};
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::window::Window;

/// An HTML processing instruction node.
#[dom_struct]
pub(crate) struct ProcessingInstruction {
    characterdata: CharacterData,
    target: TracedDOMString,
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
        }
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
}
