package app.lockbook.billing

import android.app.Activity
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LiveData
import net.lockbook.LbError

interface BillingManager : DefaultLifecycleObserver {
    val billingEvent: LiveData<BillingEvent>
    val premiumPrice: LiveData<String>

    fun launchBillingFlow(activity: Activity)

    fun showInAppMessaging(activity: Activity) {}
}

sealed class BillingEvent {
    data class GooglePlayPurchase(
        val purchaseToken: String,
        val accountId: String,
    ) : BillingEvent()

    data object SuccessfulPurchase : BillingEvent()

    data class NotifyError(
        val error: LbError,
    ) : BillingEvent()

    data class NotifyErrorMsg(
        val error: String,
    ) : BillingEvent()

    data object NotifyUnrecoverableError : BillingEvent()
}

const val GOOGLE_PLAY_SUBSCRIPTION_URI =
    "https://play.google.com/store/account/subscriptions?sku=app.lockbook.premium_subscription&package=app.lockbook"
