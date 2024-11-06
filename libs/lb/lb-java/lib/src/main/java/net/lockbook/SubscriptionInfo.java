package net.lockbook;

// make every inner class static
public class SubscriptionInfo {
    PaymentPlatform paymentPlatform;
    long periodEnd;
    
    public static interface PaymentPlatform {}

    public static class Stripe implements PaymentPlatform {
        String cardLast4Digits;
    }

    public static class GooglePlay implements PaymentPlatform {
        GooglePlayAccountState accountState;

        public enum GooglePlayAccountState {
            Ok,
            Canceled,
            GracePeriod,
            OnHold
        }
    }

    public static class AppStore implements PaymentPlatform {
        AppStoreAccountState accountState;

        public enum AppStoreAccountState {
            Ok,
            GracePeriod,
            FailedToRenew,
            Expired
        }
    }
}
