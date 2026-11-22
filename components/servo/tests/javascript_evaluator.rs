/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Javascript Evaluator API unit tests.
mod common;

use std::rc::Rc;

use http::{HeaderName, HeaderValue};
use http_body_util::combinators::BoxBody;
use hyper::body::{Bytes, Incoming};
use hyper::{Request as HyperRequest, Response as HyperResponse};
use net::test_util::{make_body, make_server};
use servo::{JSValue, JavaScriptEvaluationError, WebViewBuilder};

use crate::common::{ServoTest, WebViewDelegateImpl, evaluate_javascript};

#[test]
fn test_evaluate_javascript_basic() {
    let servo_test = ServoTest::new();
    let delegate = Rc::new(WebViewDelegateImpl::default());
    let webview = WebViewBuilder::new(servo_test.servo(), servo_test.rendering_context.clone())
        .delegate(delegate.clone())
        .build();

    let result = evaluate_javascript(&servo_test, webview.clone(), "undefined");
    assert_eq!(result, Ok(JSValue::Undefined));

    let result = evaluate_javascript(&servo_test, webview.clone(), "null");
    assert_eq!(result, Ok(JSValue::Null));

    let result = evaluate_javascript(&servo_test, webview.clone(), "42");
    assert_eq!(result, Ok(JSValue::Number(42.0)));

    let result = evaluate_javascript(&servo_test, webview.clone(), "3 + 4");
    assert_eq!(result, Ok(JSValue::Number(7.0)));

    let result = evaluate_javascript(&servo_test, webview.clone(), "'abc' + 'def'");
    assert_eq!(result, Ok(JSValue::String("abcdef".into())));

    let result = evaluate_javascript(&servo_test, webview.clone(), "let foo = {blah: 123}; foo");
    assert!(matches!(result, Ok(JSValue::Object(_))));
    if let Ok(JSValue::Object(values)) = result {
        assert_eq!(values.len(), 1);
        assert_eq!(values.get("blah"), Some(&JSValue::Number(123.0)));
    }

    let result = evaluate_javascript(&servo_test, webview.clone(), "[1, 2, 3, 4]");
    let expected = JSValue::Array(vec![
        JSValue::Number(1.0),
        JSValue::Number(2.0),
        JSValue::Number(3.0),
        JSValue::Number(4.0),
    ]);
    assert_eq!(result, Ok(expected));

    let result = evaluate_javascript(&servo_test, webview.clone(), "window");
    assert!(matches!(result, Ok(JSValue::Window(..))));

    let result = evaluate_javascript(&servo_test, webview.clone(), "document.body");
    assert!(matches!(result, Ok(JSValue::Element(..))));

    let result = evaluate_javascript(
        &servo_test,
        webview.clone(),
        "document.body.attachShadow({mode: 'open'})",
    );
    assert!(matches!(result, Ok(JSValue::ShadowRoot(..))));

    let result = evaluate_javascript(&servo_test, webview.clone(), "document.body.shadowRoot");
    assert!(matches!(result, Ok(JSValue::ShadowRoot(..))));

    let result = evaluate_javascript(
        &servo_test,
        webview.clone(),
        "document.body.innerHTML += '<iframe>'; frames[0]",
    );
    assert!(matches!(result, Ok(JSValue::Frame(..))));

    let result = evaluate_javascript(&servo_test, webview.clone(), "lettt badsyntax = 123");
    assert_eq!(result, Err(JavaScriptEvaluationError::CompilationFailure));

    let result = evaluate_javascript(&servo_test, webview.clone(), "throw new Error()");
    assert!(matches!(
        result,
        Err(JavaScriptEvaluationError::EvaluationFailure(_))
    ));
}

#[test]
fn test_evaluate_javascript_panic() {
    let servo_test = ServoTest::new();
    let delegate = Rc::new(WebViewDelegateImpl::default());
    let webview = WebViewBuilder::new(servo_test.servo(), servo_test.rendering_context.clone())
        .delegate(delegate.clone())
        .build();

    let input = "location";
    let result = evaluate_javascript(&servo_test, webview.clone(), input);
    assert!(matches!(result, Ok(JSValue::Object(..))));
}

#[test]
fn runs_in_csp_restricted() {
    let servo_test = ServoTest::new();

    static MESSAGE: &'static [u8] = b"<!DOCTYPE html><p id=\"para\">Some text here</p>";
    let handler =
        move |_: HyperRequest<Incoming>,
              response: &mut HyperResponse<BoxBody<Bytes, hyper::Error>>| {
            println!("Request");
            response.headers_mut().insert(
                HeaderName::from_static("content-security-policy"),
                HeaderValue::from_static("sandbox"),
            );
            *response.body_mut() = make_body(MESSAGE.to_vec());
        };
    let (server, url) = make_server(handler);

    let delegate = Rc::new(WebViewDelegateImpl::default());
    let webview = WebViewBuilder::new(servo_test.servo(), servo_test.rendering_context.clone())
        .delegate(delegate.clone())
        .url(url.url().into_url())
        .build();
    servo_test.spin(|| webview.url() != Some(url.url().into_url()));
    server.close();

    let result = evaluate_javascript(&servo_test, webview.clone(), "document.readyState");
    assert_eq!(result, Ok(JSValue::String("complete".into())));
}

#[test]
fn document_scoped_evaluation_targets_background_view_and_rejects_stale_edits() {
    use std::cell::RefCell;

    use servo::LoadStatus;
    use url::Url;

    let servo_test = ServoTest::new();
    let initial = Url::parse("data:text/html,<h1>Before</h1>").unwrap();
    let destination = Url::parse("data:text/html,<h1>Next page</h1>").unwrap();
    let background = WebViewBuilder::new(servo_test.servo(), servo_test.rendering_context.clone())
        .url(initial.clone())
        .build();
    let foreground =
        WebViewBuilder::new(servo_test.servo(), servo_test.rendering_context.clone()).build();
    background.hide();
    background.set_focused(false);
    foreground.show();

    let wait_for_load = |url: &Url| {
        servo_test.spin(|| {
            background.url().as_ref() != Some(url) ||
                background.load_status() != LoadStatus::Complete
        });
    };
    let evaluate = |expected, script: &str| {
        let result = Rc::new(RefCell::new(None));
        let output = result.clone();
        background.evaluate_javascript_in_document(expected, script.to_owned(), move |reply| {
            output.replace(Some(reply));
        });
        servo_test.spin(|| result.borrow().is_none());
        result.take().expect("evaluation reply")
    };

    wait_for_load(&initial);
    let read_heading = "document.querySelector('h1').textContent";
    let JSValue::Object(result) = evaluate(None, read_heading).unwrap() else {
        panic!("expected a document ID and value");
    };
    assert_eq!(result["value"], JSValue::String("Before".into()));
    let JSValue::String(document) = &result["document"] else {
        panic!("expected a decimal document ID");
    };
    let document = Some(document.parse().unwrap());

    let edit_heading = "document.querySelector('h1').textContent = 'After'";
    evaluate(document, edit_heading).unwrap();
    let JSValue::Object(result) = evaluate(document, read_heading).unwrap() else {
        panic!("expected a document ID and value");
    };
    assert_eq!(result["value"], JSValue::String("After".into()));

    background.load(destination.clone());
    wait_for_load(&destination);
    assert_eq!(
        evaluate(document, edit_heading),
        Err(JavaScriptEvaluationError::DocumentNotFound),
    );
    let JSValue::Object(result) = evaluate(None, read_heading).unwrap() else {
        panic!("expected a document ID and value");
    };
    assert_eq!(result["value"], JSValue::String("Next page".into()));
}
