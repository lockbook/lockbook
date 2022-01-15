mod integration_test;

#[cfg(test)]
mod switch_account_tier_test {
    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{
        generate_account, generate_monthly_account_tier, generate_root_metadata, test_credit_cards,
    };
    use lockbook_models::api::*;

    #[test]
    fn switch_account_tier_to_premium_and_back() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);

        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: generate_monthly_account_tier(
                    test_credit_cards::NO_AUTHENTICATION,
                    None,
                    None,
                    None,
                ),
            },
        )
        .unwrap();

        api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: AccountTier::Free,
            },
        )
        .unwrap();
    }

    #[test]
    fn switch_account_tier_new_tier_is_old_tier_free() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let result = api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: AccountTier::Free,
            },
        );

        assert_matches!(
            result,
            Err(ApiError::<SwitchAccountTierError>::Endpoint(
                SwitchAccountTierError::NewTierIsOldTier
            ))
        );
    }

    #[test]
    fn switch_account_tier_new_tier_is_old_tier_paid() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: generate_monthly_account_tier(
                    test_credit_cards::NO_AUTHENTICATION,
                    None,
                    None,
                    None,
                ),
            },
        )
        .unwrap();

        let result = api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: generate_monthly_account_tier(
                    test_credit_cards::NO_AUTHENTICATION,
                    None,
                    None,
                    None,
                ),
            },
        );

        assert_matches!(
            result,
            Err(ApiError::<SwitchAccountTierError>::Endpoint(
                SwitchAccountTierError::NewTierIsOldTier
            ))
        );
    }

    #[test]
    fn switch_account_tier_preexisting_card_does_not_exist() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let result = api_service::request(
            &account,
            SwitchAccountTierRequest {
                account_tier: AccountTier::Monthly(CardChoice::OldCard),
            },
        );

        assert_matches!(
            result,
            Err(ApiError::<SwitchAccountTierError>::Endpoint(
                SwitchAccountTierError::OldCardDoesNotExist
            ))
        );
    }

    #[test]
    fn switch_account_tier_decline() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let scenarios = vec![
            (
                test_credit_cards::decline::GENERIC,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::Generic),
            ),
            (
                test_credit_cards::decline::LOST_CARD,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::Generic),
            ), // core should not be informed a card is stolen or lost (at least the user)
            (
                test_credit_cards::decline::INSUFFICIENT_FUNDS,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::BalanceOrCreditExceeded),
            ),
            (
                test_credit_cards::decline::PROCESSING_ERROR,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::TryAgain),
            ),
            (
                test_credit_cards::decline::EXPIRED_CARD,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::ExpiredCard),
            ),
            (
                test_credit_cards::decline::INCORRECT_NUMBER,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectNumber),
            ),
            (
                test_credit_cards::decline::INCORRECT_CVC,
                SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectCVC),
            ),
        ];

        for (card_number, error) in scenarios {
            let result = api_service::request(
                &account,
                SwitchAccountTierRequest {
                    account_tier: generate_monthly_account_tier(card_number, None, None, None),
                },
            );

            assert_matches!(
                result,
                Err(ApiError::<SwitchAccountTierError>::Endpoint(error))
            );
        }
    }

    #[test]
    fn switch_account_tier_invalid_card() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let scenarios = vec![
            (
                test_credit_cards::INVALID_NUMBER,
                None,
                None,
                None,
                SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::Number),
            ),
            (
                test_credit_cards::NO_AUTHENTICATION,
                Some("1970"),
                None,
                None,
                SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::ExpYear),
            ),
            (
                test_credit_cards::NO_AUTHENTICATION,
                None,
                Some("14"),
                None,
                SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::ExpMonth),
            ),
            (
                test_credit_cards::NO_AUTHENTICATION,
                None,
                None,
                Some("11"),
                SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::CVC),
            ),
        ];

        for (card_number, maybe_exp_year, maybe_exp_month, maybe_cvc, error) in scenarios {
            let result = api_service::request(
                &account,
                SwitchAccountTierRequest {
                    account_tier: generate_monthly_account_tier(
                        card_number,
                        maybe_exp_year,
                        maybe_exp_month,
                        maybe_cvc,
                    ),
                },
            );

            assert_matches!(
                result,
                Err(ApiError::<SwitchAccountTierError>::Endpoint(error))
            );
        }
    }
}
