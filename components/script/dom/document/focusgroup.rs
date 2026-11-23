/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Directional focus navigation inside focusgroups.
//!
//! <https://open-ui.org/components/scoped-focusgroup.explainer/>

use html5ever::LocalName;
use js::context::JSContext;
use js::context::NoGC;
use keyboard_types::NamedKey;

use crate::dom::bindings::codegen::Bindings::CSSStyleDeclarationBinding::CSSStyleDeclarationMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::WindowMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::node::focus::FocusTrigger;
use crate::dom::element::Element;
use crate::dom::iterators::ShadowIncluding;
use crate::dom::node::{Node, NodeTraits};
use crate::dom::bindings::codegen::Bindings::HTMLInputElementBinding::HTMLInputElementMethods;
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
/// where a nested focusgroup is itself an item. Each comes with the segment it is in, which is
/// a run of items that nothing that is not part of the focusgroup interrupts.
fn items_with_segments(no_gc: &NoGC, owner: &Element) -> Vec<(DomRoot<Element>, usize)> {
    let mut items = Vec::new();
    let mut segment = 0;
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
            segment += 1;
        }
        let Some(element) = node.downcast::<Element>() else {
            continue;
        };
        if opts_out(element) {
            skip_below = Some(node.clone());
            continue;
        }
        let nested_owner = is_owner(element);
        if element.is_focusable_area(no_gc) {
            items.push((DomRoot::from_ref(element), segment));
        }
        if nested_owner {
            skip_below = Some(node.clone());
        }
    }
    items
}

fn items(no_gc: &NoGC, owner: &Element) -> Vec<DomRoot<Element>> {
    items_with_segments(no_gc, owner)
        .into_iter()
        .map(|(item, _)| item)
        .collect()
}

/// Whether the arrow keys are used by `element` itself, so that they do not move the focus
/// between the items of a focusgroup.
fn uses_arrow_keys(element: &Element) -> bool {
    use crate::dom::html::htmlinputelement::HTMLInputElement;
    if element.is::<crate::dom::html::htmltextareaelement::HTMLTextAreaElement>() ||
        element.is::<crate::dom::html::htmlselectelement::HTMLSelectElement>()
    {
        return true;
    }
    if let Some(input) = element.downcast::<HTMLInputElement>() {
        return !matches!(
            input.Type().str().to_ascii_lowercase().as_str(),
            "button" | "submit" | "reset" | "image" | "checkbox" | "file" | "color" | "hidden"
        );
    }
    element
        .get_attribute_string_value(&LocalName::from("contenteditable"))
        .is_some_and(|value| value.is_empty() || !value.eq_ignore_ascii_case("false"))
}

/// Whether sequential focus navigation skips `element`: of the items of each segment of a
/// focusgroup only the one that the focus enters at can be reached with the Tab key.
pub(crate) fn is_skipped_by_sequential_navigation(no_gc: &NoGC, element: &Element) -> bool {
    let Some(owner) = element
        .upcast::<Node>()
        .ancestors()
        .filter_map(DomRoot::downcast::<Element>)
        .find(|ancestor| is_owner(ancestor))
    else {
        return false;
    };
    let all_items = items_with_segments(no_gc, &owner);
    let Some(segment) = all_items
        .iter()
        .find(|(item, _)| **item == *element)
        .map(|(_, segment)| *segment)
    else {
        return false;
    };
    let in_segment: Vec<&DomRoot<Element>> = all_items
        .iter()
        .filter(|(_, item_segment)| *item_segment == segment)
        .map(|(item, _)| item)
        .collect();

    // The arrow keys cannot leave an item that uses them itself, so it and the items next to it
    // can be reached with the Tab key too.
    if let Some(position) = in_segment.iter().position(|item| ***item == *element) {
        let uses = |index: usize| in_segment.get(index).is_some_and(|item| uses_arrow_keys(item));
        if uses(position) ||
            position.checked_sub(1).is_some_and(uses) ||
            uses(position + 1)
        {
            return false;
        }
    }

    let has_memory = !tokens(&owner).is_some_and(|tokens| tokens.iter().any(|token| token == "nomemory"));
    let document = owner.owner_document();
    let remembered = has_memory
        .then(|| document.focusgroup_memory(&owner))
        .flatten()
        .filter(|remembered| in_segment.iter().any(|item| ***item == **remembered));
    let entry = remembered
        .or_else(|| {
            in_segment
                .iter()
                .find(|item| item.has_attribute(&LocalName::from("focusgroupstart")))
                .map(|item| DomRoot::clone(item))
        })
        .or_else(|| in_segment.first().map(|item| DomRoot::clone(item)));
    entry.is_some_and(|entry| *entry != *element)
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
    if tokens.iter().any(|token| token == "grid") {
        return handle_grid_key(cx, &owner, &tokens, focused, key);
    }
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

    let items = items(cx.no_gc(), &owner);
    let Some(index) = items.iter().position(|item| **item == *focused) else {
        return false;
    };

    if uses_arrow_keys(focused) {
        return false;
    }
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
        owner.owner_document().set_focusgroup_memory(&owner, target);
        target
            .upcast::<Node>()
            .run_the_focusing_steps(cx, None, FocusTrigger::Keyboard);
    }
    // Directional input is used up inside a focusgroup, whether or not the focus moved.
    true
}

/// A cell of a grid.
struct GridCell {
    element: DomRoot<Element>,
    row: usize,
    column: usize,
}

/// Lays out the cells of a table in a grid: the cells with their positions, and which cell covers
/// each position, taking row and column spans into account.
fn table_grid(table: &Element) -> (Vec<GridCell>, Vec<Vec<Option<usize>>>) {
    let span = |cell: &Element, name: &str| -> usize {
        cell.get_attribute_string_value(&LocalName::from(name))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1)
            .min(1000)
    };
    let is_html = |element: &Element, name: &str| {
        element.is_html_element() && &**element.local_name() == name
    };

    // The rows: the `tr` children of the table and of its row groups.
    let mut rows: Vec<DomRoot<Element>> = Vec::new();
    for child in table.upcast::<Node>().children() {
        let Some(child) = child.downcast::<Element>() else {
            continue;
        };
        if is_html(child, "tr") {
            rows.push(DomRoot::from_ref(child));
        } else if is_html(child, "thead") || is_html(child, "tbody") || is_html(child, "tfoot") {
            for row in child.upcast::<Node>().children() {
                if let Some(row) = row.downcast::<Element>() &&
                    is_html(row, "tr")
                {
                    rows.push(DomRoot::from_ref(row));
                }
            }
        }
    }

    let mut cells: Vec<GridCell> = Vec::new();
    let mut occupancy: Vec<Vec<Option<usize>>> = Vec::new();
    let occupy = |occupancy: &mut Vec<Vec<Option<usize>>>, row: usize, column: usize, index| {
        while occupancy.len() <= row {
            occupancy.push(Vec::new());
        }
        let line = &mut occupancy[row];
        while line.len() <= column {
            line.push(None);
        }
        line[column] = Some(index);
    };
    for (row_index, row) in rows.iter().enumerate() {
        if occupancy.len() <= row_index {
            occupancy.push(Vec::new());
        }
        let mut column = 0;
        for cell in row.upcast::<Node>().children() {
            let Some(cell) = cell.downcast::<Element>() else {
                continue;
            };
            if !is_html(cell, "td") && !is_html(cell, "th") {
                continue;
            }
            // Skip the columns that are covered from rows above.
            while occupancy[row_index].get(column).is_some_and(Option::is_some) {
                column += 1;
            }
            let index = cells.len();
            cells.push(GridCell {
                element: DomRoot::from_ref(cell),
                row: row_index,
                column,
            });
            for row_offset in 0..span(cell, "rowspan") {
                for column_offset in 0..span(cell, "colspan") {
                    occupy(&mut occupancy, row_index + row_offset, column + column_offset, index);
                }
            }
            column += span(cell, "colspan");
        }
    }
    let width = occupancy.iter().map(Vec::len).max().unwrap_or(0);
    for line in occupancy.iter_mut() {
        line.resize(width, None);
    }
    (cells, occupancy)
}

/// Moves the focus in a grid, which is a table, with a directional key.
fn handle_grid_key(
    cx: &mut JSContext,
    table: &Element,
    tokens: &[String],
    focused: &Element,
    key: &NamedKey,
) -> bool {
    let has = |token: &str| tokens.iter().any(|candidate| candidate == token);
    let row_wrap = has("wrap") || has("rowwrap");
    let column_wrap = has("wrap") || has("colwrap");
    let row_flow = has("flow") || has("rowflow");
    let column_flow = has("flow") || has("colflow");

    if !table.local_name().eq_str_ignore_ascii_case("table") {
        return false;
    }
    let (cells, occupancy) = table_grid(table);
    let rows = occupancy.len() as isize;
    let columns = occupancy.first().map_or(0, Vec::len) as isize;
    let Some(current) = cells.iter().position(|cell| *cell.element == *focused) else {
        return false;
    };
    let (mut row, mut column) = (cells[current].row as isize, cells[current].column as isize);

    let target = match key {
        NamedKey::Home | NamedKey::End => return false,
        _ => {
            let (row_step, column_step) = match key {
                NamedKey::ArrowRight => (0, 1),
                NamedKey::ArrowLeft => (0, -1),
                NamedKey::ArrowDown => (1, 0),
                NamedKey::ArrowUp => (-1, 0),
                _ => return false,
            };
            let mut found = None;
            for _ in 0..(rows * columns * 2 + 4) {
                row += row_step;
                column += column_step;
                // Leaving the grid wraps, flows to the next line or ends the movement.
                if column >= columns {
                    if row_flow {
                        column = 0;
                        row += 1;
                        if row >= rows {
                            row = 0;
                        }
                    } else if row_wrap {
                        column = 0;
                    } else {
                        break;
                    }
                } else if column < 0 {
                    if row_flow {
                        column = columns - 1;
                        row -= 1;
                        if row < 0 {
                            row = rows - 1;
                        }
                    } else if row_wrap {
                        column = columns - 1;
                    } else {
                        break;
                    }
                }
                if row >= rows {
                    if column_flow {
                        row = 0;
                        column += 1;
                        if column >= columns {
                            column = 0;
                        }
                    } else if column_wrap {
                        row = 0;
                    } else {
                        break;
                    }
                } else if row < 0 {
                    if column_flow {
                        row = rows - 1;
                        column -= 1;
                        if column < 0 {
                            column = columns - 1;
                        }
                    } else if column_wrap {
                        row = rows - 1;
                    } else {
                        break;
                    }
                }
                let Some(index) = occupancy[row as usize][column as usize] else {
                    continue;
                };
                if index == current {
                    continue;
                }
                if cells[index].element.is_focusable_area(cx.no_gc()) {
                    found = Some(index);
                    break;
                }
            }
            found
        },
    };
    if let Some(index) = target {
        cells[index]
            .element
            .upcast::<Node>()
            .run_the_focusing_steps(cx, None, FocusTrigger::Keyboard);
    }
    true
}
