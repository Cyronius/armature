/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::sync::{Mutex, mpsc};
use std::thread::Builder;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::realm::CurrentRealm;
use js::rust::HandleObject;
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::reflect_dom_object_with_proto;
use servo_base::id::PipelineId;
use servo_media::audio::context::OfflineAudioContextOptions as ServoMediaOfflineAudioContextOptions;

use crate::dom::audio::audiobuffer::{AudioBuffer, MAX_SAMPLE_RATE, MIN_SAMPLE_RATE};
use crate::dom::audio::audionode::MAX_CHANNEL_COUNT;
use crate::dom::audio::baseaudiocontext::{BaseAudioContext, BaseAudioContextOptions};
use crate::dom::audio::offlineaudiocompletionevent::OfflineAudioCompletionEvent;
use crate::dom::bindings::codegen::Bindings::BaseAudioContextBinding::AudioContextState;
use crate::dom::bindings::codegen::Bindings::BaseAudioContextBinding::BaseAudioContext_Binding::BaseAudioContextMethods;
use crate::dom::bindings::codegen::Bindings::OfflineAudioContextBinding::{
    OfflineAudioContextMethods, OfflineAudioContextOptions,
};
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::refcounted::Trusted;
use crate::dom::bindings::reflector::DomGlobal;
use crate::dom::bindings::root::DomRoot;
use crate::dom::event::{Event, EventBubbles, EventCancelable};
use crate::dom::promise::{Promise, RootedPromise, TracedPromise};
use crate::dom::window::Window;

#[dom_struct]
pub(crate) struct OfflineAudioContext {
    context: BaseAudioContext,
    channel_count: u32,
    length: u32,
    rendering_started: Cell<bool>,
    pending_rendering_promise: DomRefCell<Option<TracedPromise>>,
    /// The promises returned by `suspend()`, with the frame at which rendering suspends.
    suspend_promises: DomRefCell<Vec<(usize, TracedPromise)>>,
    suspend_callback_registered: Cell<bool>,
}

impl OfflineAudioContext {
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    fn new_inherited(
        channel_count: u32,
        length: u32,
        sample_rate: f32,
        pipeline_id: PipelineId,
    ) -> Fallible<OfflineAudioContext> {
        let options = ServoMediaOfflineAudioContextOptions {
            channels: channel_count as u8,
            length: length as usize,
            sample_rate,
        };
        let context = BaseAudioContext::new_inherited(
            BaseAudioContextOptions::OfflineAudioContext(options),
            pipeline_id,
        )?;
        Ok(OfflineAudioContext {
            context,
            channel_count,
            length,
            rendering_started: Cell::new(false),
            pending_rendering_promise: Default::default(),
            suspend_promises: Default::default(),
            suspend_callback_registered: Cell::new(false),
        })
    }

    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    fn new(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        channel_count: u32,
        length: u32,
        sample_rate: f32,
    ) -> Fallible<DomRoot<OfflineAudioContext>> {
        if channel_count > MAX_CHANNEL_COUNT ||
            channel_count == 0 ||
            length == 0 ||
            !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate)
        {
            return Err(Error::NotSupported(None));
        }
        let pipeline_id = window.pipeline_id();
        let context =
            OfflineAudioContext::new_inherited(channel_count, length, sample_rate, pipeline_id)?;
        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(context),
            window,
            proto,
        ))
    }
}

impl OfflineAudioContextMethods<crate::DomTypeHolder> for OfflineAudioContext {
    /// <https://webaudio.github.io/web-audio-api/#dom-offlineaudiocontext-offlineaudiocontext>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        options: &OfflineAudioContextOptions,
    ) -> Fallible<DomRoot<OfflineAudioContext>> {
        OfflineAudioContext::new(
            cx,
            window,
            proto,
            options.numberOfChannels,
            options.length,
            *options.sampleRate,
        )
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-offlineaudiocontext-offlineaudiocontext-numberofchannels-length-samplerate>
    fn Constructor_(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        number_of_channels: u32,
        length: u32,
        sample_rate: Finite<f32>,
    ) -> Fallible<DomRoot<OfflineAudioContext>> {
        OfflineAudioContext::new(cx, window, proto, number_of_channels, length, *sample_rate)
    }

    // https://webaudio.github.io/web-audio-api/#dom-offlineaudiocontext-oncomplete
    event_handler!(complete, GetOncomplete, SetOncomplete);

    /// <https://webaudio.github.io/web-audio-api/#dom-offlineaudiocontext-length>
    fn Length(&self) -> u32 {
        self.length
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-offlineaudiocontext-suspend>
    fn Suspend(&self, cx: &mut CurrentRealm, suspend_time: Finite<f64>) -> RootedPromise {
        let promise = Promise::new_in_realm(cx);
        let sample_rate = f64::from(*self.context.SampleRate());
        let suspend_time = *suspend_time;

        // The suspend time must not be in the past or after the end of the rendering.
        if suspend_time < 0. ||
            suspend_time < *self.context.CurrentTime() ||
            suspend_time >= f64::from(self.length) / sample_rate
        {
            promise.reject_error(cx, Error::Range(c"suspendTime is out of range".to_owned()));
            return promise;
        }

        // Round up to a multiple of the render quantum size.
        const RENDER_QUANTUM: f64 = 128.;
        let frame =
            ((suspend_time * sample_rate / RENDER_QUANTUM).ceil() * RENDER_QUANTUM) as usize;

        // Only one suspend can be scheduled for a given frame.
        if self
            .suspend_promises
            .borrow()
            .iter()
            .any(|(scheduled_frame, _)| *scheduled_frame == frame)
        {
            promise.reject_error(
                cx,
                Error::InvalidState(Some(String::from(
                    "a suspend is already scheduled for this frame",
                ))),
            );
            return promise;
        }
        self.suspend_promises
            .borrow_mut()
            .push((frame, promise.to_traced()));

        if !self.suspend_callback_registered.replace(true) {
            let this = Mutex::new(Trusted::new(self));
            let task_source = Mutex::new(
                self.global()
                    .task_manager()
                    .dom_manipulation_task_source()
                    .to_sendable(),
            );
            self.context
                .audio_context_impl()
                .lock()
                .unwrap()
                .set_offline_suspend_callback(Box::new(move |frame| {
                    let this = this.lock().unwrap().clone();
                    task_source
                        .lock()
                        .unwrap()
                        .queue(task!(offline_suspended: move |cx| {
                            let this = this.root();
                            this.context.set_state_attribute(AudioContextState::Suspended);
                            this.global()
                                .task_manager()
                                .dom_manipulation_task_source()
                                .queue_simple_event(this.upcast(), atom!("statechange"));
                            let promise = {
                                let mut promises = this.suspend_promises.borrow_mut();
                                promises
                                    .iter()
                                    .position(|(scheduled_frame, _)| *scheduled_frame == frame)
                                    .map(|index| promises.remove(index).1)
                            };
                            if let Some(promise) = promise {
                                promise.root(cx).resolve_native(cx, &());
                            }
                        }));
                }));
        }
        self.context
            .audio_context_impl()
            .lock()
            .unwrap()
            .schedule_offline_suspend(frame);

        promise
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-offlineaudiocontext-startrendering>
    fn StartRendering(&self, cx: &mut CurrentRealm) -> RootedPromise {
        let promise = Promise::new_in_realm(cx);
        if self.rendering_started.get() {
            promise.reject_error(cx, Error::InvalidState(None));
            return promise;
        }
        self.rendering_started.set(true);

        *self.pending_rendering_promise.safe_borrow_mut(cx.no_gc()) = Some(promise.to_traced());

        let length = self.length as usize;
        let channel_count = self.channel_count as usize;
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        self.context
            .audio_context_impl()
            .lock()
            .unwrap()
            .set_eos_callback(Box::new(move |buffer| {
                let channels: Vec<Vec<f32>> = (*buffer)
                    .as_ref()
                    .chunks(length)
                    .take(channel_count)
                    .map(|channel| channel.to_vec())
                    .collect();
                let _ = sender.lock().unwrap().send(channels);
            }));

        let this = Trusted::new(self);
        let task_source = self
            .global()
            .task_manager()
            .dom_manipulation_task_source()
            .to_sendable();
        Builder::new()
            .name("OfflineACResolver".to_owned())
            .spawn(move || {
                let processed_audio = receiver.recv().unwrap_or_default();
                task_source.queue(task!(resolve: move |cx| {
                    let this = this.root();
                    let mut processed_audio = processed_audio;
                    // it can end up being empty if the task failed
                    processed_audio.resize_with(this.channel_count as usize, || {
                        vec![0.; this.length as usize]
                    });
                    let buffer = AudioBuffer::new_with_channel_data(
                        cx,
                        this.global().as_window(),
                        this.length,
                        *this.context.SampleRate(),
                        processed_audio,
                    );
                    let promise = {
                        (*this
                            .pending_rendering_promise
                            .safe_borrow_mut(cx.no_gc()))
                            .take()
                            .unwrap()
                            .root(cx)
                    };
                    promise.resolve_native(cx, &buffer);
                    let global = &this.global();
                    let window = global.as_window();
                    let event = OfflineAudioCompletionEvent::new(cx, window,
                                                                 atom!("complete"),
                                                                 EventBubbles::DoesNotBubble,
                                                                 EventCancelable::NotCancelable,
                                                                 &buffer);
                    event.upcast::<Event>().fire(cx, this.upcast());
                }));
            })
            .unwrap();

        self.context.set_state_attribute(AudioContextState::Running);
        self.global()
            .task_manager()
            .dom_manipulation_task_source()
            .queue_simple_event(self.upcast(), atom!("statechange"));

        if self
            .context
            .audio_context_impl()
            .lock()
            .unwrap()
            .resume()
            .is_none()
        {
            promise.reject_error(
                cx,
                Error::Type(c"Could not start offline rendering".to_owned()),
            );
        }

        promise
    }
}
