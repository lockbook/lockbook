#[cfg(test)]
mod get_credit_card_test {
    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{
        generate_account, generate_premium_account_tier, generate_root_metadata, test_credit_cards,
    };
    use lockbook_models::api::*;

    #[test]
    fn get_credit_card() {
        // create_account
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // switch account tier to premium
        api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: generate_premium_account_tier(
                    test_credit_cards::GOOD,
                    None,
                    None,
                    None,
                ),
            },
        )
        .unwrap();

        // get the last 4 digits of the most recently added card
        let result = api_service::request(&account, GetCreditCardRequest {})
            .unwrap()
            .credit_card_last_4_digits;

        assert_matches!(result.as_str(), test_credit_cards::GOOD_LAST_4);
    }

    #[test]
    fn not_a_stripe_customer() {
        // new account
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        // attempt to get the last 4 digits of the most recently added card
        let result = api_service::request(&account, GetCreditCardRequest {});

        assert_matches!(
            result,
            Err(ApiError::<GetCreditCardError>::Endpoint(GetCreditCardError::NotAStripeCustomer))
        );
    }
}
