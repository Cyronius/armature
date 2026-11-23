/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Directional focus navigation inside focusgroups.
//!
//! <https://open-ui.org/components/scoped-focusgroup.explainer/>

use html5ever::LocalName;
use js::context::JSContext;
use keyboard_types::NamedKey;

use crate::dom::bindings::codegen::Bindings::CSSStyleDeclarationBinding::CSSStyleDeclarationMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::WindowMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::node::focus::FocusTrigger;
use crate::dom::element::Element;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::Node;
use crate::dom::window::Window;

const BEHAVIOR_TOKENS: [&str; 7] = [
    "toolbar",
    "tablist",
    "radiogroup",
    "listbox",
    "menu",
    "menubar",
    "grid",
];

/// The tokens of the `focusgroup` attribute of `element`, in lowercase.
fn tokens(element: &Element) -> Option<Vec<String>> {
    let value = element.get_attribute_string_value(&LocalName::from("focusgroup"))?;
    Some(
        value
            .split_ascii_whitespace()
            .map(|token| token.to_ascii_lowercase())
            .collect(),
    )
}

/// Whether `element` is the owner of a focusgroup.
fn is_owner(element: &Element) -> bool {
    tokens(element).is_some_and(|tokens| {
        !tokens.iter().any(|token| token == "none") &&
            tokens
                .iter()
                .any(|token| BEHAVIOR_TOKENS.contains(&token.as_str()))
    })
}

fn opts_out(element: &Element) -> bool {
    tokens(element).is_some_and(|tokens| tokens.iter().any(|token| token == "none"))
}

/// The items of a focusgroup: the focusable elements in it that are not in a nested focusgroup,
/// where a nested focusgroup is itself an item.
fn items(cx: &mut JSContext, owner: &Element) -> Vec<DomRoot<Element>> {
    let mut items = Vec::new();
    let mut skip_below: Option<DomRoot<Node>> = None;
    for node in owner
        .upcast::<Node>()
        .traverse_preorder(ShadowIncluding::Yes)
        .skip(1)
    {
        if let Some(root) = skip_below.as_ref() {
            if root.is_ancestor_of(&node) {
                continue;
            }
            skip_below = None;
        }
        let Some(element) = node.downcast::<Element>() else {
            continue;
        };
        if opts_out(element) {
            skip_below = Some(node.clone());
            continue;
        }
        let nested_owner = is_owner(element);
        if element.is_focusable_area(cx.no_gc()) {
            items.push(DomRoot::from_ref(element));
        }
        if nested_owner {
            skip_below = Some(node.clone());
        }
    }
    items
}

/// Moves the focus from `focused` according to a directional key, if `focused` is an item of a
/// focusgroup. Returns whether the key was used.
pub(crate) fn handle_directional_key(
    cx: &mut JSContext,
    window: &Window,
    focused: &Element,
    key: &NamedKey,
) -> bool {
    // The owner is the nearest ancestor that owns a focusgroup.
    let Some(owner) = focused
        .upcast::<Node>()
        .ancestors()
        .filter_map(DomRoot::downcast::<Element>)
        .find(|ancestor| is_owner(ancestor))
    else {
        return false;
    };
    let tokens = tokens(&owner).unwrap_or_default();
    // What the behavior token implies, which the axis and wrap tokens override.
    let behavior = tokens
        .iter()
        .find(|token| BEHAVIOR_TOKENS.contains(&token.as_str()))
        .map(String::as_str)
        .unwrap_or("toolbar");
    let (default_inline, default_block, default_wrap) = match behavior {
        "tablist" | "menubar" => (true, false, true),
        "menu" => (false, true, true),
        "listbox" => (false, true, false),
        "radiogroup" => (true, true, true),
        _ => (true, false, false),
    };
    let explicit_inline = tokens.iter().any(|token| token == "inline");
    let explicit_block = tokens.iter().any(|token| token == "block");
    let (has_inline, has_block) = if explicit_inline || explicit_block {
        (explicit_inline, explicit_block)
    } else {
        (default_inline, default_block)
    };
    let wraps = if tokens.iter().any(|token| token == "nowrap") {
        false
    } else {
        default_wrap || tokens.iter().any(|token| token == "wrap")
    };

    let items = items(cx, &owner);
    let Some(index) = items.iter().position(|item| **item == *focused) else {
        return false;
    };

    let target = match key {
        NamedKey::Home => items.first(),
        NamedKey::End => items.last(),
        _ => {
            // Which way does the key go along which axis?
            let style = window.GetComputedStyle(cx, &owner, None);
            let direction = style.GetPropertyValue(DOMString::from("direction"));
            let writing_mode = style.GetPropertyValue(DOMString::from("writing-mode"));
            let rtl = *direction.str() == *"rtl";
            let writing_mode = writing_mode.str().to_string();
            let vertical = writing_mode.starts_with("vertical") || writing_mode.starts_with("sideways");
            let vertical_rl = writing_mode == "vertical-rl" || writing_mode == "sideways-rl";

            // The physical axis and direction of the key.
            let (horizontal_key, positive) = match key {
                NamedKey::ArrowRight => (true, true),
                NamedKey::ArrowLeft => (true, false),
                NamedKey::ArrowDown => (false, true),
                NamedKey::ArrowUp => (false, false),
                _ => return false,
            };
            // Is that the inline axis of the group, and is it going forward in it?
            let (is_inline, forward) = if vertical {
                if horizontal_key {
                    // Block axis, which goes from right to left in vertical-rl.
                    (false, positive != vertical_rl)
                } else {
                    (true, positive)
                }
            } else if horizontal_key {
                (true, positive != rtl)
            } else {
                (false, positive)
            };
            let axis_enabled = if is_inline { has_inline } else { has_block };
            if !axis_enabled {
                return false;
            }
            if forward {
                items
                    .get(index + 1)
                    .or_else(|| wraps.then(|| items.first()).flatten())
            } else if index > 0 {
                items.get(index - 1)
            } else if wraps {
                items.last()
            } else {
                None
            }
        },
    };

    if let Some(target) = target &&
        **target != *focused
    {
        target
            .upcast::<Node>()
            .run_the_focusing_steps(cx, None, FocusTrigger::Keyboard);
    }
    // Directional input is used up inside a focusgroup, whether or not the focus moved.
    true
}
