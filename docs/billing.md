# Overview

+ Pricing details
+ Data model
+ Stripe Integration
+ Apple Integration
+ Google Integration
+ Bitcoin Integration (later)
+ Monero Integration (later)

# Pricing

Pricing will be on an `Anual` basis or a `Monthly` basis.

Billing will be:

+ `$2.50 / month per 50gb`
+ `$25 / year per $50gb`

# Data Model

Upon write operations we want to be able to check to see if a user has space available for this action or not.

We'll keep a table that will store:

+ `account`
+ `tier-max`
+ `valid_until`
+ `type` -- one of `stripe`, `apple` or `google`
+ `billing failed` -- boolean to prompt user to go through billing cycle again

# Stripe

Used for: 

+ macOS
+ Linux
+ Cli
+ Windows

## Flow

User performs a sync, a new error type is returned saying they're out of space.

A dialog pops up to explain your situation, allows you to select a plan & a payment method (if you have a choice).

For now [Stripe](https://stripe.com/) will be our payment processor. Their subscription overview can be found [here](https://stripe.com/docs/billing/subscriptions/overview) will be the processing service.

To setup payment for the first time you need to send `Plan` info and `Payment` info.

`lockbook_server` will then:
1. [Create a `PaymentMethod`](https://stripe.com/docs/api/payment_methods/create)
2. [Create a `Customer`](https://stripe.com/docs/api/customers/create)
3. [Create a `Subscription`](https://stripe.com/docs/api/subscriptions/create)

If the `Subscription`'s `status` is `active`, then it will create an entry in the `tier_table` accordingly and return success. If it's rejected due to card details then we'll indicate the failure.

If we indicate failure, they'll have to retry.

There could be some annoying state-related things that we'll have to address (customer object exists already in stripe). We can either:

1. store info
2. cleanup stripe upon failure
3. look for customer already exists errors

Stripe will inform us about billing success and failures via [webhooks](https://stripe.com/docs/webhooks). We'll tell them a URL to send info too. Upon failures, stripe will [retry](https://stripe.com/docs/webhooks/best-practices#retry-logic), [email us](https://stripe.com/docs/webhooks/best-practices#disable-logic) and ultimately stop trying. We can also [query for missed events](https://stripe.com/docs/api/events/list) and [see failures in their UI](https://dashboard.stripe.com/events).

We will likely only need to listen for billing failures. In the case of a billing failure we're going to want to indicate to the user their card was declined. We don't want to just communicate that they're out of space. We want to communicate specifically that their card was declined. Likely what we'll do in this situation is keep them in that tier with that expiry information and set `billing_failed` to `true`. Next time they try to write we'll send them to the flow for declined cards. When the new request comes in it will have to complete logic to cancel the old subscription, especially if it's being transfered from one platform to another.

When consuming webhook events, we'll likely need to enable `tls`, and [verify that the event is coming from stripe](https://stripe.com/docs/webhooks/signatures) and not some random person.

Before this flow is completable we'll have to pre-register our [prices](https://stripe.com/docs/api/prices) with stripe.

## Testing

TODO

# Apple Integration 

Used for

+ iPhones
+ iPads

We have to use Apple's in app payments because apple forces all the apps on it's appstore to do so. An overview of their subscriptions can be found [here](https://developer.apple.com/videos/play/wwdc2018/705/).

A good index for docs exists [here](https://developer.apple.com/in-app-purchase/)

## Flow

Similar to the flow described for stripe, user goes to sync and encounters an error related to billing. A `sheet` pops up that tells them either that their card was declined or that they're out of space.

The user clicks one of the buttons for whatever plan they want, apple pay pops up, they complete the flow & get access to more space.

Before this flow is possible we'll have to preconfigure our [in-app-purchase](https://help.apple.com/app-store-connect/#/devb57be10e7) with apple. Some more details [here](https://help.apple.com/app-store-connect/#/devae49fb316).

Similar to the stripe flow, apple's servers will communicate with ours via webhooks. We'll [register](https://help.apple.com/app-store-connect/#/devb57be10e7) the webhook and then we'll be able to receive events.

Apple requires [ATS](https://developer.apple.com/documentation/security/preventing_insecure_network_connections) which seems like a subset of the `tls` requirement coming from stripe.

Unlike stripe which uses crypto for authentication, apple is just using webhooks to avoid their servers being polled. You therefore have apple themselves verify that the message is valid. This is detailed [here](https://developer.apple.com/documentation/storekit/in-app_purchase/validating_receipts_with_the_app_store). 

Details for how we'll deal with failures will be similar to stripe's. It's less likely these failures will happen, because this info is apple-id wide. Apple themselves will be promting the user to update their billing info. However, if we receive that event from apple it could indicate what they call [voluntary churn](https://developer.apple.com/app-store/subscriptions/#retaining-subscribers-using-receipt-information) which is when a user cancels their subscription for lockbook. We'll follow the card declined flow in this case. Further thought in the future can be given to helping them select files to delete (oldest / least often used) or creating flows to deep-archive files for suspended users.


## Testing

TODO

# Google integration

Exactly the same as apple's experience. [Overview](https://developer.android.com/google/play/billing/subscriptions)

[Pre-register tiers with google](https://developer.android.com/google/play/billing/getting-ready#products)

[Client side details](https://developer.android.com/google/play/billing/integrate).

[Server side details](https://developer.android.com/google/play/billing/getting-ready#configure-rtdn). [Events they send](https://developer.android.com/google/play/billing/rtdn-reference#sub).

The verification happens like apple's, the token is a prompt for you to go and check with google about the status of a subscription, [details](https://medium.com/@emilieroberts/real-time-developer-notifications-happen-when-something-changes-with-a-users-subscription-cb46dc053495).



