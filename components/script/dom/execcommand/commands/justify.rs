/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use html5ever::{local_name, ns};
use js::context::JSContext;
use script_bindings::inheritance::Castable;

use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::execcommand::basecommand::{CommandName, CssPropertyName};
use crate::dom::execcommand::contenteditable::node::{
    NodeOrString, is_allowed_child, wrap_node_list,
};
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::selection::Selection;

/// The alignment that the justify commands justify the selection with.
///
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifycenter-command>
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifyfull-command>
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifyleft-command>
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifyright-command>
fn alignment_for_command(command: CommandName) -> &'static str {
    match command {
        CommandName::JustifyCenter => "center",
        CommandName::JustifyFull => "justify",
        CommandName::JustifyLeft => "left",
        CommandName::JustifyRight => "right",
        _ => unreachable!("Must be a justify command"),
    }
}

fn is_html_element_with_local_name(element: &Element, names: &[&str]) -> bool {
    *element.namespace() == ns!(html) && names.contains(&&**element.local_name())
}

/// Whether the element has a style attribute which sets exactly one CSS property,
/// which is "text-align", which is set to alignment.
fn style_attribute_only_sets_text_align_to(element: &Element, alignment: &str) -> bool {
    let style_attribute = element.style_attribute().borrow();
    let Some(declarations) = style_attribute.as_ref() else {
        return false;
    };
    let document = element.owner_document();
    let shared_lock = document.style_shared_author_lock();
    let read_lock = shared_lock.read();
    if declarations.read_with(&read_lock).len() != 1 {
        return false;
    }
    CssPropertyName::TextAlign
        .value_set_for_style(element)
        .is_some_and(|value| value == alignment)
}

/// <https://w3c.github.io/editing/docs/execCommand/#justify-the-selection>
fn justify_the_selection(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
    alignment: &str,
) {
    let active_range = selection
        .active_range(cx)
        .expect("Must always have an active range.");

    // Step 1. Block-extend the active range, and let new range be the result.
    let new_range = active_range.block_extend(cx, document);

    // Step 2. Let element list be a list of all editable Elements contained in new range
    //         that either has an attribute in the HTML namespace whose local name is "align",
    //         or has a style attribute that sets "text-align", or is a center.
    let element_list: Vec<DomRoot<Element>> = new_range
        .contained_nodes(cx.no_gc())
        .filter(|node| node.is_editable())
        .filter_map(|node| node.downcast::<Element>().map(DomRoot::from_ref))
        .filter(|element| {
            element.has_attribute(&local_name!("align")) ||
                CssPropertyName::TextAlign
                    .value_set_for_style(element)
                    .is_some() ||
                is_html_element_with_local_name(element, &["center"])
        })
        .collect();

    // Step 3. For each element in element list:
    for element in element_list {
        // Step 3.1. If element has an attribute in the HTML namespace whose local name is "align",
        //           remove that attribute.
        element.remove_attribute_by_name(cx, &local_name!("align"));

        // Step 3.2. Unset the CSS property "text-align" on element, if it's set by a style attribute.
        if CssPropertyName::TextAlign
            .value_set_for_style(&element)
            .is_some() &&
            let Some(html_element) = element.downcast::<HTMLElement>()
        {
            CssPropertyName::TextAlign.remove_from_element(cx, html_element);
        }
        // In case we have a completely empty style attribute, we need to completely remove it.
        // Otherwise, when you call `innerHTML`, it would generate a `style=""`, which is
        // not what the tests expect. They expect the whole attribute to be removed.
        if element.has_empty_style_attribute() {
            element.remove_attribute_by_name(cx, &local_name!("style"));
        }

        let has_attributes = !element.attrs().borrow().is_empty();
        // Step 3.3. If element is a div or span or center with no attributes,
        //           remove it, preserving its descendants.
        if !has_attributes && is_html_element_with_local_name(&element, &["div", "span", "center"])
        {
            element
                .upcast::<Node>()
                .remove_preserving_its_descendants(cx);
        }

        // Step 3.4. If element is a center with one or more attributes,
        //           set the tag name of element to "div".
        if has_attributes && is_html_element_with_local_name(&element, &["center"]) {
            element.set_the_tag_name(cx, "div");
        }
    }

    // Step 4. Block-extend the active range, and let new range be the result.
    let active_range = selection
        .active_range(cx)
        .expect("Must always have an active range.");
    let new_range = active_range.block_extend(cx, document);

    // Step 5. Let node list be a list of nodes, initially empty.
    let mut node_list: Vec<DomRoot<Node>> = vec![];

    // Step 6. For each node node contained in new range, append node to node list if
    //         the last member of node list (if any) is not an ancestor of node;
    //         node is editable; node is an allowed child of "div";
    //         and node's alignment value is not alignment.
    for node in new_range.contained_nodes(cx.no_gc()) {
        if node_list
            .last()
            .is_none_or(|last| !last.is_ancestor_of(&node)) &&
            node.is_editable() &&
            is_allowed_child(
                NodeOrString::Node(node.clone()),
                NodeOrString::String("div".to_owned()),
            ) &&
            node.alignment_value() != alignment
        {
            node_list.push(node.as_rooted());
        }
    }

    // Step 7. While node list is not empty:
    let mut node_list_iter = node_list.into_iter().peekable();
    while let Some(first_member) = node_list_iter.next() {
        // Step 7.1. Let sublist be a list of nodes, initially empty.
        // Step 7.2. Remove the first member of node list and append it to sublist.
        let mut sublist: Vec<DomRoot<Node>> = vec![first_member];

        // Step 7.3. While node list is not empty, and the first member of node list is the
        //           nextSibling of the last member of sublist, remove the first member of
        //           node list and append it to sublist.
        while let Some(next_member) = node_list_iter.next_if(|node| {
            sublist
                .last()
                .expect("Must always have last element here.")
                .GetNextSibling()
                .is_some_and(|next_sibling| next_sibling == *node)
        }) {
            sublist.push(next_member);
        }

        // Step 7.4. Wrap sublist. Sibling criteria returns true for any div that has one or both
        //           of the following two attributes and no other attributes, and false otherwise:
        //           - An align attribute whose value is an ASCII case-insensitive match for alignment.
        //           - A style attribute which sets exactly one CSS property
        //             (including unrecognized or invalid attributes),
        //             which is "text-align", which is set to alignment.
        //
        //           New parent instructions are to call createElement("div") on the context object,
        //           then set its CSS property "text-align" to alignment and return the result.
        wrap_node_list(
            cx,
            sublist,
            |sibling| {
                let Some(sibling) = sibling.downcast::<Element>() else {
                    return false;
                };
                if !is_html_element_with_local_name(sibling, &["div"]) {
                    return false;
                }
                let attrs = sibling.attrs().borrow();
                !attrs.is_empty() &&
                    attrs.iter().all(|attr| {
                        if *attr.namespace() != ns!() {
                            return false;
                        }
                        match *attr.local_name() {
                            local_name!("align") => attr.value().eq_ignore_ascii_case(alignment),
                            local_name!("style") => {
                                style_attribute_only_sets_text_align_to(sibling, alignment)
                            },
                            _ => false,
                        }
                    })
            },
            |cx| {
                let new_parent = document.create_element(cx, "div");
                CssPropertyName::TextAlign.set_for_element(
                    cx,
                    new_parent
                        .downcast::<HTMLElement>()
                        .expect("A div is always an HTML element"),
                    DOMString::from(alignment),
                );
                Some(DomRoot::upcast(new_parent))
            },
        );
    }
}

/// The alignment values of all visible editable nodes that are contained in the
/// block-extended active range and have no children, in tree order. Returns `None`
/// if the active range is null.
fn alignment_values_of_contained_leaves(
    cx: &mut JSContext,
    document: &Document,
) -> Option<Vec<&'static str>> {
    let selection = document.GetSelection(cx)?;
    let active_range = selection.active_range(cx)?;
    let new_range = active_range.block_extend(cx, document);
    let no_gc = cx.no_gc();
    Some(
        new_range
            .contained_nodes(no_gc)
            .filter(|node| {
                node.is_visible(no_gc) && node.is_editable() && node.children_count() == 0
            })
            .map(|node| node.alignment_value())
            .collect(),
    )
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-justifycenter-command>
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifyfull-command>
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifyleft-command>
/// <https://w3c.github.io/editing/docs/execCommand/#the-justifyright-command>
pub(crate) fn execute_justify_command(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
    command: CommandName,
) -> bool {
    // > Justify the selection with alignment "center" (or "justify", "left", "right"),
    // > then return true.
    justify_the_selection(cx, document, selection, alignment_for_command(command));
    true
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-justifycenter-command>
pub(crate) fn justify_command_indeterminate(
    cx: &mut JSContext,
    document: &Document,
    command: CommandName,
) -> bool {
    let alignment = alignment_for_command(command);
    // > Return false if the active range is null. Otherwise, block-extend the active range.
    // > Return true if among visible editable nodes that are contained in the result and have
    // > no children, at least one has alignment value "center" and at least one does not.
    // > Otherwise return false.
    alignment_values_of_contained_leaves(cx, document).is_some_and(|values| {
        values.contains(&alignment) && values.iter().any(|value| *value != alignment)
    })
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-justifycenter-command>
pub(crate) fn justify_command_state(
    cx: &mut JSContext,
    document: &Document,
    command: CommandName,
) -> bool {
    let alignment = alignment_for_command(command);
    // > Return false if the active range is null. Otherwise, block-extend the active range.
    // > Return true if there is at least one visible editable node that is contained in the
    // > result and has no children, and all such nodes have alignment value "center".
    // > Otherwise return false.
    alignment_values_of_contained_leaves(cx, document)
        .is_some_and(|values| !values.is_empty() && values.iter().all(|value| *value == alignment))
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-justifycenter-command>
pub(crate) fn justify_command_value(cx: &mut JSContext, document: &Document) -> DOMString {
    // > Return the empty string if the active range is null. Otherwise, block-extend the
    // > active range, and return the alignment value of the first visible editable node that
    // > is contained in the result and has no children. If there is no such node, return "left".
    let Some(values) = alignment_values_of_contained_leaves(cx, document) else {
        return DOMString::new();
    };
    DOMString::from(values.first().copied().unwrap_or("left"))
}
