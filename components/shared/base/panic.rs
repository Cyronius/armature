/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Recovering from panics in third-party code.

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

thread_local! {
    /// Set while running code whose panics are caught, so that the embedder's panic hook
    /// doesn't treat them as fatal. servoshell's hook terminates the process in `hard_fail`
    /// mode even when the panic is caught with `catch_unwind`.
    pub static SUPPRESS_ABORT_IN_PANIC_HOOK: Cell<bool> = const { Cell::new(false) };
}

/// Run `f`, catching a panic without letting the embedder's panic hook terminate the
/// process. Only use this for code that can be abandoned after panicking, such as a
/// third-party library call whose state is discarded.
pub fn catch_unwind_without_abort<R>(f: impl FnOnce() -> R) -> std::thread::Result<R> {
    let previous = SUPPRESS_ABORT_IN_PANIC_HOOK.replace(true);
    let result = catch_unwind(AssertUnwindSafe(f));
    SUPPRESS_ABORT_IN_PANIC_HOOK.set(previous);
    result
}
