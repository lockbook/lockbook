mod integration_test;

#[cfg(test)]
mod switch_account_tier_test {
    use chrono::Datelike;
    use lockbook_core::assert_matches;
    use lockbook_core::service::api_service;
    use lockbook_core::service::api_service::ApiError;
    use lockbook_core::service::test_utils::{generate_account, generate_root_metadata};
    use lockbook_models::api::*;

    mod test_credit_cards {
        pub const NO_AUTHENTICATION: &str = "4242424242424242";
        pub const INVALID_NUMBER: &str = "11111";

        pub mod decline {
            pub const GENERIC: &str = "4000000000000002";
            pub const INSUFFICIENT_FUNDS: &str = "4000000000009995";
            pub const LOST_CARD: &str = "4000000000009987";
            pub const EXPIRED_CARD: &str = "4000000000000069";
            pub const INCORRECT_CVC: &str = "4000000000000127"; // incorrect cvc for card resulting in decline
            pub const PROCESSING_ERROR: &str = "4000000000000119";
            pub const INCORRECT_NUMBER: &str = "4242424242424241";
        }
    }

    mod test_card_info {
        pub const GENERIC_CVC: &str = "314";
        pub const GENERIC_EXP_MONTH: &str = "8";
    }

    fn get_next_year() -> String {
        (chrono::Utc::now().year() + 1).to_string()
    }

    fn generate_monthly_account_tier(card_number: &str, maybe_exp_year: Option<&str>, maybe_exp_month: Option<&str>, maybe_cvc: Option<&str>) -> AccountTier {
        AccountTier::Monthly(CardChoice::NewCard {
            number: card_number.to_string(),
            exp_year: match maybe_exp_year {
                None => get_next_year(),
                Some(exp_year) => exp_year.to_string()
            },
            exp_month: match maybe_exp_month {
                None => test_card_info::GENERIC_EXP_MONTH,
                Some(exp_month) => exp_month
            }.to_string(),
            cvc: match maybe_cvc {
                None => test_card_info::GENERIC_CVC,
                Some(cvc) => cvc
            }.to_string(),
        })
    }

    #[test]
    fn switch_account_tier_to_premium_and_back() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);

        println!("THIS: {:?}", generate_monthly_account_tier(test_credit_cards::NO_AUTHENTICATION, None, None, None));

        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        api_service::request(&account, SwitchAccountTierRequest {
            account_tier: generate_monthly_account_tier(test_credit_cards::NO_AUTHENTICATION, None, None, None)
        }).unwrap();

        api_service::request(&account, SwitchAccountTierRequest {
            account_tier: AccountTier::Free
        }).unwrap();
    }

    #[test]
    fn switch_account_tier_new_tier_is_old_tier_free() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let result = api_service::request(&account, SwitchAccountTierRequest {
            account_tier: AccountTier::Free
        });

        assert_matches!(
            result,
            Err(ApiError::<SwitchAccountTierError>::Endpoint(SwitchAccountTierError::NewTierIsOldTier))
        );
    }

    #[test]
    fn switch_account_tier_new_tier_is_old_tier_paid() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        api_service::request(&account, SwitchAccountTierRequest {
            account_tier: generate_monthly_account_tier(
                test_credit_cards::NO_AUTHENTICATION,
                None,
                None,
                None
            )
        }).unwrap();

        let result = api_service::request(&account, SwitchAccountTierRequest {
            account_tier: generate_monthly_account_tier(test_credit_cards::NO_AUTHENTICATION, None, None, None)
        });

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

        let result = api_service::request(&account, SwitchAccountTierRequest {
            account_tier: AccountTier::Monthly(CardChoice::OldCard)
        });

        assert_matches!(
            result,
            Err(ApiError::<SwitchAccountTierError>::Endpoint(SwitchAccountTierError::PreexistingCardDoesNotExist))
        );
    }

    #[test]
    fn switch_account_tier_decline() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let scenarios = vec![
            (test_credit_cards::decline::GENERIC, SwitchAccountTierError::CardDeclined(CardDeclinedType::Generic)),
            (test_credit_cards::decline::LOST_CARD, SwitchAccountTierError::CardDeclined(CardDeclinedType::Generic)), // core should not be informed a card is stolen (at least the user)
            (test_credit_cards::decline::INSUFFICIENT_FUNDS, SwitchAccountTierError::CardDeclined(CardDeclinedType::BalanceOrCreditExceeded)),
            (test_credit_cards::decline::PROCESSING_ERROR, SwitchAccountTierError::CardDeclined(CardDeclinedType::TryAgain)),
            (test_credit_cards::decline::EXPIRED_CARD, SwitchAccountTierError::CardDeclined(CardDeclinedType::ExpiredCard)),
            (test_credit_cards::decline::INCORRECT_NUMBER, SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectNumber)),
            (test_credit_cards::decline::INCORRECT_CVC, SwitchAccountTierError::CardDeclined(CardDeclinedType::IncorrectCVC)),
        ];

        for (card_number, error) in scenarios {
            let result = api_service::request(&account, SwitchAccountTierRequest {
                account_tier: generate_monthly_account_tier(card_number, None, None, None)
            });

            assert_matches!(
                result,
                Err(ApiError::<SwitchAccountTierError>::Endpoint(
                    error
                ))
            );
        }
    }

    #[test]
    fn switch_account_tier_invalid_card() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);
        api_service::request(&account, NewAccountRequest::new(&account, &root)).unwrap();

        let scenarios = vec![
            (test_credit_cards::INVALID_NUMBER, None, None, None, SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::Number)),
            (test_credit_cards::NO_AUTHENTICATION, Some("1970"), None, None, SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::ExpYear)),
            (test_credit_cards::NO_AUTHENTICATION, None, Some("14"), None, SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::ExpMonth)),
            (test_credit_cards::NO_AUTHENTICATION, None, None, Some("11"), SwitchAccountTierError::InvalidCreditCard(InvalidCreditCardType::CVC))
        ];

        for (card_number, maybe_exp_year, maybe_exp_month, maybe_cvc, error) in scenarios {
            let result = api_service::request(&account, SwitchAccountTierRequest {
                account_tier: generate_monthly_account_tier(card_number, maybe_exp_year, maybe_exp_month, maybe_cvc)
            });

            assert_matches!(
                result,
                Err(ApiError::<SwitchAccountTierError>::Endpoint(
                    error
                ))
            );
        }
    }
}
