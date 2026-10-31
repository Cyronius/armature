/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://wicg.github.io/observable/#subscriber-api

[Exposed=*, Pref="dom_observable_enabled"]
interface Subscriber {
  undefined next(any value);
  undefined error(any error);
  undefined complete();
  undefined addTeardown(VoidFunction teardown);

  // True after the subscription closes, either by an error(), complete(), or
  // an abort() signal. Always false when the signal is aborted already.
  readonly attribute boolean active;
  readonly attribute AbortSignal signal;
};
