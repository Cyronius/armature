/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Cell, RefCell};
use std::ops::Deref;

use html5ever::buffer_queue::BufferQueue;
use html5ever::tokenizer::states::RawKind;
use html5ever::tokenizer::{
    Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer as HtmlTokenizer,
};
use html5ever::{Attribute, LocalName, local_name};
use js::jsapi::JSTracer;
use markup5ever::TokenizerResult;
use net_traits::blob_url_store::UrlWithBlobClaim;
use net_traits::request::{
    CorsSettings, CredentialsMode, Destination, ParserMetadata, Referrer, RequestClient,
};
use net_traits::{CoreResourceMsg, FetchChannels, ReferrerPolicy, ResourceThreads};
use servo_base::generic_channel::GenericSend;
use servo_base::id::{PipelineId, WebViewId};
use servo_url::ServoUrl;

use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::trace::{CustomTraceable, JSTraceable};
use crate::dom::document::Document;
use crate::dom::html::htmlscriptelement::script_fetch_request;
use crate::dom::processingoptions::determine_cors_settings_for_token;
use crate::fetch::fetch::create_a_potential_cors_request;
use crate::scripting::fetching_options::ScriptFetchOptions;

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct Tokenizer {
    #[ignore_malloc_size_of = "Defined in html5ever"]
    inner: TraceableTokenizer,
}

struct TraceableTokenizer(HtmlTokenizer<PrefetchSink>);

impl Deref for TraceableTokenizer {
    type Target = HtmlTokenizer<PrefetchSink>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[expect(unsafe_code)]
unsafe impl JSTraceable for TraceableTokenizer {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        unsafe { CustomTraceable::trace(&self.0, trc) }
    }
}

#[expect(unsafe_code)]
unsafe impl CustomTraceable for PrefetchSink {
    unsafe fn trace(&self, trc: *mut JSTracer) {
        unsafe { <Self as JSTraceable>::trace(self, trc) }
    }
}

impl Tokenizer {
    pub(crate) fn new(document: &Document) -> Self {
        let global = document.global();
        let sink = PrefetchSink {
            pipeline_id: global.pipeline_id(),
            webview_id: document.webview_id(),
            base_url: RefCell::new(None),
            document_url: document.url(),
            referrer: global.get_referrer(),
            referrer_policy: Cell::new(document.get_referrer_policy()),
            resource_threads: document.loader().resource_threads().clone(),
            // Initially we set prefetching to false, and only set it
            // true after the first script tag, since that is what will
            // block the main parser.
            prefetching: Cell::new(false),
            template_depth: Cell::new(0),
            templates: RefCell::new(Vec::new()),
            svg_depth: Cell::new(0),
            math_depth: Cell::new(0),
            picture_source: RefCell::new(None),
            images_blocked: Cell::new(false),
            request_client: global.request_client(None),
        };
        let options = Default::default();
        let inner = TraceableTokenizer(HtmlTokenizer::new(sink, options));
        Tokenizer { inner }
    }

    pub(crate) fn feed(&self, input: &BufferQueue) {
        while let TokenizerResult::Script(PrefetchHandle) = self.inner.feed(input) {}
    }
}

#[derive(JSTraceable)]
struct PrefetchSink {
    #[no_trace]
    pipeline_id: PipelineId,
    #[no_trace]
    webview_id: WebViewId,
    #[no_trace]
    document_url: ServoUrl,
    #[no_trace]
    base_url: RefCell<Option<ServoUrl>>,
    #[no_trace]
    referrer: Referrer,
    #[no_trace]
    referrer_policy: Cell<ReferrerPolicy>,
    #[no_trace]
    resource_threads: ResourceThreads,
    prefetching: Cell<bool>,
    /// How many `template` elements the scanner is in. Their contents are inert.
    template_depth: Cell<u32>,
    /// Whether each `template` element the scanner is in is inert. Declarative shadow roots are not,
    /// unless they are in an inert template.
    templates: RefCell<Vec<bool>>,
    /// How many `svg` and `math` elements the scanner is in.
    svg_depth: Cell<u32>,
    math_depth: Cell<u32>,
    /// The image that the first suitable `source` of the `picture` that the scanner is in asks for.
    #[no_trace]
    picture_source: RefCell<Option<Option<ServoUrl>>>,
    /// Whether a content security policy of the page forbids fetching images.
    images_blocked: Cell<bool>,
    #[no_trace]
    request_client: RequestClient,
}

/// The prefetch tokenizer produces trivial results
#[derive(Clone, Copy, JSTraceable)]
struct PrefetchHandle;

impl TokenSink for PrefetchSink {
    type Handle = PrefetchHandle;
    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<PrefetchHandle> {
        let tag = match token {
            Token::TagToken(ref tag) => tag,
            _ => return TokenSinkResult::Continue,
        };

        // Track the elements whose contents are not fetched, or are fetched differently.
        match (tag.kind, &tag.name) {
            (TagKind::StartTag, &local_name!("template")) => {
                let is_declarative_shadow_root = self
                    .get_attr(tag, local_name!("shadowrootmode"))
                    .is_some();
                let is_inert = self.template_depth.get() > 0 || !is_declarative_shadow_root;
                self.templates.borrow_mut().push(is_inert);
                if is_inert {
                    self.template_depth.set(self.template_depth.get() + 1);
                }
                return TokenSinkResult::Continue;
            },
            (TagKind::EndTag, &local_name!("template")) => {
                if self.templates.borrow_mut().pop() == Some(true) {
                    self.template_depth
                        .set(self.template_depth.get().saturating_sub(1));
                }
                return TokenSinkResult::Continue;
            },
            (TagKind::StartTag, &local_name!("picture")) => {
                *self.picture_source.borrow_mut() = Some(None);
                return TokenSinkResult::Continue;
            },
            (TagKind::EndTag, &local_name!("picture")) => {
                *self.picture_source.borrow_mut() = None;
                return TokenSinkResult::Continue;
            },
            (TagKind::StartTag, &local_name!("source")) => {
                // The first source of a picture without conditions that fail is the one that its image uses.
                let mut picture_source = self.picture_source.borrow_mut();
                if let Some(None) = *picture_source &&
                    self.get_attr(tag, local_name!("media")).is_none() &&
                    self.get_attr(tag, local_name!("type")).is_none_or(|attr| {
                        attr.value.trim().is_empty() ||
                            attr.value.trim().to_ascii_lowercase().starts_with("image/")
                    })
                {
                    *picture_source =
                        Some(self.get_attr(tag, local_name!("srcset")).and_then(|attr| {
                            self.parse_srcset(&attr.value)
                        }));
                }
                return TokenSinkResult::Continue;
            },
            (TagKind::StartTag, &local_name!("svg")) if !tag.self_closing => {
                self.svg_depth.set(self.svg_depth.get() + 1);
                return TokenSinkResult::Continue;
            },
            (TagKind::EndTag, &local_name!("svg")) => {
                self.svg_depth.set(self.svg_depth.get().saturating_sub(1));
                return TokenSinkResult::Continue;
            },
            (TagKind::StartTag, &local_name!("math")) if !tag.self_closing => {
                self.math_depth.set(self.math_depth.get() + 1);
                return TokenSinkResult::Continue;
            },
            (TagKind::EndTag, &local_name!("math")) => {
                self.math_depth.set(self.math_depth.get().saturating_sub(1));
                return TokenSinkResult::Continue;
            },
            (TagKind::StartTag, &local_name!("xmp")) |
            (TagKind::StartTag, &local_name!("style")) |
            (TagKind::StartTag, &local_name!("iframe")) |
            (TagKind::StartTag, &local_name!("noembed")) |
            (TagKind::StartTag, &local_name!("noframes"))
                if self.svg_depth.get() == 0 && self.math_depth.get() == 0 =>
            {
                return TokenSinkResult::RawData(RawKind::Rawtext);
            },
            (TagKind::StartTag, &local_name!("textarea")) |
            (TagKind::StartTag, &local_name!("title"))
                if self.svg_depth.get() == 0 && self.math_depth.get() == 0 =>
            {
                return TokenSinkResult::RawData(RawKind::Rcdata);
            },
            _ => {},
        }

        let fetching = self.prefetching.get() && self.template_depth.get() == 0;
        let in_svg = self.svg_depth.get() > 0;
        let in_math = self.math_depth.get() > 0;

        match (tag.kind, &tag.name) {
            (TagKind::StartTag, &local_name!("script")) if fetching => {
                // Scripts that won't run are not fetched: module-capable browsers skip `nomodule`
                // scripts, and only JavaScript and module scripts are fetched.
                let is_fetchable = !in_math &&
                    self.get_attr(tag, local_name!("nomodule")).is_none() &&
                    self.get_attr(tag, local_name!("type")).is_none_or(|attr| {
                        let value = attr.value.trim();
                        value.is_empty() ||
                            value.eq_ignore_ascii_case("module") ||
                            is_javascript_mime_type(value)
                    });
                let url = if in_svg {
                    // SVG scripts take their URL from `href`, or the legacy `xlink:href`.
                    self.get_url(tag, local_name!("href"))
                        .or_else(|| self.get_url(tag, LocalName::from("xlink:href")))
                } else {
                    self.get_url(tag, local_name!("src"))
                };
                if let Some(url) = url &&
                    is_fetchable
                {
                    debug!("Prefetch script {}", url);
                    let cors_setting = self.get_cors_settings(tag, local_name!("crossorigin"));
                    let integrity_metadata = self
                        .get_attr(tag, local_name!("integrity"))
                        .map(|attr| String::from(&attr.value))
                        .unwrap_or_default();
                    let cryptographic_nonce = self
                        .get_attr(tag, local_name!("nonce"))
                        .map(|attr| String::from(&attr.value))
                        .unwrap_or_default();
                    let request = script_fetch_request(
                        self.webview_id,
                        UrlWithBlobClaim::from_url_without_having_claimed_blob(url),
                        cors_setting,
                        ScriptFetchOptions {
                            referrer_policy: self
                                .get_referrer_policy(tag, local_name!("referrerpolicy")),
                            integrity_metadata,
                            cryptographic_nonce,
                            credentials_mode: CredentialsMode::CredentialsSameOrigin,
                            parser_metadata: ParserMetadata::ParserInserted,
                            render_blocking: false,
                        },
                        self.referrer.clone(),
                    )
                    .client(self.request_client.clone())
                    .pipeline_id(Some(self.pipeline_id));
                    let _ = self
                        .resource_threads
                        .send(CoreResourceMsg::Fetch(request, FetchChannels::Prefetch));
                }
                TokenSinkResult::RawData(RawKind::ScriptData)
            },
            // `image` is parsed as `img`, and lazy images are not fetched until they get near the
            // viewport.
            (TagKind::StartTag, &local_name!("img")) | (TagKind::StartTag, &local_name!("image"))
                if fetching &&
                    !in_math &&
                    !self.images_blocked.get() &&
                    !self
                        .get_attr(tag, local_name!("loading"))
                        .is_some_and(|attr| attr.value.eq_ignore_ascii_case("lazy")) =>
            {
                let url = if in_svg {
                    // SVG images take their URL from `href`, or the legacy `xlink:href`.
                    if tag.name == local_name!("image") {
                        self.get_url(tag, local_name!("href"))
                            .or_else(|| self.get_url(tag, LocalName::from("xlink:href")))
                    } else {
                        None
                    }
                } else {
                    // The image of an `img` is the one of its `src`, of a source of its picture,
                    // or of its `srcset`.
                    self.get_url(tag, local_name!("src"))
                        .or_else(|| {
                            self.picture_source
                                .borrow()
                                .clone()
                                .flatten()
                        })
                        .or_else(|| {
                            self.get_attr(tag, local_name!("srcset"))
                                .and_then(|attr| self.parse_srcset(&attr.value))
                        })
                };
                if let Some(url) = url {
                    debug!("Prefetch {} {}", tag.name, url);
                    let request = create_a_potential_cors_request(
                        Some(self.webview_id),
                        url,
                        Destination::Image,
                        self.get_cors_settings(tag, local_name!("crossorigin")),
                        None,
                        self.referrer.clone(),
                    )
                    .client(self.request_client.clone())
                    .pipeline_id(Some(self.pipeline_id))
                    .referrer_policy(self.get_referrer_policy(tag, local_name!("referrerpolicy")));

                    let _ = self
                        .resource_threads
                        .send(CoreResourceMsg::Fetch(request, FetchChannels::Prefetch));
                }
                TokenSinkResult::Continue
            },
            (TagKind::StartTag, &local_name!("link")) if fetching && !in_math => {
                let rel = self
                    .get_attr(tag, local_name!("rel"))
                    .map(|attr| attr.value.to_ascii_lowercase())
                    .unwrap_or_default();
                let has_rel = |token: &str| rel.split_ascii_whitespace().any(|rel| rel == token);
                let destination = if has_rel("stylesheet") &&
                    !has_rel("alternate") &&
                    self.get_attr(tag, local_name!("disabled")).is_none() &&
                    self.get_attr(tag, local_name!("type")).is_none_or(|attr| {
                        let value = attr.value.trim();
                        value.is_empty() || value.eq_ignore_ascii_case("text/css")
                    }) {
                    Some(Destination::Style)
                } else if has_rel("preload") {
                    self.get_attr(tag, local_name!("as"))
                        .and_then(|attr| match &*attr.value.to_ascii_lowercase() {
                            "image" => Some(Destination::Image),
                            "script" => Some(Destination::Script),
                            "style" => Some(Destination::Style),
                            "font" => Some(Destination::Font),
                            _ => None,
                        })
                } else {
                    None
                };
                if let Some(destination) = destination &&
                    let Some(url) = self.get_url(tag, local_name!("href"))
                {
                    debug!("Prefetch {} {}", tag.name, url);
                    let mut cors_setting = self.get_cors_settings(tag, local_name!("crossorigin"));
                    // Fonts are always fetched with CORS.
                    if destination == Destination::Font && cors_setting.is_none() {
                        cors_setting = Some(CorsSettings::Anonymous);
                    }
                    let referrer_policy =
                        self.get_referrer_policy(tag, local_name!("referrerpolicy"));
                    let integrity_metadata = self
                        .get_attr(tag, local_name!("integrity"))
                        .map(|attr| String::from(&attr.value))
                        .unwrap_or_default();

                    // https://html.spec.whatwg.org/multipage/#default-fetch-and-process-the-linked-resource
                    let request = create_a_potential_cors_request(
                        Some(self.webview_id),
                        url,
                        destination,
                        cors_setting,
                        None,
                        self.referrer.clone(),
                    )
                    .client(self.request_client.clone())
                    .pipeline_id(Some(self.pipeline_id))
                    .referrer_policy(referrer_policy)
                    .integrity_metadata(integrity_metadata);

                    let _ = self
                        .resource_threads
                        .send(CoreResourceMsg::Fetch(request, FetchChannels::Prefetch));
                }
                TokenSinkResult::Continue
            },
            (TagKind::StartTag, &local_name!("video")) if fetching && !in_svg && !in_math => {
                if let Some(url) = self.get_url(tag, local_name!("poster")) {
                    debug!("Prefetch {} poster {}", tag.name, url);
                    let request = create_a_potential_cors_request(
                        Some(self.webview_id),
                        url,
                        Destination::Image,
                        None,
                        None,
                        self.referrer.clone(),
                    )
                    .client(self.request_client.clone())
                    .pipeline_id(Some(self.pipeline_id))
                    .referrer_policy(self.referrer_policy.get());

                    let _ = self
                        .resource_threads
                        .send(CoreResourceMsg::Fetch(request, FetchChannels::Prefetch));
                }
                TokenSinkResult::Continue
            },
            (TagKind::StartTag, &local_name!("meta")) => {
                // A content security policy that forbids images stops the images from being fetched.
                if self
                    .get_attr(tag, local_name!("http-equiv"))
                    .is_some_and(|attr| attr.value.eq_ignore_ascii_case("content-security-policy")) &&
                    let Some(content) = self.get_attr(tag, local_name!("content"))
                {
                    let content = content.value.to_ascii_lowercase();
                    if content.contains("img-src 'none'") {
                        self.images_blocked.set(true);
                    }
                }
                // <https://html.spec.whatwg.org/multipage/#meta-referrer>
                if self
                    .get_attr(tag, local_name!("name"))
                    .is_some_and(|attr| attr.value.eq_ignore_ascii_case("referrer")) &&
                    let Some(content) = self.get_attr(tag, local_name!("content"))
                {
                    let policy = ReferrerPolicy::from(&*content.value);
                    if policy != ReferrerPolicy::EmptyString {
                        self.referrer_policy.set(policy);
                    }
                }
                TokenSinkResult::Continue
            },
            (TagKind::StartTag, &local_name!("script")) => {
                TokenSinkResult::RawData(RawKind::ScriptData)
            },
            (TagKind::EndTag, &local_name!("script")) => {
                // After the first script tag, the main parser is blocked, so it's worth prefetching.
                self.prefetching.set(true);
                TokenSinkResult::Script(PrefetchHandle)
            },
            (TagKind::StartTag, &local_name!("base")) => {
                if let Some(url) = self.get_url(tag, local_name!("href")) &&
                    self.base_url.borrow().is_none()
                {
                    debug!("Setting base {}", url);
                    *self.base_url.borrow_mut() = Some(url);
                }
                TokenSinkResult::Continue
            },
            _ => TokenSinkResult::Continue,
        }
    }
}

/// Whether `value` is the MIME type of a JavaScript script.
///
/// <https://mimesniff.spec.whatwg.org/#javascript-mime-type>
fn is_javascript_mime_type(value: &str) -> bool {
    let essence = value
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    matches!(
        essence.as_str(),
        "application/ecmascript" |
            "application/javascript" |
            "application/x-ecmascript" |
            "application/x-javascript" |
            "text/ecmascript" |
            "text/javascript" |
            "text/javascript1.0" |
            "text/javascript1.1" |
            "text/javascript1.2" |
            "text/javascript1.3" |
            "text/javascript1.4" |
            "text/javascript1.5" |
            "text/jscript" |
            "text/livescript" |
            "text/x-ecmascript" |
            "text/x-javascript"
    )
}

impl PrefetchSink {
    fn get_attr<'a>(&'a self, tag: &'a Tag, name: LocalName) -> Option<&'a Attribute> {
        tag.attrs.iter().find(|attr| attr.name.local == name)
    }

    /// The URL that the scanner expects a `srcset` to select: that of the candidate for the
    /// density 1, or else of its first candidate.
    fn parse_srcset(&self, srcset: &str) -> Option<ServoUrl> {
        let candidates: Vec<(&str, &str)> = srcset
            .split(',')
            .filter_map(|candidate| {
                let mut parts = candidate.split_ascii_whitespace();
                let url = parts.next()?;
                Some((url, parts.next().unwrap_or("1x")))
            })
            .collect();
        let (url, _) = candidates
            .iter()
            .find(|(_, descriptor)| *descriptor == "1x")
            .or_else(|| candidates.first())?;
        let base_url = self.base_url.borrow();
        let base = base_url.as_ref().unwrap_or(&self.document_url);
        ServoUrl::parse_with_base(Some(base), url).ok()
    }

    fn get_url(&self, tag: &Tag, name: LocalName) -> Option<ServoUrl> {
        let attr = self.get_attr(tag, name)?;
        let base_url = self.base_url.borrow();
        let base = base_url.as_ref().unwrap_or(&self.document_url);
        ServoUrl::parse_with_base(Some(base), &attr.value).ok()
    }

    fn get_referrer_policy(&self, tag: &Tag, name: LocalName) -> ReferrerPolicy {
        self.get_attr(tag, name)
            .map(|attr| ReferrerPolicy::from(&*attr.value))
            .unwrap_or(self.referrer_policy.get())
    }

    fn get_cors_settings(&self, tag: &Tag, name: LocalName) -> Option<CorsSettings> {
        let attr = self.get_attr(tag, name)?;
        determine_cors_settings_for_token(&attr.value)
    }
}
