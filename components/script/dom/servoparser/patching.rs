/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Declarative partial updates: `<template for>` elements that the parser does not insert but that
//! replace the content between the `<?start name="...">` and `<?end>` processing instructions that
//! they name.
//!
//! <https://github.com/WICG/declarative-partial-updates>

use js::context::JSContext;

use crate::dom::bindings::codegen::Bindings::ProcessingInstructionBinding::ProcessingInstructionMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::characterdata::CharacterData;
use crate::dom::characterdata::processinginstruction::ProcessingInstruction;
use crate::dom::document::Document;
use crate::dom::document::documentfragment::DocumentFragment;
use crate::dom::element::Element;
use crate::dom::html::htmltemplateelement::HTMLTemplateElement;
use crate::dom::bindings::codegen::Bindings::NodeBinding::{GetRootNodeOptions, NodeMethods};
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use html5ever::local_name;

/// A `<template for>` element that the parser is in.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct TemplatePatch {
    template: Dom<HTMLTemplateElement>,
    /// The node that the content of the template is inserted in.
    container: Dom<Node>,
    /// The child of `container` that the content is inserted before, or `None` for the end.
    before: Option<Dom<Node>>,
    /// The processing instructions that mark the range, which are removed when the template ends.
    markers: Vec<Dom<Node>>,
    /// Whether the content is only put in the range when the template ends.
    buffered: bool,
    /// Whether the content is sanitized, so that the scripts in it are dropped.
    sanitize: bool,
}

/// The name that a marker processing instruction gives to its range.
fn marker_name(data: &str) -> Option<String> {
    let rest = data
        .split_once("name")?
        .1
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let quote = rest
        .chars()
        .next()
        .filter(|quote| *quote == '"' || *quote == '\'')?;
    let rest = &rest[1..];
    Some(rest[..rest.find(quote)?].to_owned())
}

fn instruction_target(node: &Node) -> Option<String> {
    node.downcast::<ProcessingInstruction>()
        .map(|instruction| instruction.Target().to_string())
}

impl Document {
    /// Called when the parser is about to insert the template element `template` in `parent`. If it
    /// is a `<template for>` whose range exists, what it contains replaces that range.
    pub(crate) fn begin_template_patch(
        &self,
        cx: &mut JSContext,
        template: &HTMLTemplateElement,
        parent: &Node,
    ) {
        let element = template.upcast::<Element>();
        // A valid `shadowrootmode` makes the template declare a shadow root instead.
        if element
            .get_attribute_string_value(&local_name!("shadowrootmode"))
            .is_some_and(|mode| {
                mode.eq_ignore_ascii_case("open") || mode.eq_ignore_ascii_case("closed")
            })
        {
            return;
        }
        let Some(name) = element.get_attribute_string_value(&local_name!("for")) else {
            return;
        };

        let buffered = element.has_attribute(&html5ever::LocalName::from("buffer"));
        // A `sanitize` attribute that is empty or the keyword "sanitize" drops the scripts.
        let sanitize = element
            .get_attribute_string_value(&html5ever::LocalName::from("sanitize"))
            .is_some_and(|value| value.is_empty() || value.eq_ignore_ascii_case("sanitize"));

        // A template with an empty name is an ordinary template.
        if name.is_empty() {
            return;
        }

        // Find the start of the range, or the marker that stands for an empty one.
        let root = parent.GetRootNode(&GetRootNodeOptions::empty());
        let Some(start) = root.traverse_preorder(ShadowIncluding::Yes).find(|node| {
            instruction_target(node).is_some_and(|target| target == "start" || target == "marker") &&
                node.downcast::<ProcessingInstruction>().is_some_and(|instruction| {
                    marker_name(&instruction.upcast::<CharacterData>().data())
                        .is_some_and(|marker| marker == name)
                })
        }) else {
            return;
        };
        let Some(container) = start.GetParentNode() else {
            return;
        };

        let mut markers = vec![Dom::from_ref(&*start)];
        let before = if instruction_target(&start).is_some_and(|target| target == "marker") {
            Some(Dom::from_ref(&*start))
        } else {
            // The end is the next end among the siblings that is not for a range inside it.
            let mut depth = 0;
            let mut end = None;
            let mut sibling = start.GetNextSibling();
            while let Some(node) = sibling {
                match instruction_target(&node).as_deref() {
                    Some("start") => depth += 1,
                    Some("end") if depth == 0 => {
                        end = Some(node.clone());
                        break;
                    },
                    Some("end") => depth -= 1,
                    _ => {},
                }
                sibling = node.GetNextSibling();
            }
            if let Some(end) = end.as_ref() {
                markers.push(Dom::from_ref(&**end));
            }
            end.as_deref().map(Dom::from_ref)
        };

        let patch = TemplatePatch {
            template: Dom::from_ref(template),
            container: Dom::from_ref(&container),
            before,
            markers,
            buffered,
            sanitize,
        };
        if !buffered {
            Self::clear_patch_range(cx, &patch);
        }
        self.template_patches().borrow_mut().push(patch);
    }

    /// Removes what is in a range, which is after its start and before its end.
    fn clear_patch_range(cx: &mut JSContext, patch: &TemplatePatch) {
        if patch.markers.first().is_none_or(|marker| {
            instruction_target(marker).is_some_and(|target| target == "marker")
        }) {
            return;
        }
        // Only what is in the range now is removed: removing a node can run script, which can add
        // other nodes to the range.
        let start = &patch.markers[0];
        let mut in_range = Vec::new();
        let mut sibling = start.GetNextSibling();
        while let Some(node) = sibling {
            if patch.before.as_ref().is_some_and(|end| **end == *node) {
                break;
            }
            sibling = node.GetNextSibling();
            in_range.push(node);
        }
        for node in in_range {
            if node
                .GetParentNode()
                .is_some_and(|parent| *parent == *patch.container)
            {
                node.remove_self(cx);
            }
        }
    }

    /// Where the children of the content of a template that replaces a range are inserted, as the
    /// parent and the child to insert them before, if `parent` is that content and it is not buffered.
    pub(crate) fn template_patch_destination(
        parent: &Node,
    ) -> Option<(DomRoot<Node>, Option<DomRoot<Node>>, bool)> {
        let host = parent.downcast::<DocumentFragment>()?.host()?;
        let template = host.downcast::<HTMLTemplateElement>()?;
        let document = template.upcast::<Node>().owner_doc();
        let patches = document.template_patches().borrow();
        patches
            .iter()
            .find(|patch| &*patch.template == template && !patch.buffered)
            .map(|patch| {
                (
                    patch.container.as_rooted(),
                    patch.before.as_ref().map(|before| before.as_rooted()),
                    patch.sanitize,
                )
            })
    }

    /// Called when the parser has finished the element `element`.
    pub(crate) fn finish_template_patch(&self, cx: &mut JSContext, element: &Element) {
        if self.template_patches().borrow().is_empty() {
            return;
        }
        let Some(template) = element.downcast::<HTMLTemplateElement>() else {
            return;
        };
        let patch = {
            let mut patches = self.template_patches().borrow_mut();
            let Some(index) = patches
                .iter()
                .position(|patch| &*patch.template == template)
            else {
                return;
            };
            patches.remove(index)
        };

        // A buffered template puts its content in the range when it is done.
        if patch.buffered {
            Self::clear_patch_range(cx, &patch);
            let mut children = Vec::new();
            if let Some(contents) = template.existing_contents() {
                let node = contents.upcast::<Node>();
                while let Some(child) = node.GetFirstChild() {
                    child.remove_self(cx);
                    children.push(child);
                }
            }
            // The content goes in the range at once, so that its scripts see all of it.
            let fragment = DocumentFragment::new(cx, self);
            for child in children {
                if patch.sanitize {
                    // Sanitized content has no scripts.
                    if child
                        .downcast::<Element>()
                        .is_some_and(|element| element.local_name() == &local_name!("script"))
                    {
                        continue;
                    }
                    let scripts: Vec<DomRoot<Node>> = child
                        .traverse_preorder(ShadowIncluding::No)
                        .filter(|node| {
                            node.downcast::<Element>()
                                .is_some_and(|element| element.local_name() == &local_name!("script"))
                        })
                        .collect();
                    for script in scripts {
                        script.remove_self(cx);
                    }
                }
                let _ = fragment.upcast::<Node>().AppendChild(cx, &child);
            }
            let _ = patch
                .container
                .InsertBefore(cx, fragment.upcast(), patch.before.as_deref());
        }

        // The markers of the range and the template are only there until the template ends.
        for marker in patch.markers.iter() {
            marker.remove_self(cx);
        }
        element.upcast::<Node>().remove_self(cx);
    }
}
