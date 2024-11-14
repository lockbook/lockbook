package net.lockbook;

// make every inner class static
public class SubscriptionInfo {
    public PaymentPlatform paymentPlatform;
    public long periodEnd;
    
    public static interface PaymentPlatform {}

    public static class Stripe implements PaymentPlatform {
        public String cardLast4Digits;
    }

    public static class GooglePlay implements PaymentPlatform {
        public GooglePlayAccountState accountState;

        public enum GooglePlayAccountState {
            Ok,
            Canceled,
            GracePeriod,
            OnHold
        }
    }

    public static class AppStore implements PaymentPlatform {
        public AppStoreAccountState accountState;

        public enum AppStoreAccountState {
            Ok,
            GracePeriod,
            FailedToRenew,
            Expired
        }
    }
}
