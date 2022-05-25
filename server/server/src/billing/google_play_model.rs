use serde::Deserialize;


#[derive(Debug, Deserialize)]
pub struct PubsubMessage {
    pub data: String
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperNotification {
    pub version: String,
    pub package_name: String,
    pub event_time_millis: u64,
    pub one_time_product_notification: Option<OneTimeProductNotification>,
    pub subscription_notification: Option<SubscriptionNotification>,
    pub test_notification: Option<TestNotification>,
}

// (1) SUBSCRIPTION_RECOVERED - A subscription was recovered from account hold.
// (2) SUBSCRIPTION_RENEWED - An active subscription was renewed.
// (3) SUBSCRIPTION_CANCELED - A subscription was either voluntariy or involuntarily cancelled. For voluntary cancellation, sent when the user cancels.
// (4) SUBSCRIPTION_PURCHASED - A new subscription was purchased.
// (5) SUBSCRIPTION_ON_HOLD - A subscription has entered account hold (if enabled).
// (6) SUBSCRIPTION_IN_GRACE_PERIOD - A subscription has entered grace period (if enabled).
// (7) SUBSCRIPTION_RESTARTED - User has restored their subscription from Play > Account > Subscriptions. The subscription was canceled but had not expired yet when the user restores. For more information, see [Restorations](/google/play/billing/subscriptions#restore).
// (8) SUBSCRIPTION_PRICE_CHANGE_CONFIRMED - A subscription price change has successfully been confirmed by the user.
// (9) SUBSCRIPTION_DEFERRED - A subscription's recurrence time has been extended.
// (10) SUBSCRIPTION_PAUSED - A subscription has been paused.
// (11) SUBSCRIPTION_PAUSE_SCHEDULE_CHANGED - A subscription pause schedule has been changed.
// (12) SUBSCRIPTION_REVOKED - A subscription has been revoked from the user before the expiration time.
// (13) SUBSCRIPTION_EXPIRED - A subscription has expired.

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionNotification {
    pub version: String,
    pub notification_type: u32,
    pub purchase_token: String,
    pub subscription_id: String,
}

#[derive(Debug)]
pub enum NotificationType {
    SubscriptionRecovered,
    SubscriptionRenewed,
    SubscriptionCanceled,
    SubscriptionPurchased,
    SubscriptionOnHold,
    SubscriptionInGracePeriod,
    SubscriptionRestarted,
    SubscriptionPriceChangeConfirmed,
    SubscriptionDeferred,
    SubscriptionPaused,
    SubscriptionPausedScheduleChanged,
    SubscriptionRevoked,
    SubscriptionExpired,
    Unknown,
}

impl SubscriptionNotification {
    pub fn notification_type(&self) -> NotificationType {
        match self.notification_type {
            1 => NotificationType::SubscriptionRecovered,
            2 => NotificationType::SubscriptionRenewed,
            3 => NotificationType::SubscriptionCanceled,
            4 => NotificationType::SubscriptionPurchased,
            5 => NotificationType::SubscriptionOnHold,
            6 => NotificationType::SubscriptionInGracePeriod,
            7 => NotificationType::SubscriptionRestarted,
            8 => NotificationType::SubscriptionPriceChangeConfirmed,
            9 => NotificationType::SubscriptionDeferred,
            10 => NotificationType::SubscriptionPaused,
            11 => NotificationType::SubscriptionPausedScheduleChanged,
            12 => NotificationType::SubscriptionRevoked,
            13 => NotificationType::SubscriptionExpired,
            _ => NotificationType::Unknown,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OneTimeProductNotification {
    pub version: String,
    pub notification_type: u32,
    pub purchase_token: String,
    pub sku: String,
}

#[derive(Debug, Deserialize)]
pub struct TestNotification {
    pub version: String,
}
