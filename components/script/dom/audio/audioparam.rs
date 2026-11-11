/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::sync::mpsc;

use dom_struct::dom_struct;
use js::context::JSContext;
use script_bindings::cell::DomRefCell;
use script_bindings::cformat;
use script_bindings::reflector::{Reflector, reflect_dom_object};
use servo_media::audio::audio_node::{AudioNodeMessage, AudioNodeType};
use servo_media::audio::graph::NodeId;
use servo_media::audio::param::{ParamRate, ParamType, RampKind, UserAutomationEvent};

use crate::conversions::Convert;
use crate::dom::audio::baseaudiocontext::BaseAudioContext;
use crate::dom::bindings::codegen::Bindings::AudioParamBinding::{
    AudioParamMethods, AutomationRate,
};
use crate::dom::bindings::codegen::Bindings::BaseAudioContextBinding::BaseAudioContextMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::num::Finite;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::window::Window;

/// The kind of an automation event, tracked to validate new events against `setValueCurveAtTime`.
#[derive(Clone, Copy, MallocSizeOf, PartialEq)]
enum AutomationEventKind {
    /// `setValueAtTime()` and `setTargetAtTime()`.
    Point,
    /// `linearRampToValueAtTime()` and `exponentialRampToValueAtTime()`.
    Ramp,
    /// `setValueCurveAtTime()`.
    Curve,
}

/// An automation event scheduled on an [`AudioParam`].
#[derive(Clone, Copy, MallocSizeOf)]
struct AutomationEvent {
    kind: AutomationEventKind,
    start: f64,
    /// The end of a curve; equal to `start` for other events.
    end: f64,
}

#[dom_struct]
pub(crate) struct AudioParam {
    reflector_: Reflector,
    context: Dom<BaseAudioContext>,
    #[no_trace]
    node: Option<NodeId>,
    #[no_trace]
    node_type: AudioNodeType,
    #[no_trace]
    param: ParamType,
    automation_rate: Cell<AutomationRate>,
    default_value: f32,
    min_value: f32,
    max_value: f32,
    /// The scheduled automation events, used to throw `NotSupportedError` for events that
    /// conflict with a `setValueCurveAtTime()` interval.
    #[no_trace]
    events: DomRefCell<Vec<AutomationEvent>>,
}

impl AudioParam {
    #[expect(clippy::too_many_arguments)]
    pub(crate) fn new_inherited(
        context: &BaseAudioContext,
        node: Option<NodeId>,
        node_type: AudioNodeType,
        param: ParamType,
        automation_rate: AutomationRate,
        default_value: f32,
        min_value: f32,
        max_value: f32,
    ) -> AudioParam {
        AudioParam {
            reflector_: Reflector::new(),
            context: Dom::from_ref(context),
            node,
            node_type,
            param,
            automation_rate: Cell::new(automation_rate),
            default_value,
            min_value,
            max_value,
            events: Default::default(),
        }
    }

    #[expect(clippy::too_many_arguments)]
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    pub(crate) fn new(
        cx: &mut JSContext,
        window: &Window,
        context: &BaseAudioContext,
        node: Option<NodeId>,
        node_type: AudioNodeType,
        param: ParamType,
        automation_rate: AutomationRate,
        default_value: f32,
        min_value: f32,
        max_value: f32,
    ) -> DomRoot<AudioParam> {
        let audio_param = AudioParam::new_inherited(
            context,
            node,
            node_type,
            param,
            automation_rate,
            default_value,
            min_value,
            max_value,
        );
        // Update the value range
        audio_param.message_node(AudioNodeMessage::SetParamRange(
            audio_param.param,
            (min_value, max_value),
        ));
        reflect_dom_object(cx, Box::new(audio_param), window)
    }

    fn message_node(&self, message: AudioNodeMessage) {
        if let Some(node_id) = self.node {
            self.context
                .audio_context_impl()
                .lock()
                .unwrap()
                .message_node(node_id, message);
        }
    }

    /// Check that an event of `kind` at `time` does not fall inside a value curve, then record it.
    fn schedule_event(&self, kind: AutomationEventKind, time: f64) -> Fallible<()> {
        let mut events = self.events.borrow_mut();
        let inside_curve = events.iter().any(|event| {
            event.kind == AutomationEventKind::Curve &&
                match kind {
                    AutomationEventKind::Ramp => event.start < time && time < event.end,
                    _ => event.start <= time && time < event.end,
                }
        });
        if inside_curve {
            return Err(Error::NotSupported(Some(String::from(
                "the event falls inside a setValueCurveAtTime curve",
            ))));
        }
        events.push(AutomationEvent {
            kind,
            start: time,
            end: time,
        });
        Ok(())
    }

    /// Check that a value curve over `[start, end)` does not overlap other events, then record it.
    fn schedule_curve(&self, start: f64, end: f64) -> Fallible<()> {
        let mut events = self.events.borrow_mut();
        let overlaps = events.iter().any(|event| match event.kind {
            AutomationEventKind::Curve => start < event.end && event.start < end,
            // An event at the start of the curve is fine, since the curve takes over from it.
            AutomationEventKind::Ramp | AutomationEventKind::Point => {
                start < event.start && event.start < end
            },
        });
        if overlaps {
            return Err(Error::NotSupported(Some(String::from(
                "the curve overlaps another automation event",
            ))));
        }
        events.push(AutomationEvent {
            kind: AutomationEventKind::Curve,
            start,
            end,
        });
        Ok(())
    }

    pub(crate) fn context(&self) -> &BaseAudioContext {
        &self.context
    }

    pub(crate) fn node_id(&self) -> Option<NodeId> {
        self.node
    }

    pub(crate) fn param_type(&self) -> ParamType {
        self.param
    }
}

impl AudioParamMethods<crate::DomTypeHolder> for AudioParam {
    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-automationrate>
    fn AutomationRate(&self) -> AutomationRate {
        self.automation_rate.get()
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-automationrate>
    fn SetAutomationRate(&self, automation_rate: AutomationRate) -> Fallible<()> {
        // > AudioBufferSourceNode
        // > The AudioParams playbackRate and detune MUST be "k-rate". An InvalidStateError must be
        // > thrown if the rate is changed to "a-rate".
        if automation_rate == AutomationRate::A_rate &&
            self.node_type == AudioNodeType::AudioBufferSourceNode &&
            (self.param == ParamType::Detune || self.param == ParamType::PlaybackRate)
        {
            return Err(Error::InvalidState(None));
        }

        self.automation_rate.set(automation_rate);
        self.message_node(AudioNodeMessage::SetParamRate(
            self.param,
            automation_rate.convert(),
        ));

        Ok(())
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-value>
    fn GetValue(&self) -> Fallible<Finite<f32>> {
        if self.node.is_none() {
            return Ok(Finite::wrap(self.default_value));
        }
        let (tx, rx) = mpsc::channel();
        self.message_node(AudioNodeMessage::GetParamValue(self.param, tx));
        Ok(Finite::wrap(rx.recv().unwrap()))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-value>
    fn SetValue(&self, value: Finite<f32>) -> Fallible<()> {
        // Setting the value behaves like `setValueAtTime(value, currentTime)`.
        let current_time = *self.context.CurrentTime();
        self.schedule_event(AutomationEventKind::Point, current_time)?;
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::SetValue(*value),
        ));
        Ok(())
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-defaultvalue>
    fn DefaultValue(&self) -> Finite<f32> {
        Finite::wrap(self.default_value)
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-minvalue>
    fn MinValue(&self) -> Finite<f32> {
        Finite::wrap(self.min_value)
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-maxvalue>
    fn MaxValue(&self) -> Finite<f32> {
        Finite::wrap(self.max_value)
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-setvalueattime>
    fn SetValueAtTime(
        &self,
        value: Finite<f32>,
        start_time: Finite<f64>,
    ) -> Fallible<DomRoot<AudioParam>> {
        if *start_time < 0. {
            return Err(Error::Range(cformat!(
                "start time {} should not be negative",
                *start_time
            )));
        }
        self.schedule_event(AutomationEventKind::Point, *start_time)?;
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::SetValueAtTime(*value, *start_time),
        ));
        Ok(DomRoot::from_ref(self))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-linearramptovalueattime>
    fn LinearRampToValueAtTime(
        &self,
        value: Finite<f32>,
        end_time: Finite<f64>,
    ) -> Fallible<DomRoot<AudioParam>> {
        if *end_time < 0. {
            return Err(Error::Range(cformat!(
                "end time {} should not be negative",
                *end_time
            )));
        }
        self.schedule_event(AutomationEventKind::Ramp, *end_time)?;
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::RampToValueAtTime(RampKind::Linear, *value, *end_time),
        ));
        Ok(DomRoot::from_ref(self))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-exponentialramptovalueattime>
    fn ExponentialRampToValueAtTime(
        &self,
        value: Finite<f32>,
        end_time: Finite<f64>,
    ) -> Fallible<DomRoot<AudioParam>> {
        if *end_time < 0. {
            return Err(Error::Range(cformat!(
                "end time {} should not be negative",
                *end_time
            )));
        }
        if *value == 0. {
            return Err(Error::Range(cformat!(
                "target value {} should not be 0",
                *value
            )));
        }
        self.schedule_event(AutomationEventKind::Ramp, *end_time)?;
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::RampToValueAtTime(RampKind::Exponential, *value, *end_time),
        ));
        Ok(DomRoot::from_ref(self))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-settargetattime>
    fn SetTargetAtTime(
        &self,
        target: Finite<f32>,
        start_time: Finite<f64>,
        time_constant: Finite<f32>,
    ) -> Fallible<DomRoot<AudioParam>> {
        if *start_time < 0. {
            return Err(Error::Range(cformat!(
                "start time {} should not be negative",
                *start_time
            )));
        }
        if *time_constant < 0. {
            return Err(Error::Range(cformat!(
                "time constant {} should not be negative",
                *time_constant
            )));
        }
        self.schedule_event(AutomationEventKind::Point, *start_time)?;
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::SetTargetAtTime(*target, *start_time, (*time_constant).into()),
        ));
        Ok(DomRoot::from_ref(self))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-setvaluecurveattime>
    fn SetValueCurveAtTime(
        &self,
        values: Vec<Finite<f32>>,
        start_time: Finite<f64>,
        end_time: Finite<f64>,
    ) -> Fallible<DomRoot<AudioParam>> {
        if *start_time < 0. {
            return Err(Error::Range(cformat!(
                "start time {} should not be negative",
                *start_time
            )));
        }
        if values.len() < 2. as usize {
            return Err(Error::InvalidState(None));
        }

        if *end_time < 0. {
            return Err(Error::Range(cformat!(
                "end time {} should not be negative",
                *end_time
            )));
        }
        self.schedule_curve(*start_time, *start_time + *end_time)?;
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::SetValueCurveAtTime(
                values.into_iter().map(|v| *v).collect(),
                *start_time,
                *end_time,
            ),
        ));
        Ok(DomRoot::from_ref(self))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-cancelscheduledvalues>
    fn CancelScheduledValues(&self, cancel_time: Finite<f64>) -> Fallible<DomRoot<AudioParam>> {
        if *cancel_time < 0. {
            return Err(Error::Range(cformat!(
                "cancel time {} should not be negative",
                *cancel_time
            )));
        }
        // Events at or after the cancel time are removed, including curves that span it.
        self.events
            .borrow_mut()
            .retain(|event| event.start < *cancel_time && event.end <= *cancel_time);
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::CancelScheduledValues(*cancel_time),
        ));
        Ok(DomRoot::from_ref(self))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-audioparam-cancelandholdattime>
    fn CancelAndHoldAtTime(&self, cancel_time: Finite<f64>) -> Fallible<DomRoot<AudioParam>> {
        if *cancel_time < 0. {
            return Err(Error::Range(cformat!(
                "cancel time {} should not be negative",
                *cancel_time
            )));
        }
        // Events after the cancel time are removed, and a curve spanning it is cut short.
        {
            let mut events = self.events.borrow_mut();
            events.retain(|event| event.start < *cancel_time);
            for event in events.iter_mut() {
                event.end = event.end.min(*cancel_time);
            }
        }
        self.message_node(AudioNodeMessage::SetParam(
            self.param,
            UserAutomationEvent::CancelAndHoldAtTime(*cancel_time),
        ));
        Ok(DomRoot::from_ref(self))
    }
}

// https://webaudio.github.io/web-audio-api/#enumdef-automationrate
impl Convert<ParamRate> for AutomationRate {
    fn convert(self) -> ParamRate {
        match self {
            AutomationRate::A_rate => ParamRate::ARate,
            AutomationRate::K_rate => ParamRate::KRate,
        }
    }
}
