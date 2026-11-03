/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::JSContext;
use script_bindings::codegen::GenericBindings::NodeBinding::NodeMethods;
use script_bindings::inheritance::Castable;

use crate::dom::bindings::root::{DomRoot, DomSlice};
use crate::dom::document::Document;
use crate::dom::execcommand::commands::indent::normalize_sublists;
use crate::dom::execcommand::contenteditable::node::{
    record_the_values, restore_the_values, split_the_parent,
};
use crate::dom::html::htmllielement::HTMLLIElement;
use crate::dom::html::htmlolistelement::HTMLOListElement;
use crate::dom::html::htmlulistelement::HTMLUListElement;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use crate::dom::selection::Selection;

fn is_list(node: &Node) -> bool {
    node.is::<HTMLOListElement>() || node.is::<HTMLUListElement>()
}

fn is_child_of_list(node: &Node) -> bool {
    node.GetParentNode().is_some_and(|parent| is_list(&parent))
}

/// <https://w3c.github.io/editing/docs/execCommand/#the-outdent-command>
pub(crate) fn execute_outdent_command(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
) -> bool {
    let active_range = selection
        .active_range(cx)
        .expect("Must always have an active range.");
    // Step 1. Let items be a list of all lis that are inclusive ancestors of the active range's start and/or end node.
    let mut items: Vec<DomRoot<Node>> = vec![];
    for item in active_range
        .start_container()
        .inclusive_ancestors(ShadowIncluding::No)
        .chain(
            active_range
                .end_container()
                .inclusive_ancestors(ShadowIncluding::No),
        )
        .filter(|ancestor| ancestor.is::<HTMLLIElement>())
    {
        if !items.contains(&item) {
            items.push(item);
        }
    }

    // Step 2. For each item in items, normalize sublists of item.
    for item in items {
        normalize_sublists(cx, item);
    }

    // Step 3. Block-extend the active range, and let new range be the result.
    let new_range = selection
        .active_range(cx)
        .expect("Must always have an active range.")
        .block_extend(cx, document);

    // Step 4. Let node list be a list of nodes, initially empty.
    let mut node_list: Vec<DomRoot<Node>> = vec![];

    // Step 5. For each node node contained in new range,
    //         append node to node list if the last member of node list (if any) is not an ancestor of node;
    //         node is editable; and either node has no editable descendants, or is an ol or ul,
    //         or is an li whose parent is an ol or ul.
    for node in new_range.contained_nodes(cx.no_gc()) {
        if node_list
            .last()
            .is_none_or(|last| !last.is_ancestor_of(&node)) &&
            node.is_editable() &&
            (!node
                .traverse_preorder(ShadowIncluding::No)
                .skip(1)
                .any(|descendant| descendant.is_editable()) ||
                is_list(&node) ||
                (node.is::<HTMLLIElement>() && is_child_of_list(&node)))
        {
            node_list.push(node.as_rooted());
        }
    }

    // Step 6. While node list is not empty:
    let mut node_list = node_list.into_iter().peekable();
    while node_list.peek().is_some() {
        // Step 6.1. While the first member of node list is an ol or ul or is not the child of an ol or ul,
        //           outdent it and remove it from node list.
        while let Some(first) = node_list.next_if(|node| is_list(node) || !is_child_of_list(node)) {
            first.outdent(cx, document);
        }

        // Step 6.2. If node list is empty, break from these substeps.
        let Some(first) = node_list.next() else {
            break;
        };

        // Step 6.3. Let sublist be a list of nodes, initially empty.
        // Step 6.4. Remove the first member of node list and append it to sublist.
        let mut sublist: Vec<DomRoot<Node>> = vec![first];

        // Step 6.5. While the first member of node list is the nextSibling of the last member of sublist,
        //           and the first member of node list is not an ol or ul,
        //           remove the first member of node list and append it to sublist.
        while let Some(next) = node_list.next_if(|node| {
            !is_list(node) &&
                sublist
                    .last()
                    .expect("Must always have last element here.")
                    .GetNextSibling()
                    .is_some_and(|next_sibling| next_sibling == *node)
        }) {
            sublist.push(next);
        }

        // Step 6.6. Record the values of sublist, and let values be the result.
        let values = record_the_values(sublist.clone());

        // Step 6.7. Split the parent of sublist.
        rooted_vec!(let sublist_refs <- sublist.iter().map(|node| DomRoot::as_traced(node)));
        split_the_parent(cx, sublist_refs.r());

        // Step 6.8. Fix disallowed ancestors of each member of sublist.
        for node in &sublist {
            node.fix_disallowed_ancestors(cx, document);
        }

        // Step 6.9. Restore the values from values.
        restore_the_values(cx, values);
    }

    // Step 7. Return true.
    true
}
