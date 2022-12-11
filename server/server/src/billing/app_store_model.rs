use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct VerifyReceiptResponse {
    pub latest_receipt_info: Option<Vec<ReceiptInfo>>,
    pub status: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ReceiptInfo {
    pub app_account_token: String,
    pub expires_date_ms: String,
    pub original_transaction_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct VerifyReceiptRequest {
    #[serde(rename = "receipt-data")]
    pub encoded_receipt: String,
    pub password: String,
    pub exclude_old_transactions: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EncodedNotificationResponseBody {
    pub signed_payload: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NotificationResponseBody {
    pub notification_type: NotificationChange,
    pub subtype: Option<Subtype>,
    pub data: NotificationData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NotificationData {
    pub bundle_id: String,
    pub environment: Environment,
    #[serde(rename = "signedRenewalInfo")]
    pub encoded_renewal_info: Option<String>,
    #[serde(rename = "signedTransactionInfo")]
    pub encoded_transaction_info: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfo {
    pub bundle_id: String,
    pub product_id: String,
    pub app_account_token: String,
    pub expires_date: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Environment {
    Sandbox,
    Production,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
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
    Accepted,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationChange {
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
    Test,
}
