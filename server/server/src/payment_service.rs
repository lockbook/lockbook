use stripe::CreatePaymentMethod;
use crate::ServerError::InternalError;
use crate::{RequestContext, ServerError};
use lockbook_models::api::{RegisterCreditCard, RegisterCreditCardError};

pub async fn register_for_stripe(
    context: RequestContext<'_, RegisterCreditCard>,
) -> Result<(), ServerError<RegisterCreditCardError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(InternalError(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    stripe::PaymentMethod::create(&server_state.stripe_client, CreatePaymentMethod {
        afterpay_clearpay: None,
        alipay: None,
        au_becs_debit: None,
        bacs_debit: None,
        bancontact: None,
        billing_details: None,
        customer: None,
        eps: None,
        expand: &[],
        fpx: None,
        giropay: None,
        grabpay: None,
        ideal: None,
        interac_present: None,
        metadata: None,
        oxxo: None,
        p24: None,
        payment_method: None,
        sepa_debit: None,
        sofort: None,
        type_: None
    })

    Ok(())
}
