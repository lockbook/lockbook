package app.lockbook.ui

import android.content.Context
import android.content.Intent
import android.util.AttributeSet
import android.view.View
import android.widget.Button
import android.widget.TextView
import androidx.core.net.toUri
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.billing.BillingClientLifecycle.Companion.SUBSCRIPTION_URI
import app.lockbook.util.*
import net.lockbook.SubscriptionInfo
import net.lockbook.SubscriptionInfo.AppStore
import net.lockbook.SubscriptionInfo.GooglePlay
import net.lockbook.SubscriptionInfo.PaymentPlatform
import net.lockbook.SubscriptionInfo.Stripe
import java.text.SimpleDateFormat
import java.util.*

class SubscriptionInfoPreference(context: Context, attributeSet: AttributeSet?) : Preference(context, attributeSet) {

    private lateinit var subscriptionInfo: TextView
    private lateinit var paymentIssue: TextView
    private lateinit var solvePaymentIssue: Button

    init {
        layoutResource = R.layout.preference_subscription_info
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder) {
        super.onBindViewHolder(holder)

        subscriptionInfo = holder.itemView.findViewById(R.id.subscription_info)
        paymentIssue = holder.itemView.findViewById(R.id.payment_issue_text)
        solvePaymentIssue = holder.itemView.findViewById(R.id.solve_payment_issue)

        solvePaymentIssue.setOnClickListener {
            getSettingsFragment().requireActivity().startActivity(Intent(Intent.ACTION_VIEW, SUBSCRIPTION_URI.toUri()))
        }

        getSettingsFragment().model.determineSettingsInfo.observe(getSettingsFragment()) { settingsInfo ->
            setUpSubscriptionInfoPreference(settingsInfo.subscriptionInfo)
        }
    }

    private fun setUpSubscriptionInfoPreference(maybeSubscriptionInfo: SubscriptionInfo?) {
        if (maybeSubscriptionInfo != null) {
            val renewalOrExpirationText = when (maybeSubscriptionInfo.paymentPlatform) {
                is GooglePlay -> {
                    when ((maybeSubscriptionInfo.paymentPlatform as GooglePlay).accountState) {
                        GooglePlay.GooglePlayAccountState.Canceled -> {
                            context.resources.getString(R.string.expiration_day)
                        }
                        GooglePlay.GooglePlayAccountState.GracePeriod -> {
                            context.resources.getString(R.string.grace_period)
                        }
                        GooglePlay.GooglePlayAccountState.OnHold, GooglePlay.GooglePlayAccountState.Ok -> {
                            context.resources.getString(R.string.next_renewal_day)
                        }
                    }
                }
                is Stripe -> {
                    context.resources.getString(R.string.next_renewal_day)
                }
                is AppStore -> {
                    when ((maybeSubscriptionInfo.paymentPlatform as AppStore).accountState) {
                        AppStore.AppStoreAccountState.Ok -> context.resources.getString(R.string.next_renewal_day)
                        AppStore.AppStoreAccountState.GracePeriod -> context.resources.getString(R.string.grace_period)
                        AppStore.AppStoreAccountState.FailedToRenew, AppStore.AppStoreAccountState.Expired -> context.resources.getString(R.string.expiration_day)
                    }
                }
                else -> context.resources.getString(R.string.basic_error)
            }.bold()

            val accountState = (maybeSubscriptionInfo.paymentPlatform as? GooglePlay)?.accountState

            val gracePeriodViewsVisibility = if (accountState == GooglePlay.GooglePlayAccountState.GracePeriod) {
                View.VISIBLE
            } else {
                View.GONE
            }

            paymentIssue.visibility = gracePeriodViewsVisibility
            solvePaymentIssue.visibility = gracePeriodViewsVisibility

            context.resources.apply {
                subscriptionInfo.text = spannable {
                    getString(R.string.payment_platform).bold() + " " +
                        maybeSubscriptionInfo.paymentPlatform.toReadableString() + "\n" +
                        renewalOrExpirationText + " " + SimpleDateFormat(
                        "yyyy-MM-dd",
                        Locale.getDefault()
                    ).format(Date(maybeSubscriptionInfo.periodEnd))
                }
            }
        }
    }
}

fun PaymentPlatform.toReadableString(): String = when (this) {
    is Stripe -> "Stripe"
    is GooglePlay -> "Google Play"
    is AppStore -> "App Store"
    else -> "Unknown"
}
