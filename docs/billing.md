# Billing

+ Pricing details
+ Data model
+ Stripe Integration
+ Apple Integration
+ Google Integration
+ Bitcoin Integration (later)
+ Monero Integration (later)

# Pricing

Premium buys room, not features. Every account syncs; Free holds `25 MB` and Premium holds `30 GB`, for `$2.99 / month`.

# Data Model

Each account's `SubscriptionProfile` holds the billing platform it pays through, if any: Stripe, Google Play, or the App Store, with that platform's ids, the end of the paid period, and its state. The data cap follows from that state, and writes over the cap are refused.

# Stripe

Used for desktop (macOS, Windows, Linux), the CLI, and the F-Droid Android build.

People pay on Stripe's own pages, so card numbers never reach our server.

## Flow

1. The client asks for `/stripe-checkout`.
2. The server finds or creates the account's Stripe customer, records it in `stripe_ids`, expires any older open checkout sessions for that customer, and returns the URL of a [Checkout Session](https://docs.stripe.com/payments/checkout).
3. The client opens the URL in a browser. Paying returns the buyer to `lockbook.net/checkout/thanks/`; leaving returns them to `lockbook.net/checkout/canceled/`.
4. Stripe sends a webhook. The server fetches the customer's subscriptions from Stripe and updates the account to match; the client sees Premium the next time it refreshes usage.

`/stripe-billing-portal` returns the URL of Stripe's [customer portal](https://docs.stripe.com/customer-management), where the account changes its card, gets receipts, or cancels.

## Webhooks

`/stripe-webhooks` verifies the `Stripe-Signature` HMAC-SHA256 header and timestamp tolerance, extracts `data.object.customer` from JSON for these events, and reconciles the account against Stripe's current subscriptions:

+ `checkout.session.completed`
+ `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`
+ `invoice.paid`, `invoice.payment_failed`

Other events are acknowledged and ignored. Both webhook parsing and outbound Stripe REST calls read only the small set of JSON fields Lockbook uses (no closed-enum Stripe SDK and no buyer PII logged), so Stripe API version upgrades or newly enabled payment methods won't break deserialization.

The subscription's status decides the account's state: `active`, `trialing` and `past_due` are Premium, so Stripe's payment retries don't take it away; anything else ends Stripe billing (`billing_platform = None`, matching Google Play). A lapsed Stripe customer never overwrites a newer live Stripe customer or an App Store / Google Play record (including recoverable `OnHold` / `FailedToRenew` states).

## Canceling & Account Deletion

Canceling, from the portal or `/cancel-subscription`, sets `cancel_at_period_end`. Premium lasts until the end of the period already paid for, when Stripe sends `customer.subscription.deleted`. An account over the free limit after that keeps its files and can't write more until it frees space.

Deleting a Lockbook account best-effort looks up the customer in Stripe (via `billing_platform` or `stripe_ids`) and immediately cancels any live Stripe subscriptions so deleted accounts aren't left billed.

## Admin Reconciliation

`admin check-stripe [--username=<name>]` compares cached DB state against live Stripe without writing. `admin sync-stripe [--username=<name>]` reconciles drifted rows from Stripe (and backfills any missing `stripe_ids` mappings). Legacy rows stored in Unix seconds before this migration report as drifted until synced.

## Setup

The Stripe dashboard holds branding (icon, colors, public business name), payment methods (Card, Apple Pay, Google Pay, Link — no async/delayed methods), portal features (cancel at period end, card update, invoices), and customer emails (receipts, failed renewals). The server reads `STRIPE_SECRET`, `STRIPE_SIGNING_SECRET`, and `STRIPE_PREMIUM_PRICE_ID`.

Required Dashboard settings before deploy:
+ **Webhook endpoint:** must subscribe to `checkout.session.completed`, `customer.subscription.created`, `customer.subscription.updated`, `customer.subscription.deleted`, `invoice.paid`, and `invoice.payment_failed` (`customer.subscription.deleted` is what ends canceled subscriptions).
+ **Smart Retries / Failed payment behavior:** configure Stripe to **cancel** or **mark unpaid** once retries exhaust (otherwise a non-paying `past_due` subscription stays Premium indefinitely).
+ **Duplicate prevention:** enable Stripe's *Limit customers to one subscription* Checkout setting and configure the Customer Portal.
+ **Deploy order:** deploy `public-site` (`/checkout/thanks/` and `/checkout/canceled/`) before the server, and verify `MILLIS_BETWEEN_LOCK_ATTEMPTS` is set in milliseconds in production.

## Testing

Run a local server with the test key in `server/local.env`, and forward events with the [Stripe CLI](https://docs.stripe.com/stripe-cli): `stripe listen --api-key <test key> --events checkout.session.completed,customer.subscription.created,customer.subscription.updated,customer.subscription.deleted,invoice.paid,invoice.payment_failed --forward-to localhost:8000/stripe-webhooks`, putting the `whsec_` secret it prints in `STRIPE_SIGNING_SECRET`. Pay with Stripe's [test cards](https://docs.stripe.com/testing); `4000 0025 0000 3155` asks for bank approval. [Test clocks](https://docs.stripe.com/billing/testing/test-clocks) fast-forward renewals and cancellations.

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
