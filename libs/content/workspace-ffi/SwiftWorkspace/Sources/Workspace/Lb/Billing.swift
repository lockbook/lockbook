import Bridge

public struct SubscriptionInfo {
    public let periodEnd: UInt64
    public let platform: PaymentPlatform

    init(_ res: LbSubscriptionInfo) {
        periodEnd = res.period_end

        if res.app_store != nil {
            let state: AppStoreBillingState = if res.app_store.pointee.is_state_ok {
                .ok
            } else if res.app_store.pointee.is_state_grace_period {
                .gracePeriod
            } else if res.app_store.pointee.is_state_failed_to_renew {
                .failedToRenew
            } else {
                .expired
            }

            platform = .appStore(state: state)
        } else if res.google_play != nil {
            let state: GooglePlayBillingState = if res.google_play.pointee.is_state_ok {
                .ok
            } else if res.google_play.pointee.is_state_canceled {
                .canceled
            } else if res.google_play.pointee.is_state_grace_period {
                .gracePeriod
            } else {
                .onHold
            }

            platform = .googlePlay(state: state)
        } else {
            platform = .stripe(cardLast4Digits: String(cString: res.stripe.pointee.card_last_4_digits))
        }
    }

    public func isPremium() -> Bool {
        switch platform {
        case let .stripe(cardLast4Digits):
            true
        case let .googlePlay(state):
            state == .ok || state == .gracePeriod || state == .canceled
        case let .appStore(state):
            state == .ok || state == .gracePeriod
        }
    }
}

public enum PaymentPlatform {
    case stripe(cardLast4Digits: String)
    case googlePlay(state: GooglePlayBillingState)
    case appStore(state: AppStoreBillingState)
}

public enum GooglePlayBillingState {
    case ok
    case canceled
    case gracePeriod
    case onHold
}

public enum AppStoreBillingState {
    case ok
    case gracePeriod
    case failedToRenew
    case expired
}
