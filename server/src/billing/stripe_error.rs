use tracing::*;

/// A failed Stripe call. The server reports these as internal errors; the
/// buyer only ever sees Stripe's own pages.
#[derive(Debug)]
pub struct SimplifiedStripeError(pub String);

impl From<reqwest::Error> for SimplifiedStripeError {
    fn from(error: reqwest::Error) -> Self {
        debug!(?error, "Stripe HTTP error");
        SimplifiedStripeError(format!("{error}"))
    }
}
