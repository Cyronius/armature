/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::rc::Rc;

use serde::Serialize;
use serde_json::Value;
use url::Url;

use super::chrome::web_url;
use crate::running_app_state::RunningAppState;
use crate::window::{PlatformWindow, TopLevelWebViewCreationRequest};

const MAX_HISTORY_EACH_WAY: usize = 50;

#[derive(Clone, PartialEq, Serialize)]
pub(crate) struct SavedTab {
    pub(crate) url: Url,
    pub(crate) past: Vec<Url>,
    pub(crate) future: Vec<Url>,
}

fn session_url(input: &str) -> Option<Url> {
    match input {
        "servo:newtab" | "about:blank" => Url::parse(input).ok(),
        _ => web_url(input),
    }
}

impl SavedTab {
    pub(crate) fn from_history(entries: &[Url], current: usize) -> Option<Self> {
        let url = session_url(entries.get(current)?.as_str())?;
        let start = current.saturating_sub(MAX_HISTORY_EACH_WAY);
        let past = entries[start..current]
            .iter()
            .filter_map(|url| session_url(url.as_str()))
            .collect();
        let future = entries[current + 1..]
            .iter()
            .take(MAX_HISTORY_EACH_WAY)
            .filter_map(|url| session_url(url.as_str()))
            .collect();
        Some(Self { url, past, future })
    }

    fn read(value: &Value) -> Option<Self> {
        // Earlier profiles stored only the current URL of each tab.
        let url = session_url(value.as_str().or_else(|| value["url"].as_str())?)?;
        let urls = |key: &str| {
            let items = value
                .get(key)
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let start = if key == "past" {
                items.len().saturating_sub(MAX_HISTORY_EACH_WAY)
            } else {
                0
            };
            items[start..]
                .iter()
                .take(MAX_HISTORY_EACH_WAY)
                .filter_map(|item| session_url(item.as_str()?))
                .collect()
        };
        Some(Self {
            url,
            past: urls("past"),
            future: urls("future"),
        })
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub(crate) struct SavedWindow {
    pub(crate) tabs: Vec<Rc<SavedTab>>,
    pub(crate) selected: usize,
}

impl SavedWindow {
    fn read(items: &[Value], selected: usize) -> Option<Self> {
        let mut tabs = Vec::new();
        let mut active = 0;
        for (index, item) in items.iter().enumerate() {
            if let Some(tab) = SavedTab::read(item) {
                if index <= selected {
                    active = tabs.len();
                }
                tabs.push(Rc::new(tab));
            }
        }
        (!tabs.is_empty()).then_some(Self {
            tabs,
            selected: active,
        })
    }

    pub(crate) fn read_session(value: &Value) -> Vec<Self> {
        let Some(items) = value.as_array() else {
            return Vec::new();
        };
        if items.iter().any(Value::is_string) {
            return Self::read(items, 0).into_iter().collect();
        }
        items
            .iter()
            .filter_map(|window| {
                Self::read(
                    window["tabs"].as_array()?,
                    window["selected"].as_u64().unwrap_or(0) as usize,
                )
            })
            .collect()
    }

    pub(crate) fn open(self, state: &Rc<RunningAppState>, platform: Rc<dyn PlatformWindow>) {
        let mut tabs = self.tabs.into_iter();
        let Some(first) = tabs.next() else { return };
        let window = state.open_window(platform, TopLevelWebViewCreationRequest::Restored(first));
        for tab in tabs {
            window.create_toplevel_webview(
                state.clone(),
                TopLevelWebViewCreationRequest::Restored(tab),
            );
        }
        window.activate_webview_by_index(self.selected);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_legacy_urls_and_preserves_selection_after_filtering() {
        let old = SavedWindow::read_session(&json!([
            "https://user:secret@example.org/a",
            "file:///private"
        ]));
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].tabs.len(), 1);
        assert_eq!(old[0].tabs[0].url.as_str(), "https://example.org/a");

        let windows = SavedWindow::read_session(&json!([
            {"tabs":[{"url":"file:///private"},{"url":"servo:newtab"},
                {"url":"https://example.org/b","past":["https://example.org/a"],
                 "future":["data:text/plain,private","https://example.org/c"]}], "selected":2},
            {"tabs":[{"url":"about:blank"}],"selected":0}
        ]));
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].selected, 1);
        let active = &windows[0].tabs[1];
        assert_eq!(active.past[0].path(), "/a");
        assert_eq!(active.future.len(), 1);
        assert_eq!(active.future[0].path(), "/c");
        let round_trip = SavedWindow::read_session(&json!(windows));
        assert!(windows == round_trip);
    }
}
