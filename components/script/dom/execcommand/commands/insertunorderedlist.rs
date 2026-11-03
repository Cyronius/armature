/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::JSContext;

use crate::dom::document::Document;
use crate::dom::selection::Selection;

/// <https://w3c.github.io/editing/docs/execCommand/#the-insertunorderedlist-command>
pub(crate) fn execute_insert_unordered_list_command(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
) -> bool {
    // > Toggle lists with tag name "ul", then return true.
    selection.toggle_lists(cx, document, "ul");
    true
}
