/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::mpsc::{self, Receiver, Sender};

use serde_json::{Value, json};
use servo::{
    ConsoleLogLevel, Cursor, EmbedderControl, EmbedderControlId, JSValue, LoadStatus,
    NavigationRequest, OffscreenRenderingContext, WebView, WebViewBuilder, WebViewDelegate,
};
use url::Url;
use winit::event_loop::EventLoopProxy;

use super::event_loop::AppEvent;
use super::session::{SavedTab, SavedWindow};
use crate::running_app_state::{RunningAppState, UserInterfaceCommand};
use crate::window::ServoShellWindow;

pub(crate) const HEIGHT: f32 = 78.0;
pub(crate) const SIDEBAR_WIDTH: f32 = 190.0;
const PREFIX: &str = "armature-chrome:";

#[derive(Default)]
struct Messages {
    requests: RefCell<Vec<Value>>,
    replies: RefCell<Vec<Value>>,
}

struct ChromeDelegate {
    window: Weak<ServoShellWindow>,
    messages: Rc<Messages>,
}

impl WebViewDelegate for ChromeDelegate {
    fn request_navigation(&self, _: WebView, request: NavigationRequest) {
        request.deny();
    }

    fn notify_new_frame_ready(&self, _: WebView) {
        if let Some(window) = self.window.upgrade() {
            window.set_needs_repaint();
        }
    }

    fn notify_cursor_changed(&self, view: WebView, cursor: Cursor) {
        if let Some(window) = self.window.upgrade() &&
            let Some(headed) = window.platform_window().as_headed_window()
        {
            headed.chrome_cursor_changed(view.id(), cursor);
        }
    }

    fn notify_accessibility_tree_update(&self, view: WebView, update: accesskit::TreeUpdate) {
        if let Some(window) = self.window.upgrade() {
            window
                .platform_window()
                .notify_accessibility_tree_update(view, update);
        }
    }

    fn show_embedder_control(&self, view: WebView, control: EmbedderControl) {
        if let Some(window) = self.window.upgrade() {
            window
                .platform_window()
                .show_embedder_control(view.id(), control);
            window.set_needs_repaint();
        }
    }

    fn hide_embedder_control(&self, view: WebView, id: EmbedderControlId) {
        if let Some(window) = self.window.upgrade() {
            window
                .platform_window()
                .hide_embedder_control(view.id(), id);
            window.set_needs_repaint();
        }
    }

    fn show_console_message(&self, _: WebView, _: ConsoleLogLevel, message: String) {
        // Only the bundled, opaque-origin chrome WebView has this delegate.
        if let Some(body) = message.strip_prefix(PREFIX) &&
            let Ok(request) = serde_json::from_str(body)
        {
            self.messages.requests.borrow_mut().push(request);
        }
    }
}

#[derive(Default)]
pub(crate) struct BrowserData {
    history: Vec<Value>,
    bookmarks: Vec<Value>,
    pub(crate) previous: Vec<SavedWindow>,
    tabs_left: bool,
    sessions: Vec<SavedWindow>,
    pub(crate) tab_history: HashMap<servo::WebViewId, Option<Rc<SavedTab>>>,
    writer: Option<std::thread::JoinHandle<()>>,
    persist: Option<Sender<Value>>,
}

impl BrowserData {
    pub(crate) fn new() -> Self {
        let path = (!servo::opts::get().temporary_storage)
            .then(|| servo::opts::get().config_dir.as_ref())
            .flatten()
            .map(|dir| dir.join("armature-chrome.json"));
        let saved: Value = path
            .as_ref()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let previous = SavedWindow::read_session(&saved["session"]);
        let mut writer = None;
        let persist = path.and_then(|path| {
            let (sender, receiver) = mpsc::channel::<Value>();
            writer = std::thread::Builder::new()
                .name("Armature profile".into())
                .spawn(move || {
                    for value in receiver {
                        let result = (|| -> std::io::Result<()> {
                            let mut file =
                                tempfile::NamedTempFile::new_in(path.parent().unwrap_or(&path))?;
                            serde_json::to_writer(&mut file, &value)?;
                            file.persist(&path)?;
                            Ok(())
                        })();
                        if let Err(error) = result {
                            log::warn!("Could not save Armature browser state: {error}");
                        }
                    }
                })
                .ok();
            writer.as_ref().map(|_| sender)
        });
        Self {
            history: saved["history"].as_array().cloned().unwrap_or_default(),
            bookmarks: saved["bookmarks"].as_array().cloned().unwrap_or_default(),
            sessions: previous.clone(),
            previous,
            tabs_left: saved["tabsLeft"] == true,
            tab_history: HashMap::new(),
            writer,
            persist,
        }
    }

    fn save(&self) {
        if let Some(sender) = &self.persist {
            let _ = sender.send(json!({"history":self.history,"bookmarks":self.bookmarks,"session":self.sessions,"tabsLeft":self.tabs_left}));
        }
    }

    pub(crate) fn snapshot(&mut self, windows: &[Rc<ServoShellWindow>], exiting: bool) {
        let closing_all = windows.iter().all(|window| window.should_close());
        let mut session = Vec::new();
        let mut live = std::collections::HashSet::new();
        for window in windows {
            if window.should_close() && !exiting && !closing_all {
                continue;
            }
            let mut tabs = Vec::new();
            let mut selected = 0;
            let active = window.active_webview().map(|view| view.id());
            for id in window.webview_ids() {
                live.insert(id);
                let Some(entry) = self.tab_history.get(&id) else {
                    // Keep the previous complete snapshot while the first navigation commits.
                    return;
                };
                if let Some(tab) = entry {
                    if Some(id) == active {
                        selected = tabs.len();
                    }
                    tabs.push(tab.clone());
                }
            }
            if !tabs.is_empty() {
                session.push(SavedWindow { tabs, selected });
            }
        }
        self.tab_history.retain(|id, _| live.contains(id));
        if self.sessions != session {
            self.sessions = session;
            self.save();
        }
    }

    pub(crate) fn finish_writes(&mut self) {
        self.persist.take();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}

pub(super) fn web_url(input: &str) -> Option<Url> {
    let mut url = Url::parse(input).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    // A URL stored in history or recovery must not retain embedded credentials.
    url.set_username("").ok()?;
    url.set_password(None).ok()?;
    Some(url)
}

pub(crate) struct Chrome {
    pub(crate) view: WebView,
    pub(crate) context: Rc<OffscreenRenderingContext>,
    messages: Rc<Messages>,
    ready: Cell<bool>,
    pub(crate) address_focus_pending: Cell<bool>,
    last_state: RefCell<Value>,
    last_browser: RefCell<Value>,
    visits: RefCell<HashMap<String, String>>,
    recently_closed: RefCell<Vec<Url>>,
    pub(crate) panel_height: Cell<f32>,
    pub(crate) tabs_left: Cell<bool>,
    transfers: RefCell<Vec<Value>>,
    transfer_events: Receiver<(usize, String, bool)>,
    transfer_sender: Sender<(usize, String, bool)>,
    proxy: EventLoopProxy<AppEvent>,
}

impl Chrome {
    pub(crate) fn new(
        state: &RunningAppState,
        window: &Rc<ServoShellWindow>,
        context: Rc<OffscreenRenderingContext>,
        proxy: EventLoopProxy<AppEvent>,
    ) -> Self {
        // Percent encode the bundled document without form encoding's special handling of spaces.
        use std::fmt::Write;
        let mut data = String::from("data:text/html;charset=utf-8,");
        for byte in include_bytes!("chrome.html") {
            write!(data, "%{byte:02X}").expect("writing to a String");
        }
        let messages = Rc::new(Messages::default());
        let view = WebViewBuilder::new(state.servo(), context.clone())
            .url(Url::parse(&data).expect("valid chrome data URL"))
            .hidpi_scale_factor(window.platform_window().hidpi_scale_factor())
            .delegate(Rc::new(ChromeDelegate {
                window: Rc::downgrade(window),
                messages: messages.clone(),
            }))
            .build();
        view.show();
        let (transfer_sender, transfer_events) = mpsc::channel();
        Self {
            view,
            context,
            messages,
            ready: Cell::new(false),
            address_focus_pending: Cell::new(false),
            last_state: RefCell::new(Value::Null),
            last_browser: RefCell::new(Value::Null),
            visits: RefCell::new(HashMap::new()),
            recently_closed: RefCell::new(Vec::new()),
            panel_height: Cell::new(0.0),
            tabs_left: Cell::new(state.browser_data.borrow().tabs_left),
            transfers: RefCell::new(Vec::new()),
            transfer_events,
            transfer_sender,
            proxy,
        }
    }

    pub(crate) fn tick(&self, window: &Rc<ServoShellWindow>, state: &Rc<RunningAppState>) {
        for request in self.messages.requests.take() {
            self.handle_request(window, state, request);
        }
        // The inline script can announce readiness before the constellation activates its
        // pipeline. Wait until the WebView has finished loading before evaluating a reply.
        if !self.ready.get() || self.view.load_status() != LoadStatus::Complete {
            return;
        }
        let active = window.active_webview().map(|view| view.id());
        let tabs: Vec<_> = window.webviews().into_iter().map(|(id, view)| {
            json!({
                "id": id.to_string(),
                "active": Some(id) == active,
                "url": view.url().map(|url| url.to_string()),
                "title": view.page_title(),
                "loading": matches!(view.load_status(), LoadStatus::Started | LoadStatus::HeadParsed),
                "back": view.can_go_back(),
                "forward": view.can_go_forward(),
            })
        }).collect();
        for (index, status, active) in self.transfer_events.try_iter() {
            if let Some(transfer) = self.transfers.borrow_mut().get_mut(index) {
                transfer["status"] = json!(status);
                transfer["active"] = json!(active);
            }
        }
        let current = json!(tabs);
        if *self.last_state.borrow() != current {
            self.deliver("updateChrome", &current);
            self.last_state.replace(current.clone());
        }
        if self.address_focus_pending.take() {
            self.focus_address();
        }
        self.update_browser(window, state, &current);
        for reply in self.messages.replies.take() {
            self.deliver("finishChromeRequest", &reply);
        }
    }

    fn deliver(&self, receiver: &'static str, message: &Value) {
        let encoded = json!(message.to_string());
        self.view.evaluate_javascript(
            format!("window.{receiver}(JSON.parse({encoded}))"),
            move |result| {
                if let Err(error) = result {
                    log::warn!("Could not deliver {receiver} to web chrome: {error:?}");
                }
            },
        );
    }

    fn update_browser(&self, window: &ServoShellWindow, state: &RunningAppState, tabs: &Value) {
        let mut data = state.browser_data.borrow_mut();
        if self.tabs_left.replace(data.tabs_left) != data.tabs_left {
            window.set_needs_repaint();
        }
        let mut visits = self.visits.borrow_mut();
        let mut changed = false;
        let tabs = tabs.as_array().map(Vec::as_slice).unwrap_or_default();
        visits.retain(|id, _| tabs.iter().any(|tab| tab["id"].as_str() == Some(id)));
        for tab in tabs {
            let (Some(id), Some(url)) = (tab["id"].as_str(), tab["url"].as_str().and_then(web_url))
            else {
                continue;
            };
            if tab["loading"] == true {
                continue;
            }
            let url = url.to_string();
            if visits.get(id) != Some(&url) {
                visits.insert(id.to_owned(), url.clone());
                data.history.retain(|entry| entry["url"] != url);
                data.history
                    .insert(0, json!({"url":url,"title":tab["title"]}));
                data.history.truncate(100);
                changed = true;
            }
        }
        if changed {
            data.save();
        }
        let url = window
            .active_webview()
            .and_then(|view| view.url())
            .and_then(|url| web_url(url.as_str()));
        let current = json!({
            "history":data.history, "bookmarks":data.bookmarks,
            "origin":url.as_ref().map(|url|url.origin().ascii_serialization()),
            "scheme":url.as_ref().map(Url::scheme),
            "bookmarked":url.is_some_and(|url| data.bookmarks.iter().any(|item|item["url"]==url.as_str())),
            "downloads":*self.transfers.borrow(),
            "tabsLeft":data.tabs_left, "canRestore":!data.previous.is_empty(), "canReopen":!self.recently_closed.borrow().is_empty(),
        });
        if *self.last_browser.borrow() != current {
            self.deliver("updateBrowser", &current);
            self.last_browser.replace(current);
        }
    }

    pub(crate) fn record_closed(&self, view: &WebView) {
        if let Some(url) = view.url().and_then(|url| web_url(url.as_str())) {
            let mut closed = self.recently_closed.borrow_mut();
            if closed.len() == 20 {
                closed.remove(0);
            }
            closed.push(url);
        }
    }

    pub(crate) fn reopen_tab(&self, window: &Rc<ServoShellWindow>, state: &Rc<RunningAppState>) {
        if let Some(url) = self.recently_closed.borrow_mut().pop() {
            Self::open_tab(window, state, url);
        }
    }

    fn open_tab(window: &Rc<ServoShellWindow>, state: &Rc<RunningAppState>, url: Url) {
        window.create_and_activate_toplevel_webview(
            state.clone(),
            crate::window::TopLevelWebViewCreationRequest::WithUrl(url),
        );
    }

    pub(crate) fn open_panel(&self, name: &str) {
        self.view
            .evaluate_javascript(format!("window.openBrowserPanel({})", json!(name)), |_| {});
    }

    fn download(&self, state: &RunningAppState, input: &str) {
        let (Some(url), Some(directory)) = (web_url(input), dirs::download_dir()) else {
            return;
        };
        let file_name = url
            .path_segments()
            .and_then(|parts| parts.last())
            .filter(|name| !name.is_empty() && !matches!(*name, "." | ".."))
            .unwrap_or("download")
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .take(120)
            .collect::<String>();
        let index = self.transfers.borrow().len();
        self.transfers.borrow_mut().push(json!({
            "name": file_name, "status":"Starting", "active":true, "url":url.as_str(),
        }));
        let sender = self.transfer_sender.clone();
        let proxy = self.proxy.clone();
        let (tx, rx) = mpsc::channel::<servo::DownloadEvent>();
        let worker = std::thread::Builder::new()
            .name("Armature download".into())
            .spawn(move || {
                let notify = |status, active| {
                    let _ = sender.send((index, status, active));
                    let _ = proxy.send_event(AppEvent::Waker(None));
                };
                let result = (|| -> Result<PathBuf, String> {
                    let mut file =
                        tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
                    let mut bytes = 0usize;
                    let mut last_update = std::time::Instant::now();
                    loop {
                        match rx.recv().map_err(|_| "Transfer interrupted")? {
                            servo::DownloadEvent::Data(chunk) => {
                                file.write_all(&chunk).map_err(|e| e.to_string())?;
                                bytes += chunk.len();
                                if last_update.elapsed() >= std::time::Duration::from_millis(150) {
                                    notify(
                                        format!("{:.1} MB received", bytes as f64 / 1_000_000.0),
                                        true,
                                    );
                                    last_update = std::time::Instant::now();
                                }
                            },
                            servo::DownloadEvent::Finished(result) => {
                                result?;
                                break;
                            },
                        }
                    }
                    for suffix in 0..1000 {
                        let name = if suffix == 0 {
                            file_name.clone()
                        } else {
                            format!("{suffix}-{file_name}")
                        };
                        let path = directory.join(name);
                        match file.persist_noclobber(&path) {
                            Ok(_) => return Ok(path),
                            Err(error)
                                if error.error.kind() == std::io::ErrorKind::AlreadyExists =>
                            {
                                file = error.file
                            },
                            Err(error) => return Err(error.error.to_string()),
                        }
                    }
                    Err("No unused filename".to_owned())
                })();
                notify(
                    match result {
                        Ok(path) => format!("Saved to {}", path.display()),
                        Err(error) => format!("Failed: {error}"),
                    },
                    false,
                );
            });
        if let Err(error) = worker {
            self.transfers.borrow_mut()[index]["status"] = json!(format!("Failed: {error}"));
            self.transfers.borrow_mut()[index]["active"] = json!(false);
            return;
        }
        state.servo().network_manager().download(url, move |event| {
            let _ = tx.send(event);
        });
    }

    fn save_page(&self, view: &WebView) {
        let Some(directory) = dirs::download_dir() else {
            return;
        };
        let name = view
            .page_title()
            .unwrap_or_else(|| "Armature page".into())
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
            .take(80)
            .collect::<String>();
        let name = if name.trim().is_empty() {
            "Armature page".to_owned()
        } else {
            name
        };
        let index = self.transfers.borrow().len();
        self.transfers
            .borrow_mut()
            .push(json!({"name":format!("{name}.html"),"status":"Saving","active":true}));
        let sender = self.transfer_sender.clone();
        let proxy = self.proxy.clone();
        view.evaluate_javascript(
            "new XMLSerializer().serializeToString(document)",
            move |result| {
                let finish = move |status| {
                    let _ = sender.send((index, status, false));
                    let _ = proxy.send_event(AppEvent::Waker(None));
                };
                let Ok(JSValue::String(html)) = result else {
                    finish("Could not read the page".to_owned());
                    return;
                };
                std::thread::spawn(move || {
                    let result = (|| -> std::io::Result<PathBuf> {
                        for suffix in 0..1000 {
                            let file_name = if suffix == 0 {
                                format!("{name}.html")
                            } else {
                                format!("{name} ({suffix}).html")
                            };
                            let path = directory.join(file_name);
                            match OpenOptions::new().write(true).create_new(true).open(&path) {
                                Ok(mut file) => {
                                    if let Err(error) = file.write_all(html.as_bytes()) {
                                        drop(file);
                                        let _ = fs::remove_file(&path);
                                        return Err(error);
                                    }
                                    return Ok(path);
                                },
                                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                                },
                                Err(error) => return Err(error),
                            }
                        }
                        Err(std::io::Error::other("No unused filename"))
                    })();
                    finish(match result {
                        Ok(path) => format!("Saved to {}", path.display()),
                        Err(error) => format!("Save failed: {error}"),
                    });
                });
            },
        );
    }

    fn handle_request(
        &self,
        window: &Rc<ServoShellWindow>,
        state: &Rc<RunningAppState>,
        request: Value,
    ) {
        match request["op"].as_str() {
            Some("ready") => self.ready.set(true),
            Some("navigate") => {
                if let Some(url) = request["url"].as_str() {
                    focus_page(window);
                    window.queue_user_interface_command(UserInterfaceCommand::Go(url.to_owned()));
                }
            },
            Some("panel") => {
                let height = request["height"]
                    .as_f64()
                    .unwrap_or_default()
                    .clamp(0.0, 172.0) as f32;
                if self.panel_height.replace(height) != height {
                    window.set_needs_repaint();
                }
            },
            Some("tabPlacement") => {
                let mut data = state.browser_data.borrow_mut();
                data.tabs_left = request["left"] == true;
                data.save();
            },
            Some("fullscreen") => {
                if let Some(headed) = window.platform_window().as_headed_window() {
                    headed.toggle_browser_fullscreen();
                }
            },
            Some("bookmark") => {
                if let Some(view) = window.active_webview() &&
                    let Some(url) = view.url().and_then(|url| web_url(url.as_str()))
                {
                    let mut data = state.browser_data.borrow_mut();
                    if let Some(index) = data
                        .bookmarks
                        .iter()
                        .position(|item| item["url"] == url.as_str())
                    {
                        data.bookmarks.remove(index);
                    } else {
                        data.bookmarks
                            .insert(0, json!({"url":url.as_str(),"title":view.page_title()}));
                    }
                    data.save();
                }
            },
            Some("reopenTab") => self.reopen_tab(window, state),
            Some("restoreTabs") => {
                window.queue_user_interface_command(UserInterfaceCommand::RestoreSession)
            },
            Some("focusPage") => focus_page(window),
            Some("download") => {
                if let Some(url) = request["url"].as_str() {
                    self.download(state, url);
                }
            },
            Some("savePage") => {
                if let Some(view) = window.active_webview() {
                    self.save_page(&view);
                }
            },
            Some("find") => {
                if let Some(view) = window.active_webview() &&
                    let Some(text) = request["text"].as_str()
                {
                    view.evaluate_javascript(
                        format!("({FIND})({}, {})", json!(text), request["backward"] == true),
                        |_| {},
                    );
                }
            },
            Some("newTab") => window.queue_user_interface_command(UserInterfaceCommand::NewWebView),
            Some("back") => window.queue_user_interface_command(UserInterfaceCommand::Back),
            Some("forward") => window.queue_user_interface_command(UserInterfaceCommand::Forward),
            Some("reload") => window.queue_user_interface_command(UserInterfaceCommand::Reload),
            Some("selectTab" | "closeTab") => {
                if let Some(view) = target_view(window, &request["tab"]) {
                    if request["op"] == "selectTab" {
                        window.activate_webview(view.id());
                        focus_page(window);
                    } else {
                        window.close_webview(view.id());
                    }
                }
            },
            Some("queryDOM" | "mutateDOM" | "evaluateScript") => {
                let Some(id) = request["id"].as_u64() else {
                    return;
                };
                if let Err(error) = self.evaluate(window, &request, id) {
                    self.messages
                        .replies
                        .borrow_mut()
                        .push(json!({ "id": id, "error": error }));
                }
            },
            _ => {},
        }
    }

    fn evaluate(
        &self,
        window: &ServoShellWindow,
        request: &Value,
        id: u64,
    ) -> Result<(), &'static str> {
        let target = &request["target"];
        if target["frame"] != "top" {
            return Err("Only the top-level frame is supported");
        }
        let view = target_view(window, &target["tab"]).ok_or("Tab no longer exists")?;
        let document = match target["document"].as_str() {
            Some(document) => Some(document.parse().map_err(|_| "Invalid document ID")?),
            None if request["op"] == "queryDOM" && target["document"].is_null() => None,
            None => return Err("A document ID is required"),
        };
        let script = match request["op"].as_str() {
            Some("queryDOM") => {
                format!("({QUERY})({}, {})", request["selector"], request["fields"],)
            },
            Some("mutateDOM") => format!("({MUTATE})({})", request["operations"]),
            Some("evaluateScript") => {
                let script = request["script"].as_str().ok_or("Expected a script body")?;
                format!(
                    "(function() {{\n{script}\n}}).apply(undefined, {})",
                    request["args"]
                )
            },
            _ => return Err("Unknown operation"),
        };
        let messages = Rc::downgrade(&self.messages);
        view.evaluate_javascript_in_document(document, script, move |result| {
            if let Some(messages) = messages.upgrade() {
                let reply = match result {
                    Ok(value) => json!({ "id": id, "result": json_value(value) }),
                    Err(error) => json!({ "id": id, "error": format!("{error:?}") }),
                };
                messages.replies.borrow_mut().push(reply);
            }
        });
        Ok(())
    }

    pub(crate) fn focus_address(&self) {
        self.view
            .evaluate_javascript("window.focusAddress()", |_| {});
    }
}

fn focus_page(window: &ServoShellWindow) {
    if let Some(headed) = window.platform_window().as_headed_window() {
        headed.focus_chrome(window, false);
    }
}

fn target_view(window: &ServoShellWindow, tab: &Value) -> Option<WebView> {
    let tab = tab.as_str()?;
    window
        .webviews()
        .into_iter()
        .find_map(|(id, view)| (id.to_string() == tab).then_some(view))
}

fn json_value(value: JSValue) -> Value {
    match value {
        JSValue::Undefined | JSValue::Null => Value::Null,
        JSValue::Boolean(value) => json!(value),
        JSValue::Number(value) => json!(value),
        JSValue::String(value) => json!(value),
        JSValue::Array(values) => values.into_iter().map(json_value).collect(),
        JSValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(k, v)| (k, json_value(v)))
                .collect(),
        ),
        JSValue::Element(id) |
        JSValue::ShadowRoot(id) |
        JSValue::Frame(id) |
        JSValue::Window(id) => json!(id),
    }
}

const QUERY: &str = r#"function(selector, fields) {
    return Array.from(document.querySelectorAll(selector), node => {
        const result = Object.create(null);
        for (const field of fields) {
            switch (field) {
                case "text": result.text = node.textContent; break;
                case "html": result.html = node.outerHTML; break;
                case "value": result.value = node.value ?? null; break;
                case "attributes":
                    result.attributes = Object.fromEntries(
                        Array.from(node.attributes, attr => [attr.name, attr.value]));
                    break;
                default: throw new Error("Unknown DOM field: " + field);
            }
        }
        return result;
    });
}"#;

const MUTATE: &str = r#"function(operations) {
    return operations.map(operation => {
        const node = document.querySelector(operation.selector);
        if (!node) throw new Error("No match for " + operation.selector);
        switch (operation.type) {
            case "setText": node.textContent = operation.value; break;
            case "setAttribute": node.setAttribute(operation.name, operation.value); break;
            case "removeAttribute": node.removeAttribute(operation.name); break;
            case "setValue": node.value = operation.value; break;
            case "appendHTML": node.insertAdjacentHTML("beforeend", operation.value); break;
            case "remove": node.remove(); break;
            default: throw new Error("Unknown DOM mutation: " + operation.type);
        }
        return true;
    });
}"#;

const FIND: &str = r#"function(text, backward) {
    if (!text || !document.body) return;
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    const matches = [];
    let node;
    const pattern = text.toLowerCase();
    while (matches.length < 1000 && (node = walker.nextNode())) {
        const haystack = node.textContent.toLowerCase();
        let index = haystack.indexOf(pattern);
        if (index < 0 || !node.parentElement?.getClientRects().length) continue;
        for (; index >= 0 && matches.length < 1000; index = haystack.indexOf(pattern, index + pattern.length)) {
            if (index + text.length <= node.length) matches.push([node,index]);
        }
    }
    if (!matches.length) return;
    const selection = getSelection();
    const current = matches.findIndex(([node, index]) => selection.anchorNode === node && selection.anchorOffset === index);
    const direction = backward ? -1 : 1;
    const next = current === -1 ? (backward ? matches.length - 1 : 0) : (current + direction + matches.length) % matches.length;
    const [target,start] = matches[next];
    selection.setBaseAndExtent(target,start,target,start + text.length);
    target.parentElement.scrollIntoView({block:"nearest"});
}"#;
