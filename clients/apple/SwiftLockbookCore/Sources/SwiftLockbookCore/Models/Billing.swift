import Foundation

public struct SubscriptionInfo: Codable {
    public let paymentPlatform: PaymentPlatform
    public let periodEnd: UInt64
}

public enum PaymentPlatform: Codable {
    case Stripe(cardLast4Digits: String)
    case GooglePlay(accountState: GooglePlayAccountState)
    case AppStore(accountState: AppStoreAccountState)
}

public enum GooglePlayAccountState: String, Codable {
    case Ok
    case Canceled
    case GracePeriod
    case OnHold

}

public enum AppStoreAccountState: String, Codable {
    case Ok
    case GracePeriod
    case FailedToRenew

}
