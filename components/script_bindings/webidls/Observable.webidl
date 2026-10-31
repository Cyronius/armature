/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// https://wicg.github.io/observable/#observable-api

callback SubscribeCallback = undefined (Subscriber subscriber);
callback ObservableSubscriptionCallback = undefined (any value);

dictionary SubscriptionObserver {
  ObservableSubscriptionCallback next;
  ObservableSubscriptionCallback error;
  VoidFunction complete;
};

typedef (ObservableSubscriptionCallback or SubscriptionObserver) ObserverUnion;

dictionary SubscribeOptions {
  AbortSignal signal;
};

[Exposed=*, Pref="dom_observable_enabled"]
interface Observable {
  constructor(SubscribeCallback callback);
  undefined subscribe(optional ObserverUnion observer = {}, optional SubscribeOptions options = {});
};
