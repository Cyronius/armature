/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use http::HeaderMap;
use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};
use servo_url::{Host, ImmutableOrigin, ServoUrl};

/// Clipboard read and write are separately delegated policy-controlled features.
#[derive(Clone, Copy)]
pub enum ClipboardFeature {
    Read,
    Write,
}

impl ClipboardFeature {
    const ALL: [Self; 2] = [Self::Read, Self::Write];

    fn name(self) -> &'static str {
        match self {
            Self::Read => "clipboard-read",
            Self::Write => "clipboard-write",
        }
    }
}

#[derive(Clone, Debug, Deserialize, MallocSizeOf, Serialize)]
enum OriginMatch {
    Exact(ImmutableOrigin),
    Subdomain(ImmutableOrigin),
}

impl OriginMatch {
    fn matches(&self, origin: &ImmutableOrigin) -> bool {
        match (self, origin) {
            (Self::Exact(allowed), origin) => allowed == origin,
            (
                Self::Subdomain(ImmutableOrigin::Tuple(scheme, Host::Domain(host), port)),
                ImmutableOrigin::Tuple(origin_scheme, Host::Domain(origin_host), origin_port),
            ) => {
                scheme == origin_scheme &&
                    port == origin_port &&
                    origin_host
                        .strip_suffix(host)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            },
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, MallocSizeOf, Serialize)]
enum AllowList {
    All,
    Origins(Vec<OriginMatch>),
}

impl AllowList {
    fn matches(&self, origin: &ImmutableOrigin) -> bool {
        match self {
            Self::All => true,
            Self::Origins(origins) => origins.iter().any(|pattern| pattern.matches(origin)),
        }
    }

    fn from_tokens<'a>(
        tokens: impl IntoIterator<Item = &'a str>,
        owner: &ImmutableOrigin,
        source: Option<&ImmutableOrigin>,
    ) -> Self {
        let mut allow = Vec::new();
        for token in tokens {
            match token {
                "*" => return Self::All,
                "self" | "'self'" => allow.push(OriginMatch::Exact(owner.clone())),
                "'src'" => allow.extend(source.cloned().map(OriginMatch::Exact)),
                "'none'" => return Self::Origins(Vec::new()),
                _ => {
                    let value = token.trim_matches('"');
                    let (value, wildcard) = match value.split_once("://*.") {
                        Some((scheme, host)) => (format!("{scheme}://{host}"), true),
                        None => (value.to_owned(), false),
                    };
                    if let Ok(url) = ServoUrl::parse(&value) &&
                        url.username().is_empty() &&
                        url.password().is_none() &&
                        url.path() == "/" &&
                        url.query().is_none() &&
                        url.fragment().is_none()
                    {
                        let origin = url.origin();
                        if !matches!(origin, ImmutableOrigin::Tuple(..)) {
                            continue;
                        }
                        allow.push(if wildcard {
                            OriginMatch::Subdomain(origin)
                        } else {
                            OriginMatch::Exact(origin)
                        });
                    }
                },
            }
        }
        Self::Origins(allow)
    }
}

#[derive(Clone, Debug, Deserialize, MallocSizeOf, Serialize)]
pub struct ClipboardPolicy {
    inherited: [bool; 2],
    declared: [Option<AllowList>; 2],
}

impl Default for ClipboardPolicy {
    fn default() -> Self {
        Self {
            inherited: [true; 2],
            declared: [None, None],
        }
    }
}

impl ClipboardPolicy {
    pub fn from_response(
        origin: &ImmutableOrigin,
        container: Option<&ClipboardContainerPolicy>,
        headers: Option<&HeaderMap>,
    ) -> Self {
        let mut policy = Self::default();
        for feature in ClipboardFeature::ALL {
            let index = feature as usize;
            policy.inherited[index] =
                container.is_none_or(|container| container.allows(feature, origin));
            policy.declared[index] = headers
                .into_iter()
                .flat_map(|headers| headers.get_all("permissions-policy").iter())
                .filter_map(|header| header.to_str().ok())
                .flat_map(|header| header.split(','))
                .filter_map(|directive| directive.trim().split_once('='))
                .filter(|(name, _)| name.trim() == feature.name())
                .map(|(_, value)| {
                    let value = value.trim();
                    let values = value
                        .strip_prefix('(')
                        .and_then(|value| value.strip_suffix(')'))
                        .unwrap_or(value);
                    AllowList::from_tokens(values.split_ascii_whitespace(), origin, None)
                })
                .last();
        }
        policy
    }

    pub fn allows(&self, feature: ClipboardFeature, origin: &ImmutableOrigin) -> bool {
        let index = feature as usize;
        self.inherited[index] &&
            self.declared[index]
                .as_ref()
                .is_none_or(|declaration| declaration.matches(origin))
    }
}

#[derive(Clone, Debug, Deserialize, MallocSizeOf, Serialize)]
pub struct ClipboardContainerPolicy {
    parent_policy: ClipboardPolicy,
    parent_origin: ImmutableOrigin,
    declared: [Option<AllowList>; 2],
}

impl ClipboardContainerPolicy {
    pub fn new(
        parent_policy: ClipboardPolicy,
        parent_origin: ImmutableOrigin,
        source_origin: &ImmutableOrigin,
        allow: &str,
    ) -> Self {
        let mut declared = [None, None];
        for directive in allow.split(';') {
            let mut tokens = directive.split_ascii_whitespace();
            let Some(name) = tokens.next() else { continue };
            let Some(feature) = ClipboardFeature::ALL
                .into_iter()
                .find(|feature| feature.name() == name)
            else {
                continue;
            };
            let entry = &mut declared[feature as usize];
            if entry.is_none() {
                *entry = Some(if tokens.clone().next().is_none() {
                    AllowList::Origins(vec![OriginMatch::Exact(source_origin.clone())])
                } else {
                    AllowList::from_tokens(tokens, &parent_origin, Some(source_origin))
                });
            }
        }
        Self {
            parent_policy,
            parent_origin,
            declared,
        }
    }

    fn allows(&self, feature: ClipboardFeature, origin: &ImmutableOrigin) -> bool {
        if !self.parent_policy.allows(feature, &self.parent_origin) ||
            !self.parent_policy.allows(feature, origin)
        {
            return false;
        }
        let index = feature as usize;
        if let Some(allow) = &self.declared[index] {
            return allow.matches(origin);
        }

        // An explicit response declaration can delegate beyond the default same-origin policy.
        self.parent_policy.declared[index].is_some() || &self.parent_origin == origin
    }
}
