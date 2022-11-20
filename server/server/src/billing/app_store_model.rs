use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct VerifyReceiptResponse {
    #[serde(rename = "latestReceipt")]
    pub encoded_latest_receipt: String
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReceiptInfo {
    pub app_account_token: String,
    pub expires_date_ms: i64,
    pub cancellation_date_ms: i64,
    pub original_transaction_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="kebab-case")]
pub struct VerifyReceiptRequest {
    #[serde(rename = "receipt-data")]
    pub encoded_receipt: String,
    pub password: String,
    pub exclude_old_transactions: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct EncodedNotificationResponseBody {
    pub signed_payload: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct NotificationResponseBody {
    pub notification_type: SubscriptionChange,
    pub subtype: Subtype,
    pub data: NotificationData,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct NotificationData {
    pub bundle_id: String,
    pub environment: Environment,
    #[serde(rename = "signedRenewalInfo")]
    pub encoded_renewal_info: String,
    #[serde(rename = "signedTransactionInfo")]
    pub encoded_transaction_info: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="camelCase")]
pub struct TransactionInfo {
    pub bundle_id: String,
    pub product_id: String,
    pub app_account_token: String,
    pub revocation_date: u64,
    pub revocation_reason: u8,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Environment {
    Sandbox,
    Production
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum Subtype {
    InitialBuy,
    Resubscribe,
    Downgrade,
    Upgrade,
    AutoRenewEnabled,
    AutoRenewDisabled,
    Voluntary,
    BillingRetry,
    PriceIncrease,
    GracePeriod,
    BillingRecovery,
    Pending,
    Accepted
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum SubscriptionChange {
    ConsumptionRequest,
    DidChangeRenewalPref,
    DidChangeRenewalStatus,
    DidFailToRenew,
    DidRenew,
    Expired,
    GracePeriodExpired,
    OfferRedeemed,
    PriceIncrease,
    Refund,
    RefundDeclined,
    RenewalExtended,
    Revoke,
    Subscribed,
    Test
}
