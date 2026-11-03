/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::{JSContext, NoGC};
use script_bindings::inheritance::Castable;

use crate::dom::bindings::codegen::Bindings::DocumentBinding::DocumentMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::dom::execcommand::contenteditable::node::{
    NodeOrString, is_allowed_child, record_the_values, restore_the_values, split_the_parent,
    wrap_node_list,
};
use crate::dom::html::htmlbrelement::HTMLBRElement;
use crate::dom::html::htmlelement::HTMLElement;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use crate::dom::selection::Selection;

/// <https://w3c.github.io/editing/docs/execCommand/#formattable-block-name>
const FORMATTABLE_BLOCK_NAMES: [&str; 12] = [
    "address", "dd", "div", "dt", "h1", "h2", "h3", "h4", "h5", "h6", "p", "pre",
];

/// Whether node is an HTML element whose local name is a formattable block name.
fn is_html_element_with_formattable_block_name(node: &Node) -> bool {
    node.downcast::<HTMLElement>()
        .is_some_and(|element| FORMATTABLE_BLOCK_NAMES.contains(&element.local_name()))
}

/// Whether node is the ancestor of a prohibited paragraph child.
fn is_ancestor_of_prohibited_paragraph_child(no_gc: &NoGC, node: &Node) -> bool {
    node.traverse_preorder_unrooted(no_gc, ShadowIncluding::No)
        .skip(1)
        .any(|descendant| descendant.is_prohibited_paragraph_child())
}

/// The shared steps of the indeterminacy and value of the formatBlock command, which
/// return the formattable block name in effect for a node.
fn formattable_block_name_for_node(no_gc: &NoGC, node: &Node) -> String {
    let mut node = DomRoot::from_ref(node);
    // While node's parent is editable and in the same editing host as node,
    // and node is not an HTML element whose local name is a formattable block name,
    // set node to its parent.
    while !is_html_element_with_formattable_block_name(&node) &&
        let Some(parent) = node.GetParentNode() &&
        parent.is_editable() &&
        parent.same_editing_host(&node)
    {
        node = parent;
    }
    // If node is an editable HTML element whose local name is a formattable block name,
    // and node is not the ancestor of a prohibited paragraph child,
    // return node's local name, converted to ASCII lowercase.
    if node.is_editable() &&
        is_html_element_with_formattable_block_name(&node) &&
        !is_ancestor_of_prohibited_paragraph_child(no_gc, &node)
    {
        return node
            .downcast::<HTMLElement>()
            .expect("Must be an HTML element")
            .local_name()
            .to_ascii_lowercase();
    }
    // Return the empty string.
    String::new()
}

/// Returns all visible editable nodes that are contained in the block-extended
/// active range and have no children.
fn visible_editable_leaf_nodes_in_block_extended_active_range(
    cx: &mut JSContext,
    document: &Document,
) -> Option<Vec<DomRoot<Node>>> {
    // If the active range is null, return the empty string.
    let selection = document.GetSelection(cx)?;
    let active_range = selection.active_range(cx)?;
    // Block-extend the active range, and let new range be the result.
    let new_range = active_range.block_extend(cx, document);
    Some(
        new_range
            .contained_nodes(cx.no_gc())
            .filter(|node| {
                node.is_visible(cx.no_gc()) && node.is_editable() && node.children_count() == 0
            })
            .map(|node| node.as_rooted())
            .collect(),
    )
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-formatblock-command>
pub(crate) fn is_formatblock_indeterminate(cx: &mut JSContext, document: &Document) -> bool {
    // Step 1. If the active range is null, return the empty string.
    // Step 2. Block-extend the active range, and let new range be the result.
    // Step 3. Let node list be all visible editable nodes that are contained in new range and have no children.
    let Some(node_list) = visible_editable_leaf_nodes_in_block_extended_active_range(cx, document)
    else {
        return false;
    };
    // Step 4. If node list is empty, return false.
    // Step 5. Let type be null.
    let mut type_: Option<String> = None;
    // Step 6. For each node in node list:
    for node in node_list {
        // Step 6.1. While node's parent is editable and in the same editing host as node,
        // and node is not an HTML element whose local name is a formattable block name, set node to its parent.
        // Step 6.2. Let current type be the empty string.
        // Step 6.3. If node is an editable HTML element whose local name is a formattable block name,
        // and node is not the ancestor of a prohibited paragraph child, set current type to node's local name.
        let current_type = formattable_block_name_for_node(cx.no_gc(), &node);
        match type_ {
            // Step 6.4. If type is null, set type to current type.
            None => type_ = Some(current_type),
            // Step 6.5. Otherwise, if type does not equal current type, return true.
            Some(ref type_) if *type_ != current_type => return true,
            _ => {},
        }
    }
    // Step 7. Return false.
    false
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-formatblock-command>
pub(crate) fn value_for_formatblock_command(
    cx: &mut JSContext,
    document: &Document,
) -> Option<DOMString> {
    // Step 1. If the active range is null, return the empty string.
    // Step 2. Block-extend the active range, and let new range be the result.
    // Step 3. Let node be the first visible editable node that is contained in new range and has no children.
    // If there is no such node, return the empty string.
    let node_list = visible_editable_leaf_nodes_in_block_extended_active_range(cx, document)?;
    let node = node_list.first()?;
    // Step 4. While node's parent is editable and in the same editing host as node,
    // and node is not an HTML element whose local name is a formattable block name, set node to its parent.
    // Step 5. If node is an editable HTML element whose local name is a formattable block name,
    // and node is not the ancestor of a prohibited paragraph child, return node's local name,
    // converted to ASCII lowercase.
    // Step 6. Return the empty string.
    Some(formattable_block_name_for_node(cx.no_gc(), node).into())
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-formatblock-command>
pub(crate) fn execute_formatblock_command(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
    value: DOMString,
) -> bool {
    // Step 1. If value begins with a "<" character and ends with a ">" character,
    // remove the first and last characters from it.
    let value = value.str();
    let mut value: &str = &value;
    if value.len() >= 2 && value.starts_with('<') && value.ends_with('>') {
        value = &value[1..value.len() - 1];
    }

    // Step 2. Let value be converted to ASCII lowercase.
    let value = value.to_ascii_lowercase();

    // Step 3. If value is not a formattable block name, return false.
    if !FORMATTABLE_BLOCK_NAMES.contains(&value.as_str()) {
        return false;
    }

    // Step 4. Block-extend the active range, and let new range be the result.
    let active_range = selection
        .active_range(cx)
        .expect("Must always have an active range");
    let new_range = active_range.block_extend(cx, document);

    // Step 5. Let node list be an empty list of nodes.
    let mut node_list: Vec<DomRoot<Node>> = vec![];

    // Step 6. For each node node contained in new range, append node to node list if it is editable,
    // the last member of original node list (if any) is not an ancestor of node,
    // node is either a non-list single-line container or an allowed child of "p" or a dd or dt,
    // and node is not the ancestor of a prohibited paragraph child.
    for node in new_range.contained_nodes(cx.no_gc()) {
        if node.is_editable() &&
            node_list
                .last()
                .is_none_or(|last| !last.is_ancestor_of(&node)) &&
            (node
                .downcast::<Element>()
                .is_some_and(|element| element.is_non_list_single_line_container()) ||
                is_allowed_child(
                    NodeOrString::Node(node.clone()),
                    NodeOrString::String("p".to_owned()),
                ) ||
                node.downcast::<HTMLElement>()
                    .is_some_and(|element| matches!(element.local_name(), "dd" | "dt"))) &&
            !is_ancestor_of_prohibited_paragraph_child(cx.no_gc(), &node)
        {
            node_list.push(node.as_rooted());
        }
    }

    // Step 7. Record the values of node list, and let values be the result.
    let values = record_the_values(node_list.clone());

    // Step 8. For each node in node list, while node is the descendant of an editable HTML element
    // in the same editing host, whose local name is a formattable block name,
    // and which is not the ancestor of a prohibited paragraph child,
    // split the parent of the one-node list consisting of node.
    for node in node_list.iter() {
        while node.ancestors().any(|ancestor| {
            ancestor.is_editable() &&
                ancestor.same_editing_host(node) &&
                is_html_element_with_formattable_block_name(&ancestor) &&
                !is_ancestor_of_prohibited_paragraph_child(cx.no_gc(), &ancestor)
        }) {
            split_the_parent(cx, &[node]);
        }
    }

    // Step 9. Restore the values from values.
    restore_the_values(cx, values);

    // Step 10. While node list is not empty:
    let mut node_list = node_list.into_iter().peekable();
    while let Some(first) = node_list.next() {
        // Step 10.1. If the first member of node list is a single-line container:
        let sublist = if first.is_single_line_container() {
            // Step 10.1.1. Let sublist be the children of the first member of node list.
            let sublist: Vec<DomRoot<Node>> = first.children().collect();

            // Step 10.1.2. Record the values of sublist, and let values be the result.
            let values = record_the_values(sublist.clone());

            // Step 10.1.3. Remove the first member of node list from its parent, preserving its descendants.
            first.remove_preserving_its_descendants(cx);

            // Step 10.1.4. Restore the values from values.
            restore_the_values(cx, values);

            // Step 10.1.5. Remove the first member from node list.
            sublist
        } else {
            // Step 10.2. Otherwise:
            // Step 10.2.1. Let sublist be an empty list of nodes.
            // Step 10.2.2. Remove the first member of node list and append it to sublist.
            let mut sublist = vec![first];

            // Step 10.2.3. While node list is not empty, and the first member of node list is the
            // nextSibling of the last member of sublist, and the first member of node list is not
            // a single-line container, and the last member of sublist is not a br,
            // remove the first member of node list and append it to sublist.
            while let Some(next) = node_list.next_if(|next| {
                let last = sublist.last().expect("Must always have a last member");
                last.GetNextSibling().as_ref() == Some(next) &&
                    !next.is_single_line_container() &&
                    !last.is::<HTMLBRElement>()
            }) {
                sublist.push(next);
            }
            sublist
        };

        // Step 10.3. Wrap sublist. If value is "div" or "p", sibling criteria returns false;
        // otherwise it returns true for an HTML element with local name value and no attributes,
        // and false otherwise. New parent instructions return the result of running
        // createElement(value) on the context object. Then fix disallowed ancestors of the result.
        let new_parent = wrap_node_list(
            cx,
            sublist,
            |sibling| {
                if value == "div" || value == "p" {
                    return false;
                }
                sibling.downcast::<HTMLElement>().is_some_and(|element| {
                    element.local_name() == value &&
                        element.upcast::<Element>().attrs().borrow().is_empty()
                })
            },
            |cx| Some(DomRoot::upcast(document.create_element(cx, &value))),
        );
        if let Some(new_parent) = new_parent {
            new_parent.fix_disallowed_ancestors(cx, document);
        }
    }

    // Step 11. Return true.
    true
}
