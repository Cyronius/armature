/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use std::ptr;

use dom_struct::dom_struct;
use js::context::JSContext;
use js::gc::CustomAutoRooterGuard;
use js::jsapi::JSObject;
use js::rust::HandleObject;
use js::typedarray::{Float32Array, HeapFloat32Array};
use script_bindings::cell::DomRefCell;
use script_bindings::reflector::reflect_dom_object_with_proto;
use servo_media::audio::audio_node::{AudioNodeInit, AudioNodeMessage};
use servo_media::audio::graph::NodeId;
use servo_media::audio::wave_shaper_node::{
    OverSampleType as ServoMediaOverSampleType, WaveShaperNodeMessage, WaveShaperNodeOptions,
};

use crate::dom::audio::audionode::{AudioNode, AudioNodeOptionsHelper};
use crate::dom::audio::baseaudiocontext::BaseAudioContext;
use crate::dom::audionode::UnwrappedAudioNodeOptions;
use crate::dom::bindings::buffer_source::create_buffer_source;
use crate::dom::bindings::codegen::Bindings::AudioNodeBinding::{
    ChannelCountMode, ChannelInterpretation,
};
use crate::dom::bindings::codegen::Bindings::WaveShaperNodeBinding::{
    OverSampleType, WaveShaperNodeMethods, WaveShaperOptions,
};
use crate::dom::bindings::error::{Error, ErrorResult, Fallible};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::trace::RootedTraceableBox;
use crate::dom::window::Window;

#[dom_struct]
pub(crate) struct WaveShaperNode {
    node: AudioNode,
    /// A copy of the curve, since the spec makes a copy when it is set.
    curve: DomRefCell<Option<Vec<f32>>>,
    oversample: Cell<OverSampleType>,
}

impl WaveShaperNode {
    fn new_inherited(
        node_id: Option<NodeId>,
        context: &BaseAudioContext,
        node_options: UnwrappedAudioNodeOptions,
        curve: Option<Vec<f32>>,
        oversample: OverSampleType,
    ) -> Self {
        WaveShaperNode {
            node: AudioNode::new_inherited(
                node_id,
                context,
                node_options,
                1, // inputs
                1, // outputs
            ),
            curve: DomRefCell::new(curve),
            oversample: Cell::new(oversample),
        }
    }
}

fn convert_oversample(oversample: OverSampleType) -> ServoMediaOverSampleType {
    match oversample {
        OverSampleType::None => ServoMediaOverSampleType::None,
        OverSampleType::_2x => ServoMediaOverSampleType::Double,
        OverSampleType::_4x => ServoMediaOverSampleType::Quadruple,
    }
}

impl WaveShaperNodeMethods<crate::DomTypeHolder> for WaveShaperNode {
    /// <https://webaudio.github.io/web-audio-api/#dom-waveshapernode-waveshapernode>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        proto: Option<HandleObject>,
        context: &BaseAudioContext,
        options: &WaveShaperOptions,
    ) -> Fallible<DomRoot<WaveShaperNode>> {
        let node_options =
            options
                .parent
                .unwrap_or(2, ChannelCountMode::Max, ChannelInterpretation::Speakers);

        if AudioNode::channel_count_check(node_options.count) {
            return Err(Error::NotSupported(Some(String::from(
                "Channel count not supported",
            ))));
        }

        let curve = options
            .curve
            .as_ref()
            .map(|curve| curve.iter().map(|value| **value).collect::<Vec<f32>>());
        if curve.as_ref().is_some_and(|curve| curve.len() < 2) {
            return Err(Error::InvalidState(Some(String::from(
                "the curve needs at least two values",
            ))));
        }

        let audio_node_init = AudioNodeInit::WaveShaperNode(WaveShaperNodeOptions {
            curve: curve.clone(),
            oversample: convert_oversample(options.oversample),
        });
        let node_id =
            AudioNode::generate_audio_node_id(cx, &node_options, audio_node_init, context);

        Ok(reflect_dom_object_with_proto(
            cx,
            Box::new(WaveShaperNode::new_inherited(
                node_id,
                context,
                node_options,
                curve,
                options.oversample,
            )),
            window,
            proto,
        ))
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-waveshapernode-curve>
    fn GetCurve(
        &self,
        cx: &mut JSContext,
    ) -> Fallible<Option<RootedTraceableBox<HeapFloat32Array>>> {
        let curve = self.curve.borrow();
        let Some(curve) = curve.as_ref() else {
            return Ok(None);
        };
        rooted!(&in(cx) let mut array = ptr::null_mut::<JSObject>());
        create_buffer_source(cx, curve, array.handle_mut())
            .map(Some)
            .map_err(|_| Error::JSFailed)
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-waveshapernode-curve>
    fn SetCurve(&self, curve: CustomAutoRooterGuard<Option<Float32Array>>) -> ErrorResult {
        let new_curve = match &*curve {
            Some(curve) => {
                let curve = curve.to_vec().unwrap_or_default();
                if curve.len() < 2 {
                    return Err(Error::InvalidState(Some(String::from(
                        "the curve needs at least two values",
                    ))));
                }
                Some(curve)
            },
            None => None,
        };
        // The audio engine only accepts a curve if none is set.
        if new_curve.is_some() && self.curve.borrow().is_some() {
            return Err(Error::InvalidState(Some(String::from(
                "the curve can only be set once",
            ))));
        }
        self.node.message(AudioNodeMessage::WaveShaperNode(
            WaveShaperNodeMessage::SetCurve(new_curve.clone()),
        ));
        *self.curve.borrow_mut() = new_curve;
        Ok(())
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-waveshapernode-oversample>
    fn Oversample(&self) -> OverSampleType {
        self.oversample.get()
    }

    /// <https://webaudio.github.io/web-audio-api/#dom-waveshapernode-oversample>
    fn SetOversample(&self, oversample: OverSampleType) {
        // The audio engine does not support changing the oversampling yet.
        self.oversample.set(oversample);
    }
}
