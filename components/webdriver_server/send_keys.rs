/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The events for *Element Send Keys*.
//!
//! This follows `keyboard_types::webdriver::send_keys` (keyboard-types 0.8.3, MIT or Apache-2.0),
//! which panics when it releases Shift to type an unshifted character after a Shift modifier
//! (for example `"\u{E008}y"`), then tries to release that Shift again at the end.
//! FIXME: Use the crate's function once it no longer panics.

use std::collections::HashSet;

use keyboard_types::webdriver::{Event, KeyInputState};
use keyboard_types::{CompositionEvent, CompositionState, Key, Modifiers, NamedKey};
use unicode_segmentation::UnicodeSegmentation;

/// <https://w3c.github.io/webdriver/#dfn-dispatch-actions-for-a-string>
pub(crate) fn send_keys(text: &str) -> Vec<Event> {
    let mut result = Vec::new();
    let mut typeable_text = String::new();
    let mut state = KeyInputState::new();
    let mut undo_actions = HashSet::new();
    for cluster in text.graphemes(true) {
        match cluster {
            "\u{E000}" => {
                dispatch_typeable(&mut state, &mut typeable_text, &mut result);
                clear(&mut state, &mut undo_actions, &mut result);
            },
            cluster if is_modifier(cluster) => {
                dispatch_typeable(&mut state, &mut typeable_text, &mut result);
                let raw_modifier = first_char(cluster);
                result.push(state.dispatch_keydown(raw_modifier).into());
                undo_actions.insert(raw_modifier);
            },
            // <https://w3c.github.io/webdriver/#dfn-typeable>
            cluster if cluster.chars().count() == 1 => typeable_text.push_str(cluster),
            cluster => {
                dispatch_typeable(&mut state, &mut typeable_text, &mut result);
                // FIXME: Spec says undefined instead of empty string
                for (composition_state, data) in [
                    (CompositionState::Start, String::new()),
                    (CompositionState::Update, cluster.to_owned()),
                    (CompositionState::End, cluster.to_owned()),
                ] {
                    result.push(
                        CompositionEvent {
                            state: composition_state,
                            data,
                        }
                        .into(),
                    );
                }
            },
        }
    }
    dispatch_typeable(&mut state, &mut typeable_text, &mut result);
    clear(&mut state, &mut undo_actions, &mut result);
    result
}

fn first_char(text: &str) -> char {
    text.chars().next().unwrap_or_default()
}

/// Whether `text` is a single modifier key, from
/// <https://www.w3.org/TR/uievents-key/#keys-modifier>.
#[expect(deprecated)]
fn is_modifier(text: &str) -> bool {
    if text.chars().count() != 1 {
        return false;
    }
    // A scratch state, so that the key is normalised the way `dispatch_keydown` does it.
    let key = KeyInputState::new().dispatch_keydown(first_char(text)).key;
    matches!(
        key,
        Key::Named(
            NamedKey::Alt |
                NamedKey::AltGraph |
                NamedKey::CapsLock |
                NamedKey::Control |
                NamedKey::Fn |
                NamedKey::FnLock |
                NamedKey::Meta |
                NamedKey::NumLock |
                NamedKey::ScrollLock |
                NamedKey::Shift |
                NamedKey::Symbol |
                NamedKey::SymbolLock |
                NamedKey::Hyper |
                NamedKey::Super
        )
    )
}

/// Release the modifier keys pressed so far. A key that is no longer pressed, such as a Shift
/// released to type an unshifted character, produces no event.
fn clear(state: &mut KeyInputState, undo_actions: &mut HashSet<char>, result: &mut Vec<Event>) {
    let mut actions: Vec<_> = undo_actions.drain().collect();
    actions.sort_unstable();
    for action in actions {
        if let Some(event) = state.dispatch_keyup(action) {
            result.push(event.into());
        }
    }
}

fn dispatch_typeable(state: &mut KeyInputState, text: &mut String, result: &mut Vec<Event>) {
    for character in text.chars() {
        // The state's modifiers are private; a key event from a copy of it reports them.
        let shifted = state
            .clone()
            .dispatch_keydown('a')
            .modifiers
            .contains(Modifiers::SHIFT);
        if is_shifted_character(character) && !shifted {
            // Press the left Shift key.
            result.push(state.dispatch_keydown('\u{E008}').into());
        }
        if !is_shifted_character(character) && shifted {
            // Release the left Shift key.
            if let Some(event) = state.dispatch_keyup('\u{E008}') {
                result.push(event.into());
            }
        }
        result.push(state.dispatch_keydown(character).into());
        if let Some(event) = state.dispatch_keyup(character) {
            result.push(event.into());
        }
    }
    text.clear();
}

fn is_shifted_character(raw_key: char) -> bool {
    matches!(
        raw_key,
        '~' | '|' |
            '{' |
            '}' |
            '<' |
            ')' |
            '!' |
            '@' |
            '#' |
            '$' |
            '%' |
            '^' |
            '&' |
            '*' |
            '(' |
            '+' |
            '>' |
            '_' |
            '\"' |
            ':' |
            '?' |
            '\u{E00D}' |
            '\u{E05C}' |
            '\u{E056}' |
            '\u{E05B}' |
            '\u{E055}' |
            '\u{E058}' |
            '\u{E05A}' |
            '\u{E057}' |
            '\u{E059}' |
            '\u{E054}' |
            '\u{E05D}' |
            'A'..='Z'
    )
}
