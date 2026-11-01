/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/deviceorientation/#devicemotion
[SecureContext, Exposed=Window]
interface DeviceMotionEventAcceleration {
  readonly attribute double? x;
  readonly attribute double? y;
  readonly attribute double? z;
};

[SecureContext, Exposed=Window]
interface DeviceMotionEventRotationRate {
  readonly attribute double? alpha;
  readonly attribute double? beta;
  readonly attribute double? gamma;
};

[SecureContext, Exposed=Window]
interface DeviceMotionEvent : Event {
  constructor(DOMString type, optional DeviceMotionEventInit eventInitDict = {});

  readonly attribute DeviceMotionEventAcceleration? acceleration;
  readonly attribute DeviceMotionEventAcceleration? accelerationIncludingGravity;
  readonly attribute DeviceMotionEventRotationRate? rotationRate;
  readonly attribute double interval;
};

dictionary DeviceMotionEventAccelerationInit {
  double? x = null;
  double? y = null;
  double? z = null;
};

dictionary DeviceMotionEventRotationRateInit {
  double? alpha = null;
  double? beta = null;
  double? gamma = null;
};

dictionary DeviceMotionEventInit : EventInit {
  DeviceMotionEventAccelerationInit acceleration;
  DeviceMotionEventAccelerationInit accelerationIncludingGravity;
  DeviceMotionEventRotationRateInit rotationRate;
  double interval = 0;
};
