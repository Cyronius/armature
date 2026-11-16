/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The Web Neural Network API: <https://webmachinelearning.github.io/webnn/>
//!
//! Graphs are validated and recorded with rustnn on the script thread and compiled and run with
//! ONNX Runtime on the WebNN backend thread (see [`backend`]).

pub(crate) mod backend;
mod convert;
pub(crate) mod ml;
pub(crate) mod mlcontext;
pub(crate) mod mlgraph;
pub(crate) mod mlgraphbuilder;
pub(crate) mod mloperand;
pub(crate) mod mltensor;
mod options;
