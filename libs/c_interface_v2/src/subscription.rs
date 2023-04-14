use lockbook_core::{PaymentMethod, PaymentPlatform, StripeAccountTier};

use crate::*;

#[repr(C)]
pub struct LbSubInfo {
    stripe_last4: *mut c_char,
    google_play_state: u8,
    app_store_state: u8,
    period_end: u64,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_sub_info_free(si: LbSubInfo) {
    libc::free(si.stripe_last4 as *mut c_void);
}

#[repr(C)]
pub struct LbSubInfoResult {
    ok: LbSubInfo,
    err: LbError,
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn lb_sub_info_result_free(r: LbSubInfoResult) {
    if r.err.code == LbErrorCode::Success {
        lb_sub_info_free(r.ok);
    } else {
        lb_error_free(r.err);
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_sub_info_result_free` to avoid a memory leak.
/// Alternatively, the `ok` value or `err` value can be passed to `lb_sub_info_free` or
/// `lb_error_free` respectively depending on whether there's an error or not.
#[no_mangle]
pub unsafe extern "C" fn lb_get_subscription_info(core: *mut c_void) -> LbSubInfoResult {
    let mut r = LbSubInfoResult {
        ok: LbSubInfo {
            stripe_last4: null_mut(),
            google_play_state: 0,
            app_store_state: 0,
            period_end: 0,
        },
        err: lb_error_none(),
    };
    match core!(core).get_subscription_info() {
        Ok(None) => {} // Leave zero values for no subscription info.
        Ok(Some(info)) => {
            use PaymentPlatform::*;
            match info.payment_platform {
                Stripe { card_last_4_digits } => r.ok.stripe_last4 = cstr(card_last_4_digits),
                // The integer representations of both the google play and app store account
                // state enums are bound together by a unit test in this crate.
                GooglePlay { account_state } => r.ok.google_play_state = account_state as u8 + 1,
                AppStore { account_state } => r.ok.app_store_state = account_state as u8 + 1,
            }
            r.ok.period_end = info.period_end;
        }
        Err(err) => r.err = lberr(err),
    }
    r
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_upgrade_account_stripe_old_card(core: *mut c_void) -> LbError {
    match core!(core).upgrade_account_stripe(StripeAccountTier::Premium(PaymentMethod::OldCard)) {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_upgrade_account_stripe_new_card(
    core: *mut c_void,
    num: *const c_char,
    exp_year: i32,
    exp_month: i32,
    cvc: *const c_char,
) -> LbError {
    let mut e = lb_error_none();
    let number = rstr(num).to_string();
    let cvc = rstr(cvc).to_string();

    if let Err(err) =
        core!(core).upgrade_account_stripe(StripeAccountTier::Premium(PaymentMethod::NewCard {
            number,
            exp_year,
            exp_month,
            cvc,
        }))
    {
        e = lberr(err);
    }
    e
}

/// # Safety
///
/// The returned value must be passed to `lb_error_free` to avoid a memory leak.
#[no_mangle]
pub unsafe extern "C" fn lb_cancel_subscription(core: *mut c_void) -> LbError {
    match core!(core).cancel_subscription() {
        Ok(()) => lb_error_none(),
        Err(err) => lberr(err),
    }
}
