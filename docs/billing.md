# Billing

+ Pricing details
+ Data model
+ Stripe Integration
+ Apple Integration
+ Google Integration
+ Bitcoin Integration (later)
+ Monero Integration (later)

# Pricing

Premium buys room, not features. Every account syncs; Free holds `25 MB` and Premium holds `30 GB`, for `$2.99 / month` or `$30 / year`.

# Data Model

Each account's `SubscriptionProfile` holds the billing platform it pays through, if any: Stripe, Google Play, or the App Store, with that platform's ids, the end of the paid period, and its state. The data cap follows from that state, and writes over the cap are refused.

# Stripe

Used for desktop (macOS, Windows, Linux), the CLI, and the F-Droid Android build.

People pay on Stripe's own pages, so card numbers never reach our server.

## Flow

1. The client asks for `/stripe-checkout` with a plan, monthly or yearly.
2. The server finds or creates the account's Stripe customer, records it in `stripe_ids`, and returns the URL of a [Checkout Session](https://docs.stripe.com/payments/checkout) for that customer and price.
3. The client opens the URL in a browser. Paying returns the buyer to `lockbook.net/premium/thanks`; leaving returns them to `lockbook.net/premium/canceled`.
4. Stripe sends a webhook. The server fetches the subscription and stores it on the account; the client sees Premium the next time it reads its usage.

`/stripe-billing-portal` returns the URL of Stripe's [customer portal](https://docs.stripe.com/customer-management), where the account changes its card, gets receipts, or cancels.

## Webhooks

`/stripe-webhooks` verifies Stripe's signature, then for each of these events fetches the subscription it names and makes the account match it:

+ `checkout.session.completed`
+ `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`
+ `invoice.paid`, `invoice.payment_failed`

Other events are acknowledged and ignored. The subscription's status decides the account's state: `active`, `trialing` and `past_due` are Premium, so Stripe's payment retries don't take it away; anything else is Free. A lapsed subscription never overwrites a newer one that is still active.

## Canceling

Canceling, from the portal or `/cancel-subscription`, stops renewal. Premium lasts until the end of the period already paid for, when Stripe sends `customer.subscription.deleted`. An account over the free limit after that keeps its files and can't write more until it frees space.

## Setup

The dashboard holds the branding (icon, colors, public business name), the payment methods, the portal's features, the webhook endpoint and its events. The server reads `STRIPE_SECRET`, `STRIPE_SIGNING_SECRET`, `STRIPE_PREMIUM_PRICE_ID` (monthly) and `STRIPE_PREMIUM_YEARLY_PRICE_ID`. The webhook endpoint's API version must match the one `async-stripe` speaks, currently `2023-10-16`.

## Testing

Run a local server with the test key in `server/local.env`, and forward events with the [Stripe CLI](https://docs.stripe.com/stripe-cli): `stripe listen --api-key <test key> --forward-to localhost:8000/stripe-webhooks`, putting the `whsec_` secret it prints in `STRIPE_SIGNING_SECRET`. Pay with Stripe's [test cards](https://docs.stripe.com/testing); `4000 0025 0000 3155` asks for bank approval. [Test clocks](https://docs.stripe.com/billing/testing/test-clocks) fast-forward renewals and cancellations.

# Apple Integration 

Used for

+ iPhones
+ iPads

We have to use Apple's in app payments because Apple forces all the apps on its App Store to do so. An overview of their subscriptions can be found [here](https://developer.apple.com/videos/play/wwdc2018/705/).

A good index for docs exists [here](https://developer.apple.com/in-app-purchase/)

## Flow

Similar to the flow described for stripe, user goes to sync and encounters an error related to billing. A `sheet` pops up that tells them either that their card was declined or that they're out of space.

The user clicks one of the buttons for whatever plan they want, apple pay pops up, they complete the flow & get access to more space.

Before this flow is possible we'll have to preconfigure our [in-app-purchase](https://help.apple.com/app-store-connect/#/devb57be10e7) with apple. Some more details [here](https://help.apple.com/app-store-connect/#/devae49fb316).

Similar to the Stripe flow, Apple's servers will communicate with ours via webhooks. We'll [register](https://help.apple.com/app-store-connect/#/devb57be10e7) the webhook and then we'll be able to receive events.

Apple requires [ATS](https://developer.apple.com/documentation/security/preventing_insecure_network_connections).

Unlike stripe which uses crypto for authentication, apple is just using webhooks to avoid their servers being polled. You therefore have apple themselves verify that the message is valid. This is detailed [here](https://developer.apple.com/documentation/storekit/in-app_purchase/validating_receipts_with_the_app_store). 

Details for how we'll deal with failures will be similar to Stripe's. It's less likely these failures will happen, because this info is Apple ID-wide. Apple themselves will be prompting the user to update their billing info. However, if we receive that event from Apple it could indicate what they call [voluntary churn](https://developer.apple.com/app-store/subscriptions/#retaining-subscribers-using-receipt-information) which is when a user cancels their subscription for lockbook. We'll follow the card declined flow in this case. Further thought in the future can be given to helping them select files to delete (oldest / least often used) or creating flows to deep-archive files for suspended users.


## Testing

TODO

# Google integration

Exactly the same as apple's experience. [Overview](https://developer.android.com/google/play/billing/subscriptions)

[Pre-register tiers with google](https://developer.android.com/google/play/billing/getting-ready#products)

[Client side details](https://developer.android.com/google/play/billing/integrate).

[Server side details](https://developer.android.com/google/play/billing/getting-ready#configure-rtdn). [Events they send](https://developer.android.com/google/play/billing/rtdn-reference#sub).

The verification happens like apple's, the token is a prompt for you to go and check with google about the status of a subscription, [details](https://medium.com/@emilieroberts/real-time-developer-notifications-happen-when-something-changes-with-a-users-subscription-cb46dc053495).

## Billing States

There are 4 states of a Google Play subscription (described by the `GooglePlayAccountState` enum in `libs/lb/lb-rs/libs/shared/src/api.rs`):

1. `Ok`: The user has a valid subscription that is renewing monthly.
2. `Canceled`: The user's subscription has been canceled, but they still have premium benefits until the end of their billing cycle.
3. `GracePeriod`: The user's subscription is in a grace period for failing to make a payment. They have premium benefits for 7 more days until they are `OnHold`.
4. `OnHold`: The user let their subscription's grace period expire and has lost premium benefits.

Once a subscription expires (either after being `OnHold` or `Canceled`), all remaining data about that subscription will be deleted. Also, if a user does a chargeback, their subscription is immediately revoked.

## Testing

An integration can be tested locally if a user becomes a [license tester](https://developer.android.com/google/play/billing/test). This allows fake payments to be made on a verified Google Play developer account to test debug builds. Paired with a [PubSub subscription](https://developer.android.com/google/play/billing/getting-ready#configure-rtdn) pointing at your local server instance, you can emulate almost any user interaction involving subscriptions.

# Platform Migrations

Due to a smaller service fee on Stripe, there is a strong incentive to migrate users off payment platforms by Google Play and Apple. Therefore, in the future, payment migrations may be implemented.
