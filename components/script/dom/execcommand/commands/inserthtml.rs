/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::JSContext;
use script_bindings::codegen::GenericBindings::NodeBinding::NodeMethods;
use script_bindings::codegen::GenericBindings::RangeBinding::RangeMethods;
use script_bindings::codegen::GenericBindings::SelectionBinding::SelectionMethods;
use script_bindings::inheritance::Castable;

use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::document::Document;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use crate::dom::selection::Selection;

/// <https://w3c.github.io/editing/docs/execCommand/#the-inserthtml-command>
pub(crate) fn execute_insert_html_command(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
    value: DOMString,
) -> bool {
    // Step 1. Set value to the result of invoking get trusted types compliant string with TrustedHTML,
    // this's relevant global object, value, "Document execCommand", and "script".
    //
    // Note: This is done by `Document::ExecCommand` before the command is executed.

    // Step 2. Delete the selection.
    selection.delete_the_selection(
        cx,
        document,
        Default::default(),
        Default::default(),
        Default::default(),
    );

    let active_range = selection
        .active_range(cx)
        .expect("Must always have an active range");

    // Step 3. If the active range's start node is neither editable nor an editing host, return true.
    if !active_range.start_container().is_editable_or_editing_host() {
        return true;
    }

    // Step 4. Let frag be the result of calling createContextualFragment(value) on the active range.
    let Ok(frag) = active_range.create_contextual_fragment(cx, value) else {
        return false;
    };
    let frag = DomRoot::upcast::<Node>(frag);

    // Step 5. Let last child be the lastChild of frag.
    // Step 6. If last child is null, return true.
    let Some(last_child) = frag.GetLastChild() else {
        return true;
    };

    // Step 7. Let descendants be all descendants of frag.
    let descendants: Vec<DomRoot<Node>> = frag
        .traverse_preorder(ShadowIncluding::No)
        .skip(1)
        .collect();

    // Step 8. If the active range's start node is a block node:
    let start_node = active_range.start_container();
    if start_node.is_block_node() {
        // Step 8.1. Let collapsed block props be all editable collapsed block prop children of the
        // active range's start node that have index greater than or equal to the active range's start offset.
        let start_offset = active_range.start_offset();
        let collapsed_block_props: Vec<DomRoot<Node>> = start_node
            .children()
            .filter(|child| {
                child.is_editable() &&
                    child.is_collapsed_block_prop(cx.no_gc()) &&
                    child.index() >= start_offset
            })
            .collect();

        // Step 8.2. For each node in collapsed block props, remove node from its parent.
        for node in collapsed_block_props {
            node.remove_self(cx);
        }
    }

    // Step 9. Call insertNode(frag) on the active range.
    if active_range.InsertNode(cx, &frag).is_err() {
        return false;
    }

    // Step 10. If the active range's start node is a block node with no visible children,
    // call createElement("br") on the context object and append the result as the last child
    // of the active range's start node.
    let start_node = active_range.start_container();
    if start_node.is_block_node() &&
        !start_node
            .children()
            .any(|child| child.is_visible(cx.no_gc()))
    {
        let br = document.create_element(cx, "br");
        if start_node.AppendChild(cx, br.upcast()).is_err() {
            unreachable!("Must always be able to append");
        }
    }

    // Step 11. Call collapse() on the context object's selection, with last child's parent as
    // the first argument and one plus its index as the second.
    if selection
        .Collapse(
            cx,
            last_child.GetParentNode().as_deref(),
            1 + last_child.index(),
        )
        .is_err()
    {
        unreachable!("Should always be able to collapse the selection.");
    }

    // Step 12. Fix disallowed ancestors of each member of descendants.
    for descendant in descendants {
        descendant.fix_disallowed_ancestors(cx, document);
    }

    // Step 13. Return true.
    true
}
