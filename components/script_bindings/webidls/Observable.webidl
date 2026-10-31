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

callback ObservableInspectorAbortHandler = undefined (any value);

dictionary ObservableInspector {
  ObservableSubscriptionCallback next;
  ObservableSubscriptionCallback error;
  VoidFunction complete;

  VoidFunction subscribe;
  ObservableInspectorAbortHandler abort;
};

typedef (ObservableSubscriptionCallback or SubscriptionObserver) ObserverUnion;
typedef (ObservableSubscriptionCallback or ObservableInspector) ObservableInspectorUnion;

dictionary SubscribeOptions {
  AbortSignal signal;
};

callback Predicate = boolean (any value, unsigned long long index);
callback Reducer = any (any accumulator, any currentValue, unsigned long long index);
callback Mapper = any (any value, unsigned long long index);
callback Visitor = undefined (any value, unsigned long long index);

[Exposed=*, Pref="dom_observable_enabled"]
interface Observable {
  constructor(SubscribeCallback callback);
  undefined subscribe(optional ObserverUnion observer = {}, optional SubscribeOptions options = {});

  [Throws] static Observable from(any value);

  // Observable-returning operators. See https://wicg.github.io/observable/#observable-api.
  Observable map(Mapper mapper);
  Observable filter(Predicate predicate);
  Observable take(unsigned long long amount);
  Observable drop(unsigned long long amount);
  Observable inspect(optional ObservableInspectorUnion inspectorUnion = {});
  Observable finally(VoidFunction callback);

  // Promise-returning operators.
  Promise<sequence<any>> toArray(optional SubscribeOptions options = {});
  Promise<undefined> forEach(Visitor callback, optional SubscribeOptions options = {});
  Promise<boolean> every(Predicate predicate, optional SubscribeOptions options = {});
  Promise<any> first(optional SubscribeOptions options = {});
  Promise<any> last(optional SubscribeOptions options = {});
  Promise<any> find(Predicate predicate, optional SubscribeOptions options = {});
  Promise<boolean> some(Predicate predicate, optional SubscribeOptions options = {});
  Promise<any> reduce(Reducer reducer, optional any initialValue, optional SubscribeOptions options = {});
};

// https://wicg.github.io/observable/#event-target-integration
dictionary ObservableEventListenerOptions {
  boolean capture = false;
  boolean passive;
};

partial interface EventTarget {
  [Pref="dom_observable_enabled"]
  Observable when(DOMString type, optional ObservableEventListenerOptions options = {});
};
