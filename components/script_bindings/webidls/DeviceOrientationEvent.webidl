/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://w3c.github.io/deviceorientation/#deviceorientation
[SecureContext, Exposed=Window]
interface DeviceOrientationEvent : Event {
  constructor(DOMString type, optional DeviceOrientationEventInit eventInitDict = {});

  readonly attribute double? alpha;
  readonly attribute double? beta;
  readonly attribute double? gamma;
  readonly attribute boolean absolute;
};

dictionary DeviceOrientationEventInit : EventInit {
  double? alpha = null;
  double? beta = null;
  double? gamma = null;
  boolean absolute = false;
};
