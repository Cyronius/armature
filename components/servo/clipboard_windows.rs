/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::BTreeMap;
use std::io::Cursor;

use clipboard_win::{Clipboard, formats, options, raw};
use embedder_traits::ClipboardData;
use image::{ImageFormat, ImageReader};

const HTML_FORMAT: &str = "HTML Format";
const CUSTOM_FORMAT_MAP: &str = "Web Custom Format Map";
const CUSTOM_FORMAT_PREFIX: &str = "Web Custom Format";

enum NativeRepresentation {
    Text(String),
    Html(u32, String),
    Bytes(u32, Vec<u8>),
}

fn register_format(name: &str) -> Result<u32, String> {
    raw::register_format(name)
        .map(|format| format.get())
        .ok_or_else(|| format!("Could not register clipboard format {name}"))
}

pub(super) fn native_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n")
}

fn transcode_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let image = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png)
        .decode()
        .map_err(|error| error.to_string())?;
    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(encoded.into_inner())
}

pub(super) fn set_data(data: ClipboardData, validate_png: bool) -> Result<(), String> {
    // Prepare all payloads before replacing the existing clipboard item.
    let mut native_data = Vec::new();
    let mut custom_formats = BTreeMap::new();
    for (format, bytes) in data {
        let representation = match format.as_str() {
            "text/plain" => NativeRepresentation::Text(native_text(&bytes)),
            "text/html" => NativeRepresentation::Html(
                register_format(HTML_FORMAT)?,
                String::from_utf8_lossy(&bytes).into_owned(),
            ),
            "image/png" => NativeRepresentation::Bytes(
                register_format("PNG")?,
                if validate_png {
                    transcode_png(&bytes)?
                } else {
                    bytes
                },
            ),
            "image/svg+xml" | "text/uri-list" => {
                NativeRepresentation::Bytes(register_format(&format)?, bytes)
            },
            _ => {
                let Some(mime) = format.strip_prefix("web ") else {
                    return Err(format!("Unsupported clipboard format {format}"));
                };
                if custom_formats.len() == 100 {
                    return Err("Too many custom clipboard formats".into());
                }
                let name = format!("{CUSTOM_FORMAT_PREFIX}{}", custom_formats.len());
                let id = register_format(&name)?;
                custom_formats.insert(mime.to_owned(), name);
                NativeRepresentation::Bytes(id, bytes)
            },
        };
        native_data.push(representation);
    }
    if !custom_formats.is_empty() {
        let json = serde_json::to_vec(&custom_formats).map_err(|error| error.to_string())?;
        native_data.push(NativeRepresentation::Bytes(
            register_format(CUSTOM_FORMAT_MAP)?,
            json,
        ));
    }

    let _clipboard = Clipboard::new_attempts(10).map_err(|error| error.to_string())?;
    raw::empty().map_err(|error| error.to_string())?;
    for representation in native_data {
        let result = match representation {
            NativeRepresentation::Text(text) => raw::set_string_with(&text, options::NoClear),
            NativeRepresentation::Html(id, html) => raw::set_html_with(id, &html, options::NoClear),
            NativeRepresentation::Bytes(id, bytes) => raw::set_without_clear(id, &bytes),
        };
        if let Err(error) = result {
            // Keep a failed multi-format write from leaving an incomplete item.
            let _ = raw::empty();
            return Err(error.to_string());
        }
    }
    Ok(())
}

fn read_named_format(name: &str) -> Option<Vec<u8>> {
    let id = register_format(name).ok()?;
    let mut bytes = Vec::new();
    raw::get_vec(id, &mut bytes).ok()?;
    Some(bytes)
}

fn is_custom_slot(name: &str) -> bool {
    let Some(index) = name.strip_prefix(CUSTOM_FORMAT_PREFIX) else {
        return false;
    };
    index
        .parse::<u8>()
        .is_ok_and(|value| value < 100 && index == value.to_string())
}

fn html_fragment(bytes: &[u8]) -> Option<&[u8]> {
    let header = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let offset = |name: &str| {
        header
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|value| value.trim().parse::<usize>().ok())
    };
    bytes.get(offset("StartFragment:")?..offset("EndFragment:")?)
}

pub(super) fn get_data() -> Result<ClipboardData, String> {
    let _clipboard = Clipboard::new_attempts(10).map_err(|error| error.to_string())?;
    let mut data = Vec::new();
    let mut text = Vec::new();
    if raw::is_format_avail(formats::CF_UNICODETEXT) && raw::get_string(&mut text).is_ok() {
        data.push(("text/plain".into(), text));
    }

    if let Some(html) = read_named_format(HTML_FORMAT) &&
        let Some(fragment) = html_fragment(&html)
    {
        data.push(("text/html".into(), fragment.to_vec()));
    }
    for (mime, name) in [
        ("image/png", "PNG"),
        ("image/svg+xml", "image/svg+xml"),
        ("text/uri-list", "text/uri-list"),
    ] {
        if let Some(bytes) = read_named_format(name) {
            let bytes = if mime == "image/png" {
                let Ok(bytes) = transcode_png(&bytes) else {
                    continue;
                };
                bytes
            } else {
                bytes
            };
            data.push((mime.into(), bytes));
        }
    }
    if let Some(map) = read_named_format(CUSTOM_FORMAT_MAP) {
        // Windows may pad the underlying allocation with NUL bytes.
        let map = map.split(|&byte| byte == 0).next().unwrap_or_default();
        if let Ok(formats) = serde_json::from_slice::<BTreeMap<String, String>>(map) {
            for (mime, name) in formats.into_iter().take(100) {
                if is_custom_slot(&name) &&
                    let Some(bytes) = read_named_format(&name)
                {
                    data.push((format!("web {mime}"), bytes));
                }
            }
        }
    }
    Ok(data)
}
