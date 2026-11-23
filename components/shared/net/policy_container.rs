/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use content_security_policy::CspList;
use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};

use crate::ReferrerPolicy;

/// When a policy container is associated with a request, it has an additional state of "Client". As
/// per the spec:
///
/// `"client" is changed to a policy container during fetching. It provides a convenient way for
/// standards to not have to set request’s policy container.`
///
/// This can be achieved with an `Option` however this struct is used with the intent to reduce
/// ambiguity when mapping our implementation to the spec.
///
/// <https://fetch.spec.whatwg.org/#concept-request-policy-container>
#[derive(Clone, Debug, Default, Deserialize, MallocSizeOf, Serialize)]
pub enum RequestPolicyContainer {
    #[default]
    Client,
    PolicyContainer(#[conditional_malloc_size_of] Arc<PolicyContainer>),
}

impl RequestPolicyContainer {
    pub fn policy_container(&self) -> Option<&PolicyContainer> {
        match self {
            RequestPolicyContainer::Client => None,
            RequestPolicyContainer::PolicyContainer(policy_container) => Some(policy_container),
        }
    }
}

/// <https://html.spec.whatwg.org/multipage/#policy-containers>
#[derive(Clone, Debug, Default, Deserialize, MallocSizeOf, Serialize)]
pub struct PolicyContainer {
    #[ignore_malloc_size_of = "Defined in rust-content-security-policy"]
    /// <https://html.spec.whatwg.org/multipage/#policy-container-csp-list>
    pub csp_list: Option<CspList>,
    /// <https://html.spec.whatwg.org/multipage/#policy-container-embedder-policy>
    pub embedder_policy: EmbedderPolicy,
    /// <https://html.spec.whatwg.org/multipage/#policy-container-referrer-policy>
    pub referrer_policy: ReferrerPolicy,
}

impl PolicyContainer {
    pub fn set_csp_list(&mut self, csp_list: Option<CspList>) {
        self.csp_list = csp_list;
    }

    pub fn set_referrer_policy(&mut self, referrer_policy: ReferrerPolicy) {
        self.referrer_policy = referrer_policy;
    }

    pub fn get_referrer_policy(&self) -> ReferrerPolicy {
        // https://w3c.github.io/webappsec-referrer-policy/#referrer-policy-empty-string
        if self.referrer_policy == ReferrerPolicy::EmptyString {
            return ReferrerPolicy::default();
        }

        self.referrer_policy
    }
}

/// <https://html.spec.whatwg.org/multipage/#embedder-policy>
#[derive(Clone, Debug, Default, Deserialize, MallocSizeOf, Serialize)]
pub struct EmbedderPolicy {
    /// <https://html.spec.whatwg.org/multipage/#embedder-policy-value-2>
    pub value: EmbedderPolicyValue,
}

/// The token that the value of a header with a structured field item that has a token as its value
/// holds, if it is one: parameters are allowed but not looked at.
/// <https://httpwg.org/specs/rfc9651.html#parse-item>
pub fn structured_field_token(value: &str) -> Option<&str> {
    // Leading and trailing spaces are dropped.
    let value = value.trim_matches(' ');
    let token_end = value.find(';').unwrap_or(value.len());
    let token = &value[..token_end];
    let mut characters = token.chars();
    let valid_start = characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '*');
    let valid_rest = characters.all(|character| {
        character.is_ascii_alphanumeric() ||
            matches!(
                character,
                ':' | '/' | '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+' | '-' | '.' | '^' | '_' |
                    '`' | '|' | '~'
            )
    });
    if !valid_start || !valid_rest {
        return None;
    }
    // Each parameter is `;`, optional spaces, a key and optionally `=` and a bare item.
    let mut rest = &value[token_end..];
    while let Some(after_semicolon) = rest.strip_prefix(';') {
        let after_spaces = after_semicolon.trim_start_matches(' ');
        let key_end = after_spaces
            .find(|character: char| {
                !(character.is_ascii_lowercase() ||
                    character.is_ascii_digit() ||
                    matches!(character, '_' | '-' | '.' | '*'))
            })
            .unwrap_or(after_spaces.len());
        let key = &after_spaces[..key_end];
        if !key
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_lowercase() || first == '*')
        {
            return None;
        }
        rest = &after_spaces[key_end..];
        if let Some(after_equals) = rest.strip_prefix('=') {
            // The value is a bare item that runs up to the next parameter.
            let value_end = if after_equals.starts_with('"') {
                let closing = after_equals[1..].find('"')? + 2;
                closing
            } else {
                after_equals.find(';').unwrap_or(after_equals.len())
            };
            if value_end == 0 {
                return None;
            }
            rest = &after_equals[value_end..];
        }
    }
    rest.is_empty().then_some(token)
}

impl EmbedderPolicy {
    /// <https://html.spec.whatwg.org/multipage/#obtaining-an-embedder-policy>
    pub fn from_headers(headers: Option<&http::HeaderMap>) -> EmbedderPolicy {
        // The `Cross-Origin-Embedder-Policy` header is a structured field token, with parameters.
        let value = headers
            .and_then(|headers| headers.get("Cross-Origin-Embedder-Policy"))
            .and_then(|value| value.to_str().ok())
            .and_then(structured_field_token)
            .map(|token| match token {
                "require-corp" => EmbedderPolicyValue::RequireCorp,
                "credentialless" => EmbedderPolicyValue::Credentialless,
                _ => EmbedderPolicyValue::UnsafeNone,
            })
            .unwrap_or_default();
        EmbedderPolicy { value }
    }
}

/// <https://html.spec.whatwg.org/multipage/#embedder-policy-value>
#[derive(Clone, Copy, Debug, Default, Deserialize, MallocSizeOf, Serialize)]
pub enum EmbedderPolicyValue {
    /// "unsafe-none"
    #[default]
    UnsafeNone,
    /// "require-corp"
    RequireCorp,
    /// "credentialless"
    Credentialless,
}

impl EmbedderPolicyValue {
    /// The keyword of the value.
    pub fn as_str(&self) -> &'static str {
        match self {
            EmbedderPolicyValue::UnsafeNone => "unsafe-none",
            EmbedderPolicyValue::RequireCorp => "require-corp",
            EmbedderPolicyValue::Credentialless => "credentialless",
        }
    }
}
