/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Helpers for CSS value parsing.

use std::sync::LazyLock;

use net_traits::ReferrerPolicy;
use net_traits::request::Referrer;
use style::context::QuirksMode;
use style::error_reporting::ParseErrorReporter;
use style::parser::ParserContext;
use style::stylesheets::{CssRuleType, Origin, StylesheetReferrer, UrlExtraData};
use style_traits::ParsingMode;
use url::Url;

use crate::dom::document::Document;
use crate::dom::globalscope::GlobalScope;

pub(crate) static ANONYMOUS_CONTENT_URL_DATA: LazyLock<UrlExtraData> =
    LazyLock::new(|| Url::parse("about:blank").unwrap().into());

/// Creates a `ParserContext` from the given document.
///
/// Automatically configures quirks mode and error reporter from the document.
pub(crate) fn parser_context_for_document<'a>(
    document: &'a Document,
    rule_type: CssRuleType,
    parsing_mode: ParsingMode,
    url_data: &'a UrlExtraData,
) -> ParserContext<'a> {
    let quirks_mode = document.quirks_mode();
    let error_reporter = document.window().css_error_reporter();

    ParserContext::new(
        Origin::Author,
        url_data,
        Some(rule_type),
        parsing_mode,
        quirks_mode,
        /* namespaces = */ Default::default(),
        Some(error_reporter),
        None,
        /* attr_taint = */ Default::default(),
    )
}

/// Like [`parser_context_for_document`], but with a custom error reporter.
pub(crate) fn parser_context_for_document_with_reporter<'a>(
    document: &'a Document,
    rule_type: CssRuleType,
    parsing_mode: ParsingMode,
    url_data: &'a UrlExtraData,
    error_reporter: &'a dyn ParseErrorReporter,
) -> ParserContext<'a> {
    let quirks_mode = document.quirks_mode();

    ParserContext::new(
        Origin::Author,
        url_data,
        Some(rule_type),
        parsing_mode,
        quirks_mode,
        /* namespaces = */ Default::default(),
        Some(error_reporter),
        None,
        /* attr_taint = */ Default::default(),
    )
}

/// Creates a `ParserContext` without a document, using no quirks mode
/// and no error reporter.
pub(crate) fn parser_context_for_anonymous_content<'a>(
    rule_type: CssRuleType,
    parsing_mode: ParsingMode,
    url_data: &'a UrlExtraData,
) -> ParserContext<'a> {
    ParserContext::new(
        Origin::Author,
        url_data,
        Some(rule_type),
        parsing_mode,
        QuirksMode::NoQuirks,
        /* namespaces = */ Default::default(),
        None,
        None,
        /* attr_taint = */ Default::default(),
    )
}

/// The referrer and referrer policy of a fetch of a resource that a style sheet refers to. For a
/// resource of an external style sheet, these are the style sheet's URL and the referrer policy of
/// its response; otherwise, those of the global.
/// <https://w3c.github.io/webappsec-referrer-policy/#integration-with-css>
pub(crate) fn stylesheet_referrer(
    global: &GlobalScope,
    referrer: Option<&StylesheetReferrer>,
) -> (Referrer, ReferrerPolicy) {
    match referrer {
        Some(referrer) => (
            Referrer::ReferrerUrl(referrer.url.clone().into()),
            ReferrerPolicy::from(referrer.policy.as_str()),
        ),
        None => (global.get_referrer(), ReferrerPolicy::EmptyString),
    }
}
