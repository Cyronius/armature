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

impl EmbedderPolicy {
    /// <https://html.spec.whatwg.org/multipage/#obtaining-an-embedder-policy>
    pub fn from_headers(headers: Option<&http::HeaderMap>) -> EmbedderPolicy {
        // The `Cross-Origin-Embedder-Policy` header is a structured field token, with parameters.
        let value = headers
            .and_then(|headers| headers.get("Cross-Origin-Embedder-Policy"))
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or_default().trim())
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
