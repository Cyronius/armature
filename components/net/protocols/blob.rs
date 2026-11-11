/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::future::{Future, ready};
use std::pin::Pin;

use headers::{HeaderMapExt, Range};
use http::Method;
use log::debug;
use net_traits::blob_url_store::{BlobURLStoreError, parse_blob_url};
use net_traits::http_status::HttpStatus;
use net_traits::request::Request;
use net_traits::response::{Response, ResponseBody};
use net_traits::{NetworkError, ResourceFetchTiming};
use tokio::sync::mpsc::unbounded_channel;

use crate::fetch::methods::{Data, DoneChannel, FetchContext};
use crate::protocols::{ProtocolHandler, partial_content};

#[derive(Default)]
pub struct BlobProtocolHander {}

impl ProtocolHandler for BlobProtocolHander {
    fn load(
        &self,
        request: &mut Request,
        done_chan: &mut DoneChannel,
        context: &FetchContext,
    ) -> Pin<Box<dyn Future<Output = Response> + Send>> {
        let url_and_blob_claim = request.current_url_with_blob_claim();
        debug!("Loading blob {}", url_and_blob_claim.as_str());

        // Step 2.
        if request.method != Method::GET {
            return Box::pin(ready(Response::network_error(NetworkError::InvalidMethod)));
        }

        // Armature: parse the `Range` header strictly, as the Fetch standard requires.
        let range_header = match request.headers.get(http::header::RANGE) {
            Some(value) => match parse_single_range_header_value(value.as_bytes()) {
                Some(range) => Some(range),
                None => {
                    return Box::pin(ready(Response::network_error(
                        NetworkError::ResourceLoadError("Invalid range header".into()),
                    )));
                },
            },
            None => None,
        };
        let is_range_request = range_header.is_some();

        let (file_id, origin) = if let Some(token) = url_and_blob_claim.token() {
            (token.file_id, token.origin.clone())
        } else {
            // FIXME: This should never happen, we should have acquired a token beforehand
            let Ok(id) = parse_blob_url(&url_and_blob_claim.url()) else {
                return Box::pin(ready(Response::network_error(
                    NetworkError::ResourceLoadError("Invalid blob URL".into()),
                )));
            };
            (id, url_and_blob_claim.url().origin())
        };

        let mut response = Response::new(
            url_and_blob_claim.url(),
            ResourceFetchTiming::new(request.timing_type()),
        );
        response.status = HttpStatus::default();

        if is_range_request {
            response.range_requested = true;
            partial_content(&mut response);
        }

        let (mut done_sender, done_receiver) = unbounded_channel();
        *done_chan = Some((done_sender.clone(), done_receiver));
        *response.body.lock() = ResponseBody::Receiving(vec![]);

        if let Err(err) = context.filemanager.fetch_file(
            &mut done_sender,
            context.cancellation_listener.clone(),
            file_id,
            &context.file_token,
            origin,
            &mut response,
            range_header,
        ) {
            let _ = done_sender.send(Data::Done);
            let err = match err {
                BlobURLStoreError::InvalidRange => {
                    // The range cannot be satisfied: Fetch treats this as a network error.
                    return Box::pin(ready(Response::network_error(
                        NetworkError::ResourceLoadError("Range not satisfiable".into()),
                    )));
                },
                _ => format!("{:?}", err),
            };
            return Box::pin(ready(Response::network_error(
                NetworkError::BlobURLStoreError(err),
            )));
        };

        Box::pin(ready(response))
    }
}

/// <https://fetch.spec.whatwg.org/#simple-range-header-value> with `allowWhitespace` set.
fn parse_single_range_header_value(value: &[u8]) -> Option<Range> {
    fn skip_ws(data: &[u8], position: &mut usize) {
        while matches!(data.get(*position), Some(b' ' | b'\t')) {
            *position += 1;
        }
    }
    fn collect_digits(data: &[u8], position: &mut usize) -> Option<u64> {
        let start = *position;
        while data.get(*position).is_some_and(u8::is_ascii_digit) {
            *position += 1;
        }
        if start == *position {
            return None;
        }
        // Overflowing values saturate, which is equivalent for blob sizes.
        std::str::from_utf8(&data[start..*position])
            .ok()?
            .parse::<u64>()
            .ok()
            .or(Some(u64::MAX))
    }

    let data = value;
    let mut position = 0;
    if !data.starts_with(b"bytes") {
        return None;
    }
    position += "bytes".len();
    skip_ws(data, &mut position);
    if data.get(position) != Some(&b'=') {
        return None;
    }
    position += 1;
    skip_ws(data, &mut position);
    let start = collect_digits(data, &mut position);
    skip_ws(data, &mut position);
    if data.get(position) != Some(&b'-') {
        return None;
    }
    position += 1;
    skip_ws(data, &mut position);
    let end = collect_digits(data, &mut position);
    if position < data.len() {
        return None;
    }
    let normalized = match (start, end) {
        (None, None) => return None,
        (Some(start), Some(end)) if start > end => return None,
        (Some(start), Some(end)) => format!("bytes={start}-{end}"),
        (Some(start), None) => format!("bytes={start}-"),
        (None, Some(end)) => format!("bytes=-{end}"),
    };
    let header_value = http::HeaderValue::from_str(&normalized).ok()?;
    let mut headers = http::HeaderMap::new();
    headers.insert(http::header::RANGE, header_value);
    headers.typed_get::<Range>()
}
