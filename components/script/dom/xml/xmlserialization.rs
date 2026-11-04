/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The XML serialization algorithm of the DOM Parsing specification:
//! <https://w3c.github.io/DOM-Parsing/#xml-serialization>
//!
//! The algorithm is written recursively in the specification. This implementation keeps an
//! explicit stack instead, so that deep trees cannot overflow the native stack.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use html5ever::{LocalName, Namespace, Prefix, local_name, ns};
use script_bindings::codegen::InheritTypes::TextTypeId;

use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::inheritance::{Castable, CharacterDataTypeId, NodeTypeId};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::xmlname::matches_name_production;
use crate::dom::characterdata::CharacterData;
use crate::dom::document::Document;
use crate::dom::documenttype::DocumentType;
use crate::dom::element::Element;
use crate::dom::html::htmltemplateelement::HTMLTemplateElement;
use crate::dom::node::Node;
use crate::dom::processinginstruction::ProcessingInstruction;

/// The serialization of a node would not be well-formed, and the caller asked for a well-formed
/// serialization. Callers turn this into an "InvalidStateError" DOMException.
#[derive(Debug)]
pub(crate) struct NotWellFormed;

type SerializationResult = Result<(), NotWellFormed>;

/// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serialization>
pub(crate) fn serialize_node(
    node: &Node,
    require_well_formed: bool,
) -> Result<String, NotWellFormed> {
    let mut serializer = XmlSerializer::new(require_well_formed);
    serializer.run(vec![Step::Node(
        DomRoot::from_ref(node),
        ns!(),
        Rc::new(NamespacePrefixMap::new()),
    )])?;
    Ok(serializer.markup)
}

/// The XML serialization of the children of `node` (or of its template contents, if `node` is a
/// `template` element), each serialized as if it had no parent. This is what `innerHTML` returns
/// in XML documents.
pub(crate) fn serialize_children(
    node: &Node,
    require_well_formed: bool,
) -> Result<String, NotWellFormed> {
    let mut serializer = XmlSerializer::new(require_well_formed);
    let prefix_map = Rc::new(NamespacePrefixMap::new());
    let steps = children_to_serialize(node)
        .map(|child| Step::Node(child, ns!(), prefix_map.clone()))
        .collect();
    serializer.run(steps)?;
    Ok(serializer.markup)
}

/// The children of `node` that its serialization contains, last child first. For a `template`
/// element in the HTML namespace, these are the children of its template contents.
fn children_to_serialize(node: &Node) -> Box<dyn Iterator<Item = DomRoot<Node>>> {
    if let Some(template) = node.downcast::<HTMLTemplateElement>() {
        return match template.existing_contents() {
            Some(contents) => Box::new(contents.upcast::<Node>().rev_children()),
            None => Box::new(std::iter::empty()),
        };
    }
    Box::new(node.rev_children())
}

/// One step of the serialization.
enum Step {
    /// Run the XML serialization algorithm on a node, given the context namespace and the
    /// namespace prefix map.
    Node(DomRoot<Node>, Namespace, Rc<NamespacePrefixMap>),
    /// Write the end tag of an element whose children have been serialized.
    EndTag(String),
}

/// <https://w3c.github.io/DOM-Parsing/#dfn-namespace-prefix-map>
///
/// The null namespace is represented by the empty namespace.
#[derive(Clone, Default)]
struct NamespacePrefixMap(HashMap<Namespace, Vec<Prefix>>);

impl NamespacePrefixMap {
    /// A map that contains only the "xml" prefix, as the serialization starts with.
    fn new() -> Self {
        let mut map = Self::default();
        map.add(Prefix::from("xml"), ns!(xml));
        map
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-retrieving-a-preferred-prefix-string>
    fn preferred_prefix(
        &self,
        preferred: Option<&Prefix>,
        namespace: &Namespace,
    ) -> Option<Prefix> {
        let candidates = self.0.get(namespace)?;
        if let Some(preferred) = preferred &&
            candidates.contains(preferred)
        {
            return Some(preferred.clone());
        }
        candidates.last().cloned()
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-found>
    fn contains(&self, prefix: &Prefix, namespace: &Namespace) -> bool {
        self.0
            .get(namespace)
            .is_some_and(|candidates| candidates.contains(prefix))
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-add>
    fn add(&mut self, prefix: Prefix, namespace: Namespace) {
        self.0.entry(namespace).or_default().push(prefix);
    }
}

/// A copy of an attribute, so that the element's attribute list is not borrowed while its
/// children are serialized.
struct SerializedAttribute {
    namespace: Namespace,
    prefix: Option<Prefix>,
    local_name: LocalName,
    value: String,
}

impl SerializedAttribute {
    /// Whether this attribute declares the default namespace: an `xmlns` attribute in the XMLNS
    /// namespace, as the specification has it, or (as browsers also treat it) one in no
    /// namespace, which is what `setAttribute("xmlns", ...)` and the HTML parser create.
    /// Treating the latter as a declaration keeps the serialization from writing two `xmlns`
    /// attributes on one element.
    fn is_default_namespace_declaration(&self) -> bool {
        self.prefix.is_none() &&
            self.local_name == local_name!("xmlns") &&
            (self.namespace == ns!(xmlns) || self.namespace == ns!())
    }
}

struct XmlSerializer {
    markup: String,
    /// <https://w3c.github.io/DOM-Parsing/#dfn-generate-a-prefix>
    prefix_index: u32,
    require_well_formed: bool,
}

impl XmlSerializer {
    fn new(require_well_formed: bool) -> Self {
        Self {
            markup: String::new(),
            prefix_index: 1,
            require_well_formed,
        }
    }

    /// Fail if a well-formed serialization is required and `condition` holds.
    fn check(&self, condition: bool) -> SerializationResult {
        if self.require_well_formed && condition {
            return Err(NotWellFormed);
        }
        Ok(())
    }

    /// Run the steps on the stack until it is empty. Steps are taken from the end.
    fn run(&mut self, mut steps: Vec<Step>) -> SerializationResult {
        steps.reverse();
        while let Some(step) = steps.pop() {
            match step {
                Step::Node(node, namespace, prefix_map) => {
                    self.serialize(&node, namespace, &prefix_map, &mut steps)?
                },
                Step::EndTag(qualified_name) => {
                    self.markup.push_str("</");
                    self.markup.push_str(&qualified_name);
                    self.markup.push('>');
                },
            }
        }
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serialization-algorithm>
    ///
    /// Steps for the node's children are pushed onto `steps` instead of being run here.
    fn serialize(
        &mut self,
        node: &Node,
        namespace: Namespace,
        prefix_map: &Rc<NamespacePrefixMap>,
        steps: &mut Vec<Step>,
    ) -> SerializationResult {
        match node.type_id() {
            NodeTypeId::Element(_) => {
                let element = node.downcast::<Element>().unwrap();
                self.serialize_element(element, namespace, prefix_map, steps)
            },
            NodeTypeId::Document(_) => {
                // <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-document-node>
                // Step 1. If require well-formed is true, and node's document element is null,
                // then throw an exception.
                let document = node.downcast::<Document>().unwrap();
                self.check(document.GetDocumentElement().is_none())?;
                // Step 2. Otherwise, serialize each of node's children.
                push_children(steps, node.rev_children(), &namespace, prefix_map);
                Ok(())
            },
            NodeTypeId::DocumentFragment(_) => {
                // <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-documentfragment-node>
                push_children(steps, node.rev_children(), &namespace, prefix_map);
                Ok(())
            },
            NodeTypeId::CharacterData(CharacterDataTypeId::Comment) => {
                self.serialize_comment(&node.downcast::<CharacterData>().unwrap().data())
            },
            NodeTypeId::CharacterData(CharacterDataTypeId::Text(TextTypeId::CDATASection)) => {
                // <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-cdatasection-node>
                self.markup.push_str("<![CDATA[");
                self.markup
                    .push_str(&node.downcast::<CharacterData>().unwrap().data());
                self.markup.push_str("]]>");
                Ok(())
            },
            NodeTypeId::CharacterData(CharacterDataTypeId::Text(TextTypeId::Text)) => {
                self.serialize_text(&node.downcast::<CharacterData>().unwrap().data())
            },
            NodeTypeId::CharacterData(CharacterDataTypeId::ProcessingInstruction) => {
                let processing_instruction = node.downcast::<ProcessingInstruction>().unwrap();
                self.serialize_processing_instruction(
                    &processing_instruction.target().str(),
                    &processing_instruction.upcast::<CharacterData>().data(),
                )
            },
            NodeTypeId::DocumentType => {
                let doctype = node.downcast::<DocumentType>().unwrap();
                self.serialize_doctype(
                    &doctype.name().str(),
                    &doctype.public_id().str(),
                    &doctype.system_id().str(),
                )
            },
            // An Attr serializes as the empty string.
            NodeTypeId::Attr => Ok(()),
        }
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-an-element-node>
    fn serialize_element(
        &mut self,
        element: &Element,
        namespace: Namespace,
        prefix_map: &NamespacePrefixMap,
        steps: &mut Vec<Step>,
    ) -> SerializationResult {
        let local_name = element.local_name();

        // Step 1. If require well-formed is true, and node's local name contains the character
        // ":" or does not match the XML Name production, then throw an exception.
        self.check(local_name.contains(':') || !matches_name_production(local_name))?;

        // Step 2. Let markup be "<".
        self.markup.push('<');

        // Step 3-5. Let qualified name be the empty string, skip end tag be false and ignore
        // namespace definition attribute be false.
        let qualified_name;
        let mut ignore_namespace_definition_attribute = false;

        // Step 6. Let map be a copy of prefix map.
        let mut map = prefix_map.clone();

        // Step 7. Let local prefixes map be an empty map. (Null namespaces are stored as the
        // empty namespace.)
        let mut local_prefixes_map = HashMap::new();

        let attributes: Vec<_> = element
            .attrs()
            .borrow()
            .iter()
            .map(|attribute| SerializedAttribute {
                namespace: attribute.namespace().clone(),
                prefix: attribute.prefix().cloned(),
                local_name: attribute.local_name().clone(),
                value: String::from(&**attribute.value()),
            })
            .collect();

        // Step 8. Let local default namespace be the result of recording the namespace
        // information for node given map and local prefixes map.
        let local_default_namespace =
            record_namespace_information(&attributes, &mut map, &mut local_prefixes_map);

        // Step 9. Let inherited ns be namespace.
        let mut inherited_ns = namespace;

        // Step 10. Let ns be node's namespace.
        let ns = element.namespace().clone();

        if inherited_ns == ns {
            // Step 11. If inherited ns is ns, then:
            // Step 11.1. If local default namespace is not null, then set ignore namespace
            // definition attribute to true.
            ignore_namespace_definition_attribute = local_default_namespace.is_some();

            // Step 11.2-11.3. Use the "xml:" prefix for the XML namespace, and no prefix
            // otherwise.
            qualified_name = if ns == ns!(xml) {
                format!("xml:{local_name}")
            } else {
                local_name.to_string()
            };

            // Step 11.4. Append qualified name to markup.
            self.markup.push_str(&qualified_name);
        } else {
            // Step 12. Otherwise:
            // Step 12.1. Let prefix be node's namespace prefix.
            let prefix = element.prefix().clone();

            // Step 12.2. Let candidate prefix be the result of retrieving a preferred prefix
            // string prefix from map given ns.
            let mut candidate_prefix = map.preferred_prefix(prefix.as_ref(), &ns);

            // Step 12.3. If prefix is "xmlns", then:
            if prefix.as_deref() == Some("xmlns") {
                // Step 12.3.1. If require well-formed is true, then throw an exception.
                self.check(true)?;
                // Step 12.3.2. Set candidate prefix to prefix.
                candidate_prefix = prefix.clone();
            }

            if let Some(candidate_prefix) = candidate_prefix {
                // Step 12.4. Found a suitable namespace prefix: if candidate prefix is not null:
                // Step 12.4.1. Append candidate prefix, ":" and node's local name to qualified
                // name.
                qualified_name = format!("{candidate_prefix}:{local_name}");

                // Step 12.4.2. If local default namespace is not null and is not the XML
                // namespace, then set inherited ns to local default namespace (null if it is the
                // empty string).
                if let Some(local_default_namespace) = &local_default_namespace &&
                    *local_default_namespace != ns!(xml)
                {
                    inherited_ns = local_default_namespace.clone();
                }

                // Step 12.4.3. Append qualified name to markup.
                self.markup.push_str(&qualified_name);
            } else if let Some(mut prefix) = prefix {
                // Step 12.5. Otherwise, if prefix is not null, then:
                // Step 12.5.1. If local prefixes map contains prefix, then set prefix to the
                // result of generating a prefix given map, ns, and prefix index.
                if local_prefixes_map.contains_key(&prefix) {
                    prefix = self.generate_prefix(&mut map, &ns);
                }

                // Step 12.5.2. Add prefix to map given ns.
                map.add(prefix.clone(), ns.clone());

                // Step 12.5.3-12.5.4. Append prefix, ":" and node's local name to qualified
                // name, and qualified name to markup.
                qualified_name = format!("{prefix}:{local_name}");
                self.markup.push_str(&qualified_name);

                // Step 12.5.5. Declare the prefix.
                self.markup.push_str(" xmlns:");
                self.markup.push_str(&prefix);
                self.markup.push_str("=\"");
                self.serialize_attribute_value(&ns)?;
                self.markup.push('"');

                // Step 12.5.6. If local default namespace is not null, then set inherited ns to
                // local default namespace (null if it is the empty string).
                if let Some(local_default_namespace) = &local_default_namespace {
                    inherited_ns = local_default_namespace.clone();
                }
            } else if local_default_namespace
                .as_ref()
                // The empty string (no namespace) is never equal to ns here, because ns is
                // either null or a non-empty namespace.
                .is_none_or(|local_default| *local_default == ns!() || *local_default != ns)
            {
                // Step 12.6. Otherwise, if local default namespace is null, or local default
                // namespace is not null and its value is not equal to ns, then:
                // Step 12.6.1. Set ignore namespace definition attribute to true.
                ignore_namespace_definition_attribute = true;

                // Step 12.6.2-12.6.4. Append node's local name to qualified name, set inherited
                // ns to ns and append qualified name to markup.
                qualified_name = local_name.to_string();
                inherited_ns = ns.clone();
                self.markup.push_str(&qualified_name);

                // Step 12.6.5. Declare the default namespace.
                self.markup.push_str(" xmlns=\"");
                self.serialize_attribute_value(&ns)?;
                self.markup.push('"');
            } else {
                // Step 12.7. Otherwise (local default namespace is ns), append node's local name
                // to qualified name and to markup, and set inherited ns to ns.
                qualified_name = local_name.to_string();
                inherited_ns = ns.clone();
                self.markup.push_str(&qualified_name);
            }
        }

        // Step 13. Append the XML serialization of the attributes of node.
        self.serialize_attributes(
            &attributes,
            &mut map,
            &mut local_prefixes_map,
            ignore_namespace_definition_attribute,
        )?;

        let node = element.upcast::<Node>();
        let has_children = node.children_count() > 0;

        // Step 14. If ns is the HTML namespace, and node's children is empty, and node's local
        // name is a void element name, then append " /" to markup and set skip end tag to
        // true.
        // Step 15. If ns is not the HTML namespace, and node's children is empty, then append
        // "/" to markup and set skip end tag to true.
        let skip_end_tag = if has_children {
            false
        } else if ns == ns!(html) {
            if is_void_element_name(local_name) {
                self.markup.push_str(" /");
                true
            } else {
                false
            }
        } else {
            self.markup.push('/');
            true
        };

        // Step 16. Append ">" to markup.
        self.markup.push('>');

        // Step 17. If skip end tag is true, then return markup.
        if skip_end_tag {
            return Ok(());
        }

        // Step 20. Append "</", qualified name and ">" to markup, after the children.
        steps.push(Step::EndTag(qualified_name));

        // Step 18. If ns is the HTML namespace, and node's local name is "template", serialize
        // its template contents.
        // Step 19. Otherwise, serialize each of node's children, given inherited ns and map.
        push_children(
            steps,
            children_to_serialize(node),
            &inherited_ns,
            &Rc::new(map),
        );
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serialization-of-the-attributes>
    fn serialize_attributes(
        &mut self,
        attributes: &[SerializedAttribute],
        map: &mut NamespacePrefixMap,
        local_prefixes_map: &mut HashMap<Prefix, Namespace>,
        ignore_namespace_definition_attribute: bool,
    ) -> SerializationResult {
        // Step 2. Let localname set be an empty set.
        let mut localname_set = HashSet::new();

        // Step 3. For each attr of element's attribute list:
        for attribute in attributes {
            // Step 3.1-3.4. Fail on two attributes with the same namespace and local name.
            let attribute_namespace = &attribute.namespace;
            let is_new = localname_set.insert((attribute_namespace, &attribute.local_name));
            self.check(!is_new)?;

            // Step 3.5. Let candidate prefix be null.
            let mut candidate_prefix = None;

            // Step 3.6. If attribute namespace is not null, then:
            if *attribute_namespace != ns!() {
                // Step 3.6.1. Let candidate prefix be the result of retrieving a preferred prefix
                // string from map given attribute namespace.
                let prefix = attribute.prefix.as_ref();
                candidate_prefix = map.preferred_prefix(prefix, attribute_namespace);

                if *attribute_namespace == ns!(xmlns) {
                    // Step 3.6.2. If attribute namespace is the XMLNS namespace, then:
                    // Step 3.6.2.1. Skip the attribute if its value is the XML namespace, if it
                    // is the default namespace declaration and that is to be ignored, or if it
                    // redeclares a prefix that an ancestor already declared.
                    let declared_namespace = Namespace::from(&*attribute.value);
                    let declared_prefix = Prefix::from(&*attribute.local_name);
                    let redundant_prefix_declaration = prefix.is_some() &&
                        local_prefixes_map.get(&declared_prefix) != Some(&declared_namespace) &&
                        map.contains(&declared_prefix, &declared_namespace);
                    if declared_namespace == ns!(xml) ||
                        (prefix.is_none() && ignore_namespace_definition_attribute) ||
                        redundant_prefix_declaration
                    {
                        continue;
                    }

                    // Step 3.6.2.2. If require well-formed is true, and attr's value is the
                    // XMLNS namespace, then throw an exception.
                    self.check(declared_namespace == ns!(xmlns))?;

                    // Step 3.6.2.3. If require well-formed is true, and attr's value is the
                    // empty string, then throw an exception. (Namespace prefix declarations
                    // cannot undeclare a namespace; an empty default namespace declaration is
                    // fine.)
                    self.check(prefix.is_some() && attribute.value.is_empty())?;

                    // Step 3.6.2.4. If prefix is "xmlns", then set candidate prefix to "xmlns".
                    if prefix.is_some_and(|prefix| &**prefix == "xmlns") {
                        candidate_prefix = prefix.cloned();
                    }
                } else if candidate_prefix.is_none() {
                    // Step 3.6.3. Otherwise, if candidate prefix is null:
                    // Step 3.6.3.1. Keep the attribute's prefix if this element does not declare
                    // it, and generate one otherwise.
                    let new_prefix = match prefix {
                        Some(prefix) if !local_prefixes_map.contains_key(prefix) => prefix.clone(),
                        _ => self.generate_prefix(map, attribute_namespace),
                    };

                    // Step 3.6.3.2. Add candidate prefix to map given attribute namespace.
                    map.add(new_prefix.clone(), attribute_namespace.clone());

                    // Step 3.6.3.3-3.6.3.4. Set local prefixes map[candidate prefix] to the
                    // attribute namespace.
                    local_prefixes_map.insert(new_prefix.clone(), attribute_namespace.clone());

                    // Step 3.6.3.5. Declare the prefix.
                    self.markup.push_str(" xmlns:");
                    self.markup.push_str(&new_prefix);
                    self.markup.push_str("=\"");
                    self.serialize_attribute_value(attribute_namespace)?;
                    self.markup.push('"');

                    candidate_prefix = Some(new_prefix);
                }
            } else if ignore_namespace_definition_attribute &&
                attribute.is_default_namespace_declaration()
            {
                // An `xmlns` attribute in no namespace declares the default namespace just like
                // one in the XMLNS namespace, and is skipped like one.
                continue;
            }

            // Step 3.7. Append " " to result.
            self.markup.push(' ');

            // Step 3.8. If candidate prefix is not null, then append candidate prefix and ":"
            // to result.
            if let Some(candidate_prefix) = candidate_prefix {
                self.markup.push_str(&candidate_prefix);
                self.markup.push(':');
            }

            // Step 3.9. If require well-formed is true, and attr's local name contains ":", does
            // not match the XML Name production, or is "xmlns" in no namespace, then throw an
            // exception.
            let local_name = &attribute.local_name;
            self.check(
                local_name.contains(':') ||
                    !matches_name_production(local_name) ||
                    (*local_name == local_name!("xmlns") && *attribute_namespace == ns!()),
            )?;

            // Step 3.10. Append attr's local name, "=\"", the serialized value and "\"".
            self.markup.push_str(local_name);
            self.markup.push_str("=\"");
            self.serialize_attribute_value(&attribute.value)?;
            self.markup.push('"');
        }
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-serializing-an-attribute-value>
    fn serialize_attribute_value(&mut self, value: &str) -> SerializationResult {
        // Step 1. If require well-formed is true, and attribute value contains characters that
        // are not matched by the XML Char production, then throw an exception.
        self.check(!value.chars().all(is_xml_char))?;

        // Step 2-3. Return attribute value, escaped. Browsers also escape ">", and so does this.
        for character in value.chars() {
            match character {
                '&' => self.markup.push_str("&amp;"),
                '"' => self.markup.push_str("&quot;"),
                '<' => self.markup.push_str("&lt;"),
                '>' => self.markup.push_str("&gt;"),
                '\t' => self.markup.push_str("&#9;"),
                '\n' => self.markup.push_str("&#xA;"),
                '\r' => self.markup.push_str("&#xD;"),
                _ => self.markup.push(character),
            }
        }
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-generate-a-prefix>
    fn generate_prefix(&mut self, map: &mut NamespacePrefixMap, namespace: &Namespace) -> Prefix {
        // Step 1. Let generated prefix be the concatenation of "ns" and prefix index.
        let generated_prefix = Prefix::from(format!("ns{}", self.prefix_index));
        // Step 2. Increment prefix index by one.
        self.prefix_index += 1;
        // Step 3. Add generated prefix to map given new namespace.
        map.add(generated_prefix.clone(), namespace.clone());
        // Step 4. Return generated prefix.
        generated_prefix
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-comment-node>
    fn serialize_comment(&mut self, data: &str) -> SerializationResult {
        // Step 1. If require well-formed is true, and node's data contains characters that are
        // not matched by the XML Char production or contains "--" or ends with "-", then throw
        // an exception.
        self.check(!data.chars().all(is_xml_char) || data.contains("--") || data.ends_with('-'))?;

        // Step 2. Otherwise, return "<!--", node's data and "-->".
        self.markup.push_str("<!--");
        self.markup.push_str(data);
        self.markup.push_str("-->");
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-text-node>
    fn serialize_text(&mut self, data: &str) -> SerializationResult {
        // Step 2. If require well-formed is true, and markup contains characters that are not
        // matched by the XML Char production, then throw an exception.
        self.check(!data.chars().all(is_xml_char))?;

        // Step 3-6. Escape "&", "<" and ">".
        for character in data.chars() {
            match character {
                '&' => self.markup.push_str("&amp;"),
                '<' => self.markup.push_str("&lt;"),
                '>' => self.markup.push_str("&gt;"),
                _ => self.markup.push(character),
            }
        }
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-documenttype-node>
    fn serialize_doctype(
        &mut self,
        name: &str,
        public_id: &str,
        system_id: &str,
    ) -> SerializationResult {
        // Step 1. If require well-formed is true, and node's public ID contains characters that
        // are not matched by the XML PubidChar production, then throw an exception.
        self.check(!public_id.chars().all(is_xml_pubid_char))?;

        // Step 2. If require well-formed is true, and node's system ID contains characters that
        // are not matched by the XML Char production or contains both a """ and a "'", then
        // throw an exception.
        self.check(
            !system_id.chars().all(is_xml_char) ||
                (system_id.contains('"') && system_id.contains('\'')),
        )?;

        // Step 3-6. Append "<!DOCTYPE", " " and node's name to markup.
        self.markup.push_str("<!DOCTYPE ");
        self.markup.push_str(name);

        // Step 7. If node's public ID is not the empty string, then append " PUBLIC " and the
        // serialization of the public ID.
        if !public_id.is_empty() {
            self.markup.push_str(" PUBLIC ");
            self.serialize_id(public_id);
        }

        // Step 8. If node's system ID is not the empty string and node's public ID is the empty
        // string, then append " SYSTEM".
        if !system_id.is_empty() && public_id.is_empty() {
            self.markup.push_str(" SYSTEM");
        }

        // Step 9. If node's system ID is not the empty string, then append " " and the
        // serialization of the system ID.
        if !system_id.is_empty() {
            self.markup.push(' ');
            self.serialize_id(system_id);
        }

        // Step 10. Append ">" to markup.
        self.markup.push('>');
        Ok(())
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-serialization-of-the-id>
    fn serialize_id(&mut self, id: &str) {
        // Step 1. If id contains """, let q be "'", and let q be """ otherwise.
        let quote = if id.contains('"') { '\'' } else { '"' };
        // Step 2. Return the concatenation of q, id, and q.
        self.markup.push(quote);
        self.markup.push_str(id);
        self.markup.push(quote);
    }

    /// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-a-processinginstruction-node>
    fn serialize_processing_instruction(
        &mut self,
        target: &str,
        data: &str,
    ) -> SerializationResult {
        // Step 1. If require well-formed is true, and node's target contains a ":" character or
        // is an ASCII case-insensitive match for "xml", then throw an exception.
        self.check(target.contains(':') || target.eq_ignore_ascii_case("xml"))?;

        // Step 2. If require well-formed is true, and node's data contains characters that are
        // not matched by the XML Char production or contains "?>", then throw an exception.
        self.check(!data.chars().all(is_xml_char) || data.contains("?>"))?;

        // Step 3. Let markup be "<?", node's target, " ", node's data and "?>".
        self.markup.push_str("<?");
        self.markup.push_str(target);
        self.markup.push(' ');
        self.markup.push_str(data);
        self.markup.push_str("?>");
        Ok(())
    }
}

/// Push steps that serialize `children` (last child first) given `namespace` and `prefix_map`.
fn push_children(
    steps: &mut Vec<Step>,
    children: impl Iterator<Item = DomRoot<Node>>,
    namespace: &Namespace,
    prefix_map: &Rc<NamespacePrefixMap>,
) {
    steps.extend(children.map(|child| Step::Node(child, namespace.clone(), prefix_map.clone())));
}

/// <https://w3c.github.io/DOM-Parsing/#dfn-recording-the-namespace-information>
fn record_namespace_information(
    attributes: &[SerializedAttribute],
    map: &mut NamespacePrefixMap,
    local_prefixes_map: &mut HashMap<Prefix, Namespace>,
) -> Option<Namespace> {
    // Step 1. Let default namespace attr value be null.
    let mut default_namespace_attr_value = None;

    // Step 2. For each attr of attributes:
    for attribute in attributes {
        // Step 2.3.1. If attribute prefix is null (attr is a default namespace declaration),
        // set default namespace attr value to attr's value, and continue.
        if attribute.is_default_namespace_declaration() {
            default_namespace_attr_value = Some(Namespace::from(&*attribute.value));
            continue;
        }

        // Step 2.3. If attribute namespace is the XMLNS namespace, then:
        if attribute.namespace != ns!(xmlns) {
            continue;
        }

        // Step 2.3.2.1-2.3.2.2. Let prefix definition be attr's local name, and namespace
        // definition be attr's value (the empty string standing for no namespace).
        let prefix_definition = Prefix::from(&*attribute.local_name);
        let namespace_definition = Namespace::from(&*attribute.value);

        // Step 2.3.2.3. If namespace definition is the XML namespace, then continue.
        if namespace_definition == ns!(xml) {
            continue;
        }

        // Step 2.3.2.5. If prefix definition is found in map given namespace definition, then
        // continue.
        if map.contains(&prefix_definition, &namespace_definition) {
            continue;
        }

        // Step 2.3.2.6. Add prefix definition to map given namespace definition.
        map.add(prefix_definition.clone(), namespace_definition.clone());

        // Step 2.3.2.7-2.3.2.8. Set local prefixes map[prefix definition] to namespace
        // definition.
        local_prefixes_map.insert(prefix_definition, namespace_definition);
    }

    // Step 3. Return default namespace attr value.
    default_namespace_attr_value
}

/// The element names that step 14 of
/// <https://w3c.github.io/DOM-Parsing/#dfn-xml-serializing-an-element-node> serializes as
/// empty-element tags in the HTML namespace.
fn is_void_element_name(local_name: &LocalName) -> bool {
    matches!(
        &**local_name,
        "area" |
            "base" |
            "basefont" |
            "bgsound" |
            "br" |
            "col" |
            "embed" |
            "frame" |
            "hr" |
            "img" |
            "input" |
            "keygen" |
            "link" |
            "menuitem" |
            "meta" |
            "param" |
            "source" |
            "track" |
            "wbr"
    )
}

/// <https://www.w3.org/TR/xml/#NT-Char>
fn is_xml_char(character: char) -> bool {
    matches!(character,
        '\u{9}' | '\u{A}' | '\u{D}' |
        '\u{20}'..='\u{D7FF}' |
        '\u{E000}'..='\u{FFFD}' |
        '\u{10000}'..='\u{10FFFF}')
}

/// <https://www.w3.org/TR/xml/#NT-PubidChar>
fn is_xml_pubid_char(character: char) -> bool {
    character.is_ascii_alphanumeric() ||
        matches!(
            character,
            ' ' | '\r' |
                '\n' |
                '-' |
                '\'' |
                '(' |
                ')' |
                '+' |
                ',' |
                '.' |
                '/' |
                ':' |
                '=' |
                '?' |
                ';' |
                '!' |
                '*' |
                '#' |
                '@' |
                '$' |
                '_' |
                '%'
        )
}
