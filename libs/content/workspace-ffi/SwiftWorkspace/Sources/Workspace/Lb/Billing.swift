import Bridge

public struct SubscriptionInfo {
    public let periodEnd: UInt64
    public let platform: PaymentPlatform
    
    init(_ res: LbSubscriptionInfo) {
        self.periodEnd = res.period_end
        
        if res.app_store != nil {
            let state: AppStoreBillingState
            
            if res.app_store.pointee.is_state_ok {
                state = .ok
            } else if res.app_store.pointee.is_state_grace_period {
                state = .gracePeriod
            } else if res.app_store.pointee.is_state_failed_to_renew {
                state = .failedToRenew
            } else {
                state = .expired
            }
            
            platform = .appStore(state: state)
        } else if res.google_play != nil {
            let state: GooglePlayBillingState
            
            if res.google_play.pointee.is_state_ok {
                state = .ok
            } else if res.google_play.pointee.is_state_canceled {
                state = .canceled
            } else if res.google_play.pointee.is_state_grace_period {
                state = .gracePeriod
            } else {
                state = .onHold
            }
            
            platform = .googlePlay(state: state)
        } else {
            platform = .stripe(cardLast4Digits: String(cString: res.stripe.pointee.card_last_4_digits))
        }
    }
    
    public func isPremium() -> Bool {
        switch self.platform {
        case .stripe(let cardLast4Digits):
            return true
        case .googlePlay(let state):
            return state == .ok || state == .gracePeriod || state == .canceled
        case .appStore(let state):
            return state == .ok || state == .gracePeriod
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
