//! Plan standing for Settings and the sidebar usage bar.
//!
//! Stripe’s public `SubscriptionInfo` has **no** cancel flag. After cancel the
//! server still returns a Stripe row, but `data_cap` drops to free
//! (`StripeAccountState::Canceled` → free tier). Always pass `get_usage` cap.

use lb::model::api::{
    AppStoreAccountState, FREE_TIER_USAGE_SIZE, GooglePlayAccountState, PaymentPlatform,
    SubscriptionInfo,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountTier {
    Free,
    Premium,
}

impl AccountTier {
    pub fn label(self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Premium => "Premium",
        }
    }
}

/// Display standing derived from subscription + server data cap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountStanding {
    pub tier: AccountTier,
    /// e.g. "Renews on August 20, 2026", "Canceled", "Access continues until…".
    pub detail: Option<String>,
    /// Billing source for the payment line (not last-4 for privacy in some UIs).
    pub source: Option<&'static str>,
    /// Card last-4 when Stripe (for payment row).
    pub card_last_4: Option<String>,
}

/// Local calendar date for a subscription period end.
///
/// Stripe writes `current_period_end` in **seconds**; Google Play / App Store
/// use **millis**. ≥ 1e12 → millis, else seconds.
pub fn format_period_end(period_end: u64) -> String {
    use chrono::Local;
    period_end_datetime(period_end)
        .map(|utc| utc.with_timezone(&Local).format("%B %d, %Y").to_string())
        .unwrap_or_else(|| "the end of the billing period".into())
}

fn period_end_datetime(period_end: u64) -> Option<chrono::DateTime<chrono::Utc>> {
    if period_end >= 1_000_000_000_000 {
        chrono::DateTime::from_timestamp_millis(period_end as i64)
    } else {
        chrono::DateTime::from_timestamp(period_end as i64, 0)
    }
}

impl AccountStanding {
    pub fn from_subscription_and_cap(
        info: Option<&SubscriptionInfo>, data_cap_exact: Option<u64>,
    ) -> Self {
        let cap_is_free = data_cap_exact.is_some_and(|c| c <= FREE_TIER_USAGE_SIZE);

        let Some(info) = info else {
            return Self { tier: AccountTier::Free, detail: None, source: None, card_last_4: None };
        };

        let date = format_period_end(info.period_end);
        let renews = if date == "the end of the billing period" {
            "Renews at the end of the billing period.".to_owned()
        } else {
            format!("Renews on {date}.")
        };
        let access_until = if date == "the end of the billing period" {
            "Access continues until the period ends.".to_owned()
        } else {
            format!("Access continues until {date}.")
        };

        match &info.payment_platform {
            // No account_state on the public Stripe payload — use data cap.
            PaymentPlatform::Stripe { card_last_4_digits } => {
                let last4 = Some(card_last_4_digits.clone());
                if cap_is_free {
                    Self {
                        tier: AccountTier::Free,
                        detail: Some("Canceled".into()),
                        source: Some("Stripe"),
                        card_last_4: last4,
                    }
                } else {
                    Self {
                        tier: AccountTier::Premium,
                        detail: Some(renews),
                        source: Some("Stripe"),
                        card_last_4: last4,
                    }
                }
            }

            PaymentPlatform::GooglePlay { account_state } => match account_state {
                GooglePlayAccountState::Ok if !cap_is_free => Self {
                    tier: AccountTier::Premium,
                    detail: Some(renews),
                    source: Some("Google Play"),
                    card_last_4: None,
                },
                GooglePlayAccountState::Ok => Self {
                    tier: AccountTier::Free,
                    detail: Some("Canceled".into()),
                    source: Some("Google Play"),
                    card_last_4: None,
                },
                GooglePlayAccountState::Canceled if !cap_is_free => Self {
                    tier: AccountTier::Premium,
                    detail: Some(access_until),
                    source: Some("Google Play"),
                    card_last_4: None,
                },
                GooglePlayAccountState::Canceled => Self {
                    tier: AccountTier::Free,
                    detail: Some(access_until),
                    source: Some("Google Play"),
                    card_last_4: None,
                },
                GooglePlayAccountState::GracePeriod => Self {
                    tier: AccountTier::Premium,
                    detail: Some(access_until),
                    source: Some("Google Play"),
                    card_last_4: None,
                },
                GooglePlayAccountState::OnHold => Self {
                    tier: AccountTier::Free,
                    detail: Some("On hold".into()),
                    source: Some("Google Play"),
                    card_last_4: None,
                },
            },

            PaymentPlatform::AppStore { account_state } => match account_state {
                AppStoreAccountState::Ok if !cap_is_free => Self {
                    tier: AccountTier::Premium,
                    detail: Some(renews),
                    source: Some("App Store"),
                    card_last_4: None,
                },
                AppStoreAccountState::Ok => Self {
                    tier: AccountTier::Free,
                    detail: None,
                    source: Some("App Store"),
                    card_last_4: None,
                },
                AppStoreAccountState::GracePeriod => Self {
                    tier: AccountTier::Premium,
                    detail: Some(access_until),
                    source: Some("App Store"),
                    card_last_4: None,
                },
                AppStoreAccountState::FailedToRenew => Self {
                    tier: AccountTier::Free,
                    detail: Some("Failed to renew".into()),
                    source: Some("App Store"),
                    card_last_4: None,
                },
                AppStoreAccountState::Expired => Self {
                    tier: AccountTier::Free,
                    detail: Some("Expired".into()),
                    source: Some("App Store"),
                    card_last_4: None,
                },
            },
        }
    }

    pub fn payment_line(&self) -> Option<String> {
        match (self.source, &self.card_last_4) {
            (Some("Stripe"), Some(last4)) => Some(format!("Stripe · {last4}")),
            (Some(src), _) => Some((*src).to_owned()),
            _ => None,
        }
    }

    /// True when cancel is a meaningful action (premium / still billed).
    pub fn can_cancel(&self) -> bool {
        self.tier == AccountTier::Premium
            && !matches!(
                self.detail.as_deref(),
                Some(d) if d.starts_with("Access continues") || d == "Canceled"
            )
    }
}
