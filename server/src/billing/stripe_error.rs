use tracing::*;

/// A failed Stripe call. The server reports these as internal errors; the
/// buyer only ever sees Stripe's own pages.
#[derive(Debug)]
pub struct SimplifiedStripeError(pub String);

impl From<stripe::StripeError> for SimplifiedStripeError {
    fn from(error: stripe::StripeError) -> Self {
        debug!(?error, "Stripe error");
        SimplifiedStripeError(format!("{error:?}"))
    }
}
