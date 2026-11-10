/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::str::FromStr;

use dom_struct::dom_struct;
use http::header::{HeaderMap as HyperHeaders, HeaderName, HeaderValue};
use js::context::JSContext;
use js::rust::HandleObject;
use net_traits::fetch::headers::{
    extract_mime_type_from_values, get_decode_and_split_header_value, is_forbidden_method,
};
use net_traits::request::is_cors_safelisted_request_header;
use net_traits::trim_http_whitespace;
use script_bindings::cell::DomRefCell;
use script_bindings::cformat;
use script_bindings::reflector::{Reflector, reflect_dom_object_with_proto};
use style::str::starts_with_ignore_ascii_case;

use crate::dom::bindings::codegen::Bindings::HeadersBinding::{HeadersInit, HeadersMethods};
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::iterable::Iterable;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::{ByteString, is_token};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct Headers {
    reflector_: Reflector,
    guard: Cell<Guard>,
    /// Values representable by the HTTP library, for network requests and responses.
    #[no_trace]
    header_list: DomRefCell<HyperHeaders>,
    /// Fetch permits control bytes that the HTTP library rejects, so DOM operations use this list.
    dom_header_list: DomRefCell<Vec<(String, Vec<u8>)>>,
}

/// <https://fetch.spec.whatwg.org/#concept-headers-guard>
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf, PartialEq)]
pub(crate) enum Guard {
    Immutable,
    Request,
    RequestNoCors,
    Response,
    None,
}

impl Headers {
    pub(crate) fn new_inherited() -> Headers {
        Headers {
            reflector_: Reflector::new(),
            guard: Cell::new(Guard::None),
            header_list: DomRefCell::new(HyperHeaders::new()),
            dom_header_list: DomRefCell::new(Vec::new()),
        }
    }

    pub(crate) fn new(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Headers> {
        Self::new_with_proto(cx, global, None)
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
    ) -> DomRoot<Headers> {
        reflect_dom_object_with_proto(cx, Box::new(Headers::new_inherited()), global, proto)
    }
}

impl HeadersMethods<crate::DomTypeHolder> for Headers {
    /// <https://fetch.spec.whatwg.org/#dom-headers>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        init: Option<HeadersInit>,
    ) -> Fallible<DomRoot<Headers>> {
        let dom_headers_new = Headers::new_with_proto(cx, global, proto);
        dom_headers_new.fill(init.as_ref())?;
        Ok(dom_headers_new)
    }

    /// <https://fetch.spec.whatwg.org/#concept-headers-append>
    fn Append(&self, name: ByteString, value: ByteString) -> ErrorResult {
        self.append(name, &value)
    }

    /// <https://fetch.spec.whatwg.org/#dom-headers-delete>
    fn Delete(&self, name: ByteString) -> ErrorResult {
        // Step 1 If validating (name, ``) for this returns false, then return.
        let Some(mut valid_name) = self.validate_name_and_value(name, b"")? else {
            return Ok(());
        };

        // Validated tokens are always ASCII.
        valid_name.make_ascii_lowercase();

        // Step 2 If this’s guard is "request-no-cors", name is not a no-CORS-safelisted request-header name,
        // and name is not a privileged no-CORS request-header name, then return.
        if self.guard.get() == Guard::RequestNoCors &&
            !is_cors_safelisted_request_header(&valid_name, &b"invalid".to_vec())
        {
            return Ok(());
        }

        // 3. If this’s header list does not contain name, then return.
        // 4. Delete name from this’s header list.
        self.dom_header_list
            .borrow_mut()
            .retain(|(name, _)| name != &valid_name);
        self.header_list.borrow_mut().remove(valid_name);

        // 5. If this’s guard is "request-no-cors", then remove privileged no-CORS request-headers from this.
        if self.guard.get() == Guard::RequestNoCors {
            self.remove_privileged_no_cors_request_headers();
        }

        Ok(())
    }

    /// <https://fetch.spec.whatwg.org/#dom-headers-get>
    fn Get(&self, name: ByteString) -> Fallible<Option<ByteString>> {
        // 1. If name is not a header name, then throw a TypeError.
        let valid_name = validate_name(name)?.to_ascii_lowercase();

        // 2. Return the result of getting name from this’s header list.
        Ok(
            get_value_from_dom_header_list(&valid_name, &self.dom_header_list.borrow())
                .map(ByteString::new),
        )
    }

    /// <https://fetch.spec.whatwg.org/#dom-headers-getsetcookie>
    fn GetSetCookie(&self) -> Vec<ByteString> {
        // 1. If this’s header list does not contain `Set-Cookie`, then return « ».
        // 2. Return the values of all headers in this’s header list whose name is a
        // byte-case-insensitive match for `Set-Cookie`, in order.
        self.dom_header_list
            .borrow()
            .iter()
            .filter(|(name, _)| name == "set-cookie")
            .map(|(_, value)| ByteString::new(value.clone()))
            .collect()
    }

    /// <https://fetch.spec.whatwg.org/#dom-headers-has>
    fn Has(&self, name: ByteString) -> Fallible<bool> {
        // 1. If name is not a header name, then throw a TypeError.
        let valid_name = validate_name(name)?.to_ascii_lowercase();
        // 2. Return true if this’s header list contains name; otherwise false.
        Ok(self
            .dom_header_list
            .borrow()
            .iter()
            .any(|(name, _)| name == &valid_name))
    }

    /// <https://fetch.spec.whatwg.org/#dom-headers-set>
    fn Set(&self, name: ByteString, value: ByteString) -> Fallible<()> {
        // 1. Normalize value
        let value = trim_http_whitespace(&value);

        // 2. If validating (name, value) for this returns false, then return.
        let Some(mut valid_name) = self.validate_name_and_value(name, value)? else {
            return Ok(());
        };
        // Validated tokens are always ASCII.
        valid_name.make_ascii_lowercase();

        // 3. If this’s guard is "request-no-cors" and (name, value) is not a
        // no-CORS-safelisted request-header, then return.
        if self.guard.get() == Guard::RequestNoCors &&
            !is_cors_safelisted_request_header(&valid_name, &value)
        {
            return Ok(());
        }

        // 4. Set (name, value) in this’s header list.
        // https://fetch.spec.whatwg.org/#concept-header-list-set
        let mut dom_headers = self.dom_header_list.borrow_mut();
        if let Some(first) = dom_headers.iter().position(|(name, _)| name == &valid_name) {
            dom_headers[first].1 = value.to_vec();
            let mut index = first + 1;
            while index < dom_headers.len() {
                if dom_headers[index].0 == valid_name {
                    dom_headers.remove(index);
                } else {
                    index += 1;
                }
            }
        } else {
            dom_headers.push((valid_name.clone(), value.to_vec()));
        }
        drop(dom_headers);

        if let Ok(name) = HeaderName::from_str(&valid_name) {
            let mut headers = self.header_list.borrow_mut();
            if let Ok(value) = HeaderValue::from_bytes(value) {
                headers.insert(name, value);
            } else {
                headers.remove(name);
            }
        }

        // 5. If this’s guard is "request-no-cors", then remove privileged no-CORS request-headers from this.
        if self.guard.get() == Guard::RequestNoCors {
            self.remove_privileged_no_cors_request_headers();
        }

        Ok(())
    }
}

impl Headers {
    pub(crate) fn copy_from_headers(&self, headers: &Headers) -> ErrorResult {
        for (name, value) in headers.dom_header_list.borrow().iter() {
            self.Append(
                ByteString::new(name.as_bytes().to_vec()),
                ByteString::new(value.clone()),
            )?;
        }
        Ok(())
    }

    fn append(&self, name: ByteString, value: &ByteString) -> ErrorResult {
        // 1. Normalize value.
        let value = trim_http_whitespace(value);

        // 2. If validating (name, value) for headers returns false, then return.
        let Some(mut valid_name) = self.validate_name_and_value(name, value)? else {
            return Ok(());
        };

        // Validated tokens are always ASCII.
        valid_name.make_ascii_lowercase();

        // 3. If headers’s guard is "request-no-cors":
        if self.guard.get() == Guard::RequestNoCors {
            // 3.1. Let temporaryValue be the result of getting name from headers’s header list.
            let tmp_value = if let Some(mut tmp_value) =
                get_value_from_dom_header_list(&valid_name, &self.dom_header_list.borrow())
            {
                // 3.3. Otherwise, set temporaryValue to temporaryValue, followed by 0x2C 0x20, followed by value.
                tmp_value.extend(b", ");
                tmp_value.extend(value);
                tmp_value
            } else {
                // 3.2. If temporaryValue is null, then set temporaryValue to value.
                value.to_vec()
            };
            // 3.4. If (name, temporaryValue) is not a no-CORS-safelisted request-header, then return.
            if !is_cors_safelisted_request_header(&valid_name, &tmp_value) {
                return Ok(());
            }
        }

        // 4. Append (name, value) to headers’s header list.
        self.dom_header_list
            .borrow_mut()
            .push((valid_name.clone(), value.to_vec()));
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_str(&valid_name),
            HeaderValue::from_bytes(value),
        ) {
            self.header_list.borrow_mut().append(name, value);
        }

        // 5. If headers’s guard is "request-no-cors", then remove privileged no-CORS request-headers from headers.
        if self.guard.get() == Guard::RequestNoCors {
            self.remove_privileged_no_cors_request_headers();
        }

        Ok(())
    }

    /// <https://fetch.spec.whatwg.org/#concept-headers-fill>
    pub(crate) fn fill(&self, filler: Option<&HeadersInit>) -> ErrorResult {
        match filler {
            Some(HeadersInit::ByteStringSequenceSequence(v)) => {
                for seq in v {
                    if seq.len() == 2 {
                        self.append(seq[0].clone(), &seq[1])?;
                    } else {
                        return Err(Error::Type(cformat!(
                            "Each header object must be a sequence of length 2 - found one with length {}",
                            seq.len()
                        )));
                    }
                }
                Ok(())
            },
            Some(HeadersInit::ByteStringByteStringRecord(m)) => {
                for (key, value) in m.iter() {
                    self.append(key.clone(), value)?;
                }
                Ok(())
            },
            None => Ok(()),
        }
    }

    pub(crate) fn for_request(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Headers> {
        let headers_for_request = Headers::new(cx, global);
        headers_for_request.guard.set(Guard::Request);
        headers_for_request
    }

    pub(crate) fn for_response(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Headers> {
        let headers_for_response = Headers::new(cx, global);
        headers_for_response.guard.set(Guard::Response);
        headers_for_response
    }

    pub(crate) fn set_guard(&self, new_guard: Guard) {
        self.guard.set(new_guard)
    }

    pub(crate) fn get_guard(&self) -> Guard {
        self.guard.get()
    }

    pub(crate) fn set_headers(&self, hyper_headers: HyperHeaders) {
        *self.dom_header_list.borrow_mut() = hyper_headers
            .iter()
            .map(|(name, value)| (name.as_str().to_owned(), value.as_bytes().to_vec()))
            .collect();
        *self.header_list.borrow_mut() = hyper_headers;
    }

    pub(crate) fn get_headers_list(&self) -> HyperHeaders {
        self.header_list.borrow_mut().clone()
    }

    /// <https://fetch.spec.whatwg.org/#concept-header-extract-mime-type>
    pub(crate) fn extract_mime_type(&self) -> Vec<u8> {
        get_value_from_dom_header_list("content-type", &self.dom_header_list.borrow())
            .map(get_decode_and_split_header_value)
            .and_then(|values| extract_mime_type_from_values(&values))
            .map(|mime| mime.to_string().into_bytes())
            .unwrap_or_default()
    }

    /// <https://fetch.spec.whatwg.org/#concept-header-list-sort-and-combine>
    pub(crate) fn sort_and_combine(&self) -> Vec<(String, Vec<u8>)> {
        let mut sorted_headers = self.dom_header_list.borrow().clone();
        sorted_headers.sort_by(|a, b| a.0.cmp(&b.0));
        let mut header_vec = vec![];
        for (name, value) in sorted_headers {
            if name == "set-cookie" ||
                header_vec
                    .last()
                    .is_none_or(|(last_name, _)| last_name != &name)
            {
                header_vec.push((name, value));
            } else if let Some((_, combined_value)) = header_vec.last_mut() {
                combined_value.extend_from_slice(b", ");
                combined_value.extend(value);
            }
        }
        header_vec
    }

    /// <https://fetch.spec.whatwg.org/#ref-for-privileged-no-cors-request-header-name>
    pub(crate) fn remove_privileged_no_cors_request_headers(&self) {
        // <https://fetch.spec.whatwg.org/#privileged-no-cors-request-header-name>
        self.dom_header_list
            .borrow_mut()
            .retain(|(name, _)| name != "range");
        self.header_list.borrow_mut().remove("range");
    }

    /// <https://fetch.spec.whatwg.org/#headers-validate>
    pub(crate) fn validate_name_and_value(
        &self,
        name: ByteString,
        value: &[u8],
    ) -> Fallible<Option<String>> {
        // 1. If name is not a header name or value is not a header value, then throw a TypeError.
        let valid_name = validate_name(name)?;
        if !is_legal_header_value(value) {
            return Err(Error::Type(c"Header value is not valid".to_owned()));
        }
        // 2. If headers’s guard is "immutable", then throw a TypeError.
        if self.guard.get() == Guard::Immutable {
            return Err(Error::Type(c"Guard is immutable".to_owned()));
        }
        // 3. If headers’s guard is "request" and (name, value) is a forbidden request-header, then return false.
        if self.guard.get() == Guard::Request && is_forbidden_request_header(&valid_name, value) {
            return Ok(None);
        }
        // 4. If headers’s guard is "response" and name is a forbidden response-header name, then return false.
        if self.guard.get() == Guard::Response && is_forbidden_response_header(&valid_name) {
            return Ok(None);
        }

        Ok(Some(valid_name))
    }
}

impl Iterable for Headers {
    type Key = ByteString;
    type Value = ByteString;

    fn get_iterable_length(&self, _cx: &mut JSContext) -> u32 {
        let sorted_header_vec = self.sort_and_combine();
        sorted_header_vec.len() as u32
    }

    fn get_value_at_index(&self, _cx: &mut JSContext, index: u32) -> ByteString {
        let sorted_header_vec = self.sort_and_combine();
        ByteString::new(sorted_header_vec.into_iter().nth(index as usize).unwrap().1)
    }

    fn get_key_at_index(&self, _cx: &mut JSContext, index: u32) -> ByteString {
        let sorted_header_vec = self.sort_and_combine();
        ByteString::new(
            sorted_header_vec
                .into_iter()
                .nth(index as usize)
                .unwrap()
                .0
                .into_bytes(),
        )
    }
}

fn get_value_from_dom_header_list(name: &str, headers: &[(String, Vec<u8>)]) -> Option<Vec<u8>> {
    let values = headers
        .iter()
        .filter(|(header_name, _)| header_name == name)
        .map(|(_, value)| value.as_slice())
        .collect::<Vec<_>>();
    if values.is_empty() {
        None
    } else {
        Some(values.join(&b", "[..]))
    }
}

/// This function will internally convert `name` to lowercase for matching, so explicitly converting
/// before calling is not necessary
///
/// <https://fetch.spec.whatwg.org/#forbidden-request-header>
pub(crate) fn is_forbidden_request_header(name: &str, value: &[u8]) -> bool {
    let forbidden_header_names = [
        "accept-charset",
        "accept-encoding",
        "access-control-request-headers",
        "access-control-request-method",
        "connection",
        "content-length",
        "cookie",
        "cookie2",
        "date",
        "dnt",
        "expect",
        "host",
        "keep-alive",
        "origin",
        "referer",
        "set-cookie",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
        "via",
        // This list is defined in the fetch spec, however the draft spec for private-network-access
        // proposes this additional forbidden name, which is currently included in WPT tests. See:
        // https://wicg.github.io/private-network-access/#forbidden-header-names
        "access-control-request-private-network",
    ];

    // Step 1: If name is a byte-case-insensitive match for one of (forbidden_header_names), return
    // true
    if forbidden_header_names
        .into_iter()
        .any(|header| name.eq_ignore_ascii_case(header))
    {
        return true;
    }

    let forbidden_header_prefixes = ["sec-", "proxy-"];

    // Step 2: If name when byte-lowercased starts with `proxy-` or `sec-`, then return true.
    if forbidden_header_prefixes
        .into_iter()
        .any(|prefix| starts_with_ignore_ascii_case(name, prefix))
    {
        return true;
    }

    let potentially_forbidden_header_names = [
        "x-http-method",
        "x-http-method-override",
        "x-method-override",
    ];

    // Step 3: If name is a byte-case-insensitive match for one of (potentially_forbidden_header_names)
    if potentially_forbidden_header_names
        .into_iter()
        .any(|header| name.eq_ignore_ascii_case(header))
    {
        // Step 3.1: Let parsedValues be the result of getting, decoding, and splitting value.
        let parsed_values = get_decode_and_split_header_value(value.to_vec());

        // Step 3.2: For each method of parsedValues: if the isomorphic encoding of method is a
        // forbidden method, then return true.
        return parsed_values
            .iter()
            .any(|s| is_forbidden_method(s.as_bytes()));
    }

    // Step 4: Return false.
    false
}

/// <https://fetch.spec.whatwg.org/#forbidden-response-header-name>
fn is_forbidden_response_header(name: &str) -> bool {
    // A forbidden response-header name is a header name that is a byte-case-insensitive match for one of
    name.eq_ignore_ascii_case("set-cookie") || name.eq_ignore_ascii_case("set-cookie2")
}

fn validate_name(name: ByteString) -> Fallible<String> {
    if !is_field_name(&name) {
        return Err(Error::Type(c"Name is not valid".to_owned()));
    }
    match String::from_utf8(name.into()) {
        Ok(ns) => Ok(ns),
        _ => Err(Error::Type(c"Non-UTF8 header name found".to_owned())),
    }
}

/// <http://tools.ietf.org/html/rfc7230#section-3.2>
fn is_field_name(name: &ByteString) -> bool {
    is_token(name)
}

// Fetch permits any bytes except NUL, newline, and leading or trailing HTTP tab or space.
/// <https://fetch.spec.whatwg.org/#concept-header-value>
fn is_legal_header_value(value: &[u8]) -> bool {
    let value_len = value.len();
    if value_len == 0 {
        return true;
    }
    match value[0] {
        b' ' | b'\t' => return false,
        _ => {},
    };
    match value[value_len - 1] {
        b' ' | b'\t' => return false,
        _ => {},
    };
    for &ch in value {
        match ch {
            b'\0' | b'\n' | b'\r' => return false,
            _ => {},
        }
    }
    true
}

/// <https://tools.ietf.org/html/rfc5234#appendix-B.1>
pub(crate) fn is_vchar(x: u8) -> bool {
    matches!(x, 0x21..=0x7E)
}

/// <http://tools.ietf.org/html/rfc7230#section-3.2.6>
pub(crate) fn is_obs_text(x: u8) -> bool {
    matches!(x, 0x80..=0xFF)
}
