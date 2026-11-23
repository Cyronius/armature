/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Cross-origin opener policy: a popup whose policy does not match the one of its opener loses
//! its opener, and the opener loses the popup.
//!
//! <https://html.spec.whatwg.org/multipage/#cross-origin-opener-policies>
//!
//! The policies of the documents of top-level browsing contexts are kept in a process-wide table,
//! which only works when the script threads share a process, and a popup that is cut off from its
//! opener stays in the same group of browsing contexts for the constellation, which just stops
//! showing it to the opener.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};

use http::HeaderMap;
use servo_base::id::BrowsingContextId;
use servo_url::ImmutableOrigin;

use crate::dom::windowproxy::WindowProxy;

/// <https://html.spec.whatwg.org/multipage/#cross-origin-opener-policy-value>
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum CoopValue {
    #[default]
    UnsafeNone,
    SameOrigin,
    SameOriginAllowPopups,
}

impl CoopValue {
    /// <https://html.spec.whatwg.org/multipage/#obtain-coop>
    pub(crate) fn from_headers(headers: Option<&HeaderMap>) -> CoopValue {
        let Some(value) = headers
            .and_then(|headers| headers.get("Cross-Origin-Opener-Policy"))
            .and_then(|value| value.to_str().ok())
        else {
            return CoopValue::UnsafeNone;
        };
        // The header is a structured field token with parameters.
        match value.split(';').next().unwrap_or_default().trim() {
            "same-origin" => CoopValue::SameOrigin,
            "same-origin-allow-popups" => CoopValue::SameOriginAllowPopups,
            _ => CoopValue::UnsafeNone,
        }
    }
}

type PolicyTable = HashMap<BrowsingContextId, (CoopValue, ImmutableOrigin)>;

/// The policy and origin of the active document of each top-level browsing context.
static POLICIES: LazyLock<Mutex<PolicyTable>> = LazyLock::new(Default::default);

/// The popups that lost their opener.
static SEVERED: LazyLock<Mutex<HashSet<BrowsingContextId>>> = LazyLock::new(Default::default);

/// Whether a popup that loaded a document with `new_value` and `new_origin` loses the opener
/// whose active document has `opener_value` and `opener_origin`.
fn should_sever(
    opener_value: CoopValue,
    opener_origin: Option<&ImmutableOrigin>,
    new_value: CoopValue,
    new_origin: &ImmutableOrigin,
) -> bool {
    use CoopValue::*;
    let same_origin = opener_origin.is_some_and(|origin| origin == new_origin);
    let keeps_opener = match (opener_value, new_value) {
        (UnsafeNone, UnsafeNone) => true,
        (SameOrigin, SameOrigin) | (SameOriginAllowPopups, SameOriginAllowPopups) => same_origin,
        (SameOriginAllowPopups, UnsafeNone) => true,
        _ => false,
    };
    !keeps_opener
}

/// A document was created from a response in `window_proxy`.
pub(crate) fn document_created(
    window_proxy: &WindowProxy,
    value: CoopValue,
    origin: ImmutableOrigin,
) {
    let browsing_context_id = window_proxy.browsing_context_id();
    // The policy of a frame is the one of its top-level document, which a popup that the frame
    // opens has to match.
    if window_proxy.parent().is_some() {
        let top_policy = POLICIES
            .lock()
            .unwrap()
            .get(&window_proxy.top().browsing_context_id())
            .cloned();
        if let Some(policy) = top_policy {
            POLICIES.lock().unwrap().insert(browsing_context_id, policy);
        }
        return;
    }
    if let Some(opener_id) = window_proxy.opener_browsing_context_id() &&
        !window_proxy.is_disowned()
    {
        let opener_policy = POLICIES.lock().unwrap().get(&opener_id).cloned();
        let (opener_value, opener_origin) = match opener_policy {
            Some((value, origin)) => (value, Some(origin)),
            None => (CoopValue::UnsafeNone, None),
        };
        if should_sever(opener_value, opener_origin.as_ref(), value, &origin) {
            window_proxy.disown();
            // The popup starts over in a group of browsing contexts of its own.
            window_proxy.set_name(Default::default());
            SEVERED.lock().unwrap().insert(browsing_context_id);
        }
    }
    POLICIES
        .lock()
        .unwrap()
        .insert(browsing_context_id, (value, origin));
}

/// Whether the opener of the popup `browsing_context_id` lost it.
pub(crate) fn is_severed(browsing_context_id: BrowsingContextId) -> bool {
    SEVERED.lock().unwrap().contains(&browsing_context_id)
}

/// Whether a popup that `opener` opens must not have an opener: the top-level document of
/// `opener` has the `same-origin` policy and `opener` is a frame of another origin.
/// <https://html.spec.whatwg.org/multipage/#browsing-context-group-switch-due-to-cross-origin-opener-policy>
pub(crate) fn forces_noopener(opener: &WindowProxy, opener_origin: &ImmutableOrigin) -> bool {
    if opener.parent().is_none() {
        return false;
    }
    let top_id = opener.top().browsing_context_id();
    match POLICIES.lock().unwrap().get(&top_id) {
        Some((CoopValue::SameOrigin, top_origin)) => top_origin != opener_origin,
        _ => false,
    }
}
