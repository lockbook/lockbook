use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PubSubNotification {
    pub message: PubSubMessage,
}

#[derive(Debug, Deserialize)]
pub struct PubSubMessage {
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeveloperNotification {
    pub version: String,
    pub one_time_product_notification: Option<OneTimeProductNotification>,
    pub subscription_notification: Option<SubscriptionNotification>,
    pub test_notification: Option<TestNotification>,
}

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
