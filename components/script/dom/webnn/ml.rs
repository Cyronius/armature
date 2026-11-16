/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::context::JSContext;
use js::realm::CurrentRealm;
use rustnn::mlcontextoptions::MLPowerPreference as RustnnPowerPreference;
use script_bindings::reflector::{Reflector, reflect_dom_object};

use crate::dom::bindings::codegen::Bindings::WebNNBinding::{
    MLContextOptions, MLMethods, MLPowerPreference,
};
use crate::dom::bindings::refcounted::{Trusted, TrustedPromise};
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::DomRoot;
use crate::dom::globalscope::GlobalScope;
use crate::dom::promise::{Promise, RootedPromise};
use crate::dom::webnn::backend::{self, Request};
use crate::dom::webnn::convert::backend_error;
use crate::dom::webnn::mlcontext::MLContext;

/// <https://webmachinelearning.github.io/webnn/#api-ml>
#[dom_struct]
pub(crate) struct ML {
    reflector_: Reflector,
}

impl ML {
    fn new_inherited() -> Self {
        Self {
            reflector_: Reflector::new(),
        }
    }

    pub(crate) fn new(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Self> {
        reflect_dom_object(cx, Box::new(Self::new_inherited()), global)
    }
}

impl MLMethods<crate::DomTypeHolder> for ML {
    /// <https://webmachinelearning.github.io/webnn/#dom-ml-createcontext>
    fn CreateContext(&self, realm: &mut CurrentRealm, options: &MLContextOptions) -> RootedPromise {
        let global = GlobalScope::from_current_realm(realm);
        let promise = Promise::new_in_realm(realm);

        let power_preference = match options.powerPreference {
            MLPowerPreference::Default => RustnnPowerPreference::Default,
            MLPowerPreference::High_performance => RustnnPowerPreference::HighPerformance,
            MLPowerPreference::Low_power => RustnnPowerPreference::LowPower,
        };
        let id = backend::next_id();
        let trusted_promise = TrustedPromise::new(&promise);
        let this = Trusted::new(self);
        let task_source = global
            .task_manager()
            .dom_manipulation_task_source()
            .to_sendable();
        backend::send(Request::CreateContext {
            id,
            power_preference,
            accelerated: options.accelerated,
            reply: Box::new(move |result| {
                task_source.queue(task!(webnn_context_created: move |cx| {
                    let promise = trusted_promise.root(cx);
                    match result {
                        Ok(accelerated) => {
                            let global = this.root().global();
                            let context = MLContext::new(cx, &global, id, accelerated);
                            promise.resolve_native(cx, &context);
                        },
                        Err(error) => promise.reject_error(cx, backend_error(error)),
                    }
                }));
            }),
        });
        promise
    }
}
