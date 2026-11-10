// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod clippath;
mod converter;
mod filter;
mod image;
mod marker;
mod mask;
mod options;
mod paint_server;
mod shapes;
mod style;
mod svgtree;
mod switch;
mod units;
mod use_node;

#[cfg(feature = "text")]
mod text;
#[cfg(feature = "text")]
pub(crate) use converter::Cache;
pub use image::{ImageHrefDataResolverFn, ImageHrefResolver, ImageHrefStringResolverFn};
pub use options::Options;
#[cfg(feature = "writer")]
pub(crate) use svgtree::{AId, EId};

/// List of all errors.
#[derive(Debug)]
pub enum Error {
    /// Only UTF-8 content are supported.
    NotAnUtf8Str,

    /// `svgz` feature is required to parse SVGZ data.
    SvgzFeatureNotEnabled,

    /// Compressed SVG must use the GZip algorithm.
    MalformedGZip,

    /// We do not allow SVG with more than 1_000_000 elements for security reasons.
    ElementsLimitReached,

    /// SVG doesn't have a valid size.
    ///
    /// Occurs when width and/or height are <= 0.
    ///
    /// Also occurs if width, height and viewBox are not set.
    InvalidSize,

    /// Failed to parse an SVG data.
    ParsingFailed(roxmltree::Error),
}

impl From<roxmltree::Error> for Error {
    fn from(e: roxmltree::Error) -> Self {
        Error::ParsingFailed(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::NotAnUtf8Str => {
                write!(f, "provided data has not an UTF-8 encoding")
            }
            Self::SvgzFeatureNotEnabled => {
                write!(f, "enable svgz cargo feature to decode SVGZ data")
            }
            Error::MalformedGZip => {
                write!(f, "provided data has a malformed GZip content")
            }
            Error::ElementsLimitReached => {
                write!(f, "the maximum number of SVG elements has been reached")
            }
            Error::InvalidSize => {
                write!(f, "SVG has an invalid size")
            }
            Error::ParsingFailed(ref e) => {
                write!(f, "SVG data parsing failed cause {}", e)
            }
        }
    }
}

impl std::error::Error for Error {}

pub(crate) trait OptionLog {
    fn log_none<F: FnOnce()>(self, f: F) -> Self;
}

impl<T> OptionLog for Option<T> {
    #[inline]
    fn log_none<F: FnOnce()>(self, f: F) -> Self {
        self.or_else(|| {
            f();
            None
        })
    }
}

// Armature (AU-007): svgtypes predates CSS viewport units. Replace dimensions
// in SVG length attributes and inline styles before handing a nested image to
// the SVG parser. Leave XML markup and unrelated attributes intact.
fn resolve_viewport_lengths(text: &str, viewport: crate::Size) -> Option<String> {
    if !text.contains("vw") && !text.contains("vh") {
        return None;
    }
    let document = roxmltree::Document::parse(text).ok()?;
    let mut edits = Vec::new();
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        for attribute in node.attributes() {
            let is_length = matches!(
                attribute.name(),
                "width" | "height" | "x" | "y" | "x1" | "y1" | "x2" | "y2" | "cx" |
                    "cy" | "r" | "rx" | "ry" | "dx" | "dy" | "stroke-width" |
                    "stroke-dashoffset" | "stroke-dasharray" | "font-size" |
                    "letter-spacing" | "word-spacing" | "startOffset" | "style"
            );
            if !is_length || attribute.namespace().is_some() {
                continue;
            }
            let range = attribute.range();
            let source = &text[range.clone()];
            let Some(quote_start) = source.find(['\'', '"']) else {
                continue;
            };
            let quote = source.as_bytes()[quote_start];
            let Some(quote_end) = source[quote_start + 1..]
                .bytes()
                .position(|byte| byte == quote)
                .map(|offset| quote_start + 1 + offset)
            else {
                continue;
            };
            let value_range = range.start + quote_start + 1..range.start + quote_end;
            let value = &text[value_range.clone()];
            let replacement = if attribute.name() == "style" {
                replace_viewport_style_dimensions(value, viewport.width(), viewport.height())
            } else {
                replace_viewport_dimensions(value, viewport.width(), viewport.height())
            };
            if let Some(value) = replacement {
                edits.push((value_range, value));
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut resolved = text.to_owned();
    for (range, value) in edits {
        resolved.replace_range(range, &value);
    }
    Some(resolved)
}

// Armature (AU-007): Only CSS declarations with length values are rewritten;
// URLs and strings in paint declarations must retain their original bytes.
fn replace_viewport_style_dimensions(style: &str, width: f32, height: f32) -> Option<String> {
    let mut result = String::new();
    let mut changed = false;
    for declaration in style.split_inclusive(';') {
        if let Some((name, value)) = declaration.split_once(':') &&
            matches!(
                name.trim(),
                "width" | "height" | "x" | "y" | "cx" | "cy" | "r" | "rx" | "ry" |
                    "stroke-width" | "stroke-dashoffset" | "stroke-dasharray" |
                    "font-size" | "letter-spacing" | "word-spacing" | "transform-origin"
            ) &&
            let Some(value) = replace_viewport_dimensions(value, width, height)
        {
            result.push_str(name);
            result.push(':');
            result.push_str(&value);
            changed = true;
        } else {
            result.push_str(declaration);
        }
    }
    changed.then_some(result)
}

// Armature (AU-007): Replace CSS dimensions rather than substrings, so
// identifiers such as `icon50vw` and longer units are left alone.
fn replace_viewport_dimensions(value: &str, width: f32, height: f32) -> Option<String> {
    let bytes = value.as_bytes();
    let mut result = String::new();
    let mut last = 0;
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if start > 0 &&
            (bytes[start - 1].is_ascii_alphanumeric() || matches!(bytes[start - 1], b'_' | b'-'))
        {
            index += 1;
            continue;
        }
        if matches!(bytes[index], b'+' | b'-') {
            index += 1;
        }
        let digits_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        let mut has_digits = index != digits_start;
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            let fractional_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            has_digits |= index != fractional_start;
        }
        if !has_digits {
            index = start + 1;
            continue;
        }
        if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
            let exponent_start = index;
            index += 1;
            if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                index += 1;
            }
            let digits_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index == digits_start {
                index = exponent_start;
            }
        }
        let dimension = if value[index..].starts_with("vw") {
            width
        } else if value[index..].starts_with("vh") {
            height
        } else {
            index = start + 1;
            continue;
        };
        let end = index + 2;
        if end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            index = start + 1;
            continue;
        }
        if let Ok(number) = value[start..index].parse::<f32>() && number.is_finite() {
            result.push_str(&value[last..start]);
            result.push_str(&(number * dimension / 100.).to_string());
            result.push_str("px");
            last = end;
        }
        index = end;
    }
    if last == 0 {
        None
    } else {
        result.push_str(&value[last..]);
        Some(result)
    }
}

impl crate::Tree {
    /// Parses `Tree` from an SVG data.
    ///
    /// Can contain an SVG string or a gzip compressed data.
    pub fn from_data(data: &[u8], opt: &Options) -> Result<Self, Error> {
        if data.starts_with(&[0x1f, 0x8b]) {
            #[cfg(feature = "svgz")]
            {
                let data = decompress_svgz(data)?;
                let text = std::str::from_utf8(&data).map_err(|_| Error::NotAnUtf8Str)?;
                Self::from_str(text, opt)
            }

            #[cfg(not(feature = "svgz"))]
            Err(Error::SvgzFeatureNotEnabled)
        } else {
            let text = std::str::from_utf8(data).map_err(|_| Error::NotAnUtf8Str)?;
            Self::from_str(text, opt)
        }
    }

    /// Similar to the `from_data` method, except that it ignores all `image` elements linking to
    /// external files, as required by the SVG specification when SVG files are loaded
    /// for `<image href="..." />` tags.
    pub fn from_data_nested(data: &[u8], opt: &Options) -> Result<Self, Error> {
        Self::from_data_nested_with_size(data, opt, opt.default_size)
    }

    // Armature (AU-003): A nested SVG's relative root sizes use the embedding image viewport.
    pub(crate) fn from_data_nested_with_size(
        data: &[u8],
        opt: &Options,
        default_size: crate::Size,
    ) -> Result<Self, Error> {
        let nested_opt = Options {
            resources_dir: None,
            dpi: opt.dpi,
            font_size: opt.font_size,
            languages: opt.languages.clone(),
            shape_rendering: opt.shape_rendering,
            text_rendering: opt.text_rendering,
            image_rendering: opt.image_rendering,
            default_size,
            image_href_resolver: ImageHrefResolver {
                resolve_data: Box::new(|a, b, c| (opt.image_href_resolver.resolve_data)(a, b, c)),
                // External images should be ignored.
                resolve_string: Box::new(|_, _| None),
            },
            // In the referenced SVG, we start with the unmodified user-provided
            // fontdb, not the one from the cache.
            #[cfg(feature = "text")]
            fontdb: opt.fontdb.clone(),
            // Can't clone the resolver, so we create a new one that forwards to it.
            #[cfg(feature = "text")]
            font_resolver: crate::FontResolver {
                select_font: Box::new(|font, db| (opt.font_resolver.select_font)(font, db)),
                select_fallback: Box::new(|c, used_fonts, db| {
                    (opt.font_resolver.select_fallback)(c, used_fonts, db)
                }),
            },
            ..Options::default()
        };

        // Armature (AU-007): svgtypes does not recognize viewport units. Resolve
        // them while the embedding image's viewport is available, before parsing.
        let viewport_data = std::str::from_utf8(data)
            .ok()
            .and_then(|text| resolve_viewport_lengths(text, default_size));
        let data = viewport_data.as_deref().map_or(data, str::as_bytes);
        let mut tree = Self::from_data(data, &nested_opt)?;
        // Armature (AU-003): Content outside a relative nested SVG viewport must not
        // enlarge its intrinsic size and scale all of its descendants on embedding.
        if let Ok(text) = std::str::from_utf8(data) &&
            let Ok(document) = roxmltree::Document::parse(text)
        {
            let root = document.root_element();
            let relative = |name| {
                root.attribute(name).is_none_or(|value| {
                    let value = value.trim();
                    value.ends_with('%') || value.eq_ignore_ascii_case("auto")
                })
            };
            if relative("width") && relative("height") {
                tree.size = default_size;
            }
        }
        Ok(tree)
    }

    /// Parses `Tree` from an SVG string.
    pub fn from_str(text: &str, opt: &Options) -> Result<Self, Error> {
        let xml_opt = roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        };

        let doc =
            roxmltree::Document::parse_with_options(text, xml_opt).map_err(Error::ParsingFailed)?;

        Self::from_xmltree(&doc, opt)
    }

    /// Parses `Tree` from `roxmltree::Document`.
    pub fn from_xmltree(doc: &roxmltree::Document, opt: &Options) -> Result<Self, Error> {
        let doc = svgtree::Document::parse_tree(doc, opt.style_sheet.as_deref())?;
        self::converter::convert_doc(&doc, opt)
    }
}

/// Decompresses an SVGZ file.
#[cfg(feature = "svgz")]
pub fn decompress_svgz(data: &[u8]) -> Result<Vec<u8>, Error> {
    use std::io::Read;

    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut decoded = Vec::with_capacity(data.len() * 2);
    decoder
        .read_to_end(&mut decoded)
        .map_err(|_| Error::MalformedGZip)?;
    Ok(decoded)
}

#[inline]
pub(crate) fn f32_bound(min: f32, val: f32, max: f32) -> f32 {
    debug_assert!(min.is_finite());
    debug_assert!(max.is_finite());

    if val > max {
        max
    } else if val >= min {
        val
    } else {
        // Catches `val < min` as well as a NaN `val`.
        min
    }
}
