/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The WebNN backend thread.
//!
//! Graphs are recorded and validated on the script thread by rustnn's backend-less graph
//! builder. Everything that touches a compute backend (contexts, compiled graphs and tensors)
//! lives on this thread, which owns rustnn's `MLContext`s. Requests are processed in order,
//! which gives the specification's timeline ordering of `writeTensor()`, `dispatch()` and
//! `readTensor()` for free. Replies are closures that queue a task back on the requesting
//! global.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use crossbeam_channel::{Sender, unbounded};
use log::{debug, warn};
use rustnn::GraphInfo;
use rustnn::mlcontext::{MLContext, MLGraph, MLTensor, MLTensorDescriptor};
use rustnn::mlcontextoptions::{MLContextOptions, MLPowerPreference};
use servo_base::panic::catch_unwind_without_abort;

/// Identifies a context, graph or tensor on the backend thread.
pub(crate) type BackendId = u64;

/// An error reported by the backend, mapped to a DOMException by the caller.
#[derive(Debug)]
pub(crate) enum BackendError {
    NotSupported(String),
    Operation(String),
}

pub(crate) type Reply<T> = Box<dyn FnOnce(Result<T, BackendError>) + Send>;

/// A named tensor binding for `dispatch()`.
pub(crate) type Bindings = Vec<(String, BackendId)>;

pub(crate) enum Request {
    CreateContext {
        id: BackendId,
        power_preference: MLPowerPreference,
        accelerated: bool,
        /// Replies with whether the context runs on an accelerator.
        reply: Reply<bool>,
    },
    DestroyContext {
        id: BackendId,
    },
    BuildGraph {
        context: BackendId,
        id: BackendId,
        graph: Box<GraphInfo>,
        reply: Reply<()>,
    },
    DestroyGraph {
        id: BackendId,
    },
    CreateTensor {
        context: BackendId,
        id: BackendId,
        descriptor: MLTensorDescriptor,
        /// Initial contents, for constant tensors.
        data: Option<Vec<u8>>,
        reply: Reply<()>,
    },
    DestroyTensor {
        id: BackendId,
    },
    WriteTensor {
        context: BackendId,
        tensor: BackendId,
        data: Vec<u8>,
    },
    ReadTensor {
        context: BackendId,
        tensor: BackendId,
        reply: Reply<Vec<u8>>,
    },
    Dispatch {
        context: BackendId,
        graph: BackendId,
        inputs: Bindings,
        outputs: Bindings,
    },
}

/// The most threads ONNX Runtime may use for one operation, shared by all graphs.
const MAX_RUNTIME_THREADS: usize = 4;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate an id for a backend object. Ids are allocated on the script side so that requests
/// can refer to an object before the backend has created it.
pub(crate) fn next_id() -> BackendId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Send a request to the backend thread, starting it if necessary.
pub(crate) fn send(request: Request) {
    static SENDER: OnceLock<Sender<Request>> = OnceLock::new();
    let sender = SENDER.get_or_init(|| {
        let (sender, receiver) = unbounded();
        thread::Builder::new()
            .name("WebNN".to_owned())
            .spawn(move || {
                let mut backend = Backend::default();
                while let Ok(request) = receiver.recv() {
                    backend.handle(request);
                }
            })
            .expect("Failed to spawn the WebNN thread");
        sender
    });
    if sender.send(request).is_err() {
        warn!("The WebNN thread has exited");
    }
}

#[derive(Default)]
struct Backend {
    /// Whether the ONNX Runtime library has been loaded, or why loading it failed.
    runtime: Option<Result<(), String>>,
    contexts: HashMap<BackendId, MLContext<'static>>,
    graphs: HashMap<BackendId, MLGraph<'static>>,
    tensors: HashMap<BackendId, MLTensor>,
}

fn operation_error(error: impl std::fmt::Display) -> BackendError {
    BackendError::Operation(error.to_string())
}

/// Run `f`, turning a panic in a backend into an error rather than taking the thread down.
fn guarded<T>(f: impl FnOnce() -> Result<T, BackendError>) -> Result<T, BackendError> {
    catch_unwind_without_abort(f).unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<&str>()
            .map(|message| message.to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown error".to_owned());
        Err(BackendError::Operation(format!(
            "The WebNN backend failed: {message}"
        )))
    })
}

impl Backend {
    /// Load ONNX Runtime, once. `ort` panics if the library cannot be found when it is first
    /// used, so check for it explicitly and report a missing library as `NotSupportedError`.
    fn ensure_runtime(&mut self) -> Result<(), BackendError> {
        let runtime = self.runtime.get_or_insert_with(|| {
            let path = std::env::var("ORT_DYLIB_PATH")
                .ok()
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| default_runtime_library().to_owned());
            let environment = ort::init_from(&path).map_err(|error| {
                format!(
                    "WebNN needs ONNX Runtime, which could not be loaded from {path:?}: {error}"
                )
            })?;
            // By default every ONNX Runtime session gets a thread pool as large as the machine,
            // and a page can create many graphs. Share one small pool instead. Environment
            // settings are first-come, so committing here also takes precedence over rustnn's
            // own initialization.
            let threads = thread::available_parallelism()
                .map_or(1, |threads| threads.get())
                .min(MAX_RUNTIME_THREADS);
            let thread_pool = ort::environment::GlobalThreadPoolOptions::default()
                .with_intra_threads(threads)
                .and_then(|options| options.with_inter_threads(1))
                .and_then(|options| options.with_spin_control(false))
                .map_err(|error| format!("Could not configure ONNX Runtime: {error}"))?;
            environment
                .with_name("WebNN")
                .with_telemetry(false)
                .with_global_thread_pool(thread_pool)
                .commit();
            Ok(())
        });
        runtime.clone().map_err(BackendError::NotSupported)
    }

    fn handle(&mut self, request: Request) {
        match request {
            Request::CreateContext {
                id,
                power_preference,
                accelerated,
                reply,
            } => {
                let result = self.ensure_runtime().and_then(|_| {
                    guarded(|| {
                        let options = MLContextOptions::new(power_preference, accelerated);
                        let context = MLContext::create(&options)
                            .map_err(|error| BackendError::NotSupported(error.to_string()))?;
                        let accelerated = context.accelerated();
                        self.contexts.insert(id, context);
                        Ok(accelerated)
                    })
                });
                reply(result);
            },
            Request::DestroyContext { id } => {
                self.contexts.remove(&id);
            },
            Request::BuildGraph {
                context,
                id,
                graph,
                reply,
            } => {
                let result = guarded(|| {
                    let context = self.context(context)?;
                    let graph = context
                        .rustnn_build_graph(*graph)
                        .map_err(operation_error)?;
                    self.graphs.insert(id, graph);
                    Ok(())
                });
                reply(result);
            },
            Request::DestroyGraph { id } => {
                self.graphs.remove(&id);
            },
            Request::CreateTensor {
                context,
                id,
                descriptor,
                data,
                reply,
            } => {
                let result = guarded(|| {
                    let context = self.context(context)?;
                    let tensor = context
                        .create_tensor(&descriptor)
                        .map_err(operation_error)?;
                    if let Some(data) = data {
                        // Constant tensors are created writable so that they can be filled
                        // here; script never writes them afterwards.
                        context
                            .write_tensor(&tensor, &data)
                            .map_err(operation_error)?;
                    }
                    self.tensors.insert(id, tensor);
                    Ok(())
                });
                reply(result);
            },
            Request::DestroyTensor { id } => {
                self.tensors.remove(&id);
            },
            Request::WriteTensor {
                context,
                tensor,
                data,
            } => {
                let result = guarded(|| {
                    let tensor = self.tensors.get(&tensor).ok_or_else(missing)?;
                    let context = self.contexts.get_mut(&context).ok_or_else(missing)?;
                    context.write_tensor(tensor, &data).map_err(operation_error)
                });
                if let Err(error) = result {
                    debug!("WebNN writeTensor failed: {error:?}");
                }
            },
            Request::ReadTensor {
                context,
                tensor,
                reply,
            } => {
                let result = guarded(|| {
                    let tensor = self.tensors.get(&tensor).ok_or_else(missing)?;
                    let context = self.contexts.get_mut(&context).ok_or_else(missing)?;
                    let mut data = vec![0u8; tensor.rustnn_required_bytes()];
                    context
                        .read_tensor(tensor, &mut data)
                        .map_err(operation_error)?;
                    Ok(data)
                });
                reply(result);
            },
            Request::Dispatch {
                context,
                graph,
                inputs,
                outputs,
            } => {
                let result = guarded(|| self.dispatch(context, graph, &inputs, &outputs));
                if let Err(error) = result {
                    debug!("WebNN dispatch failed: {error:?}");
                }
            },
        }
    }

    fn context(&mut self, id: BackendId) -> Result<&mut MLContext<'static>, BackendError> {
        self.contexts.get_mut(&id).ok_or_else(missing)
    }

    fn dispatch(
        &mut self,
        context: BackendId,
        graph: BackendId,
        inputs: &Bindings,
        outputs: &Bindings,
    ) -> Result<(), BackendError> {
        let inputs = named_tensors(&self.tensors, inputs)?;
        let outputs = named_tensors(&self.tensors, outputs)?;
        let graph = self.graphs.get_mut(&graph).ok_or_else(missing)?;
        let context = self.contexts.get_mut(&context).ok_or_else(missing)?;
        context
            .dispatch(graph, &inputs, &outputs)
            .map_err(operation_error)
    }
}

fn named_tensors<'a>(
    tensors: &'a HashMap<BackendId, MLTensor>,
    bindings: &'a Bindings,
) -> Result<BTreeMap<&'a str, &'a MLTensor>, BackendError> {
    bindings
        .iter()
        .map(|(name, id)| Ok((name.as_str(), tensors.get(id).ok_or_else(missing)?)))
        .collect()
}

fn missing() -> BackendError {
    BackendError::Operation("The WebNN object has been destroyed".to_owned())
}

fn default_runtime_library() -> &'static str {
    if cfg!(target_os = "windows") {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    }
}
