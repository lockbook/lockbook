package app.lockbook.ui

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.AttributeSet
import android.view.View
import android.widget.Button
import android.widget.TextView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.billing.BillingClientLifecycle.Companion.SUBSCRIPTION_URI
import app.lockbook.util.*
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
            getSettingsFragment().requireActivity().startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(SUBSCRIPTION_URI)))
        }

        getSettingsFragment().model.determineSettingsInfo.observe(getSettingsFragment()) { settingsInfo ->
            setUpSubscriptionInfoPreference(settingsInfo.subscriptionInfo)
        }
    }

    private fun setUpSubscriptionInfoPreference(maybeSubscriptionInfo: SubscriptionInfo?) {
        if (maybeSubscriptionInfo != null) {
            val renewalOrExpirationText = when (maybeSubscriptionInfo.paymentPlatform) {
                is PaymentPlatform.GooglePlay -> {
                    when (maybeSubscriptionInfo.paymentPlatform.accountState) {
                        GooglePlayAccountState.Canceled -> {
                            context.resources.getString(R.string.expiration_day)
                        }
                        GooglePlayAccountState.GracePeriod -> {
                            context.resources.getString(R.string.grace_period)
                        }
                        GooglePlayAccountState.OnHold, GooglePlayAccountState.Ok -> {
                            context.resources.getString(R.string.next_renewal_day)
                        }
                    }
                }
                is PaymentPlatform.Stripe -> {
                    context.resources.getString(R.string.next_renewal_day)
                }
                is PaymentPlatform.AppStore -> {
                    when (maybeSubscriptionInfo.paymentPlatform.accountState) {
                        AppStoreAccountState.Ok -> context.resources.getString(R.string.next_renewal_day)
                        AppStoreAccountState.GracePeriod -> context.resources.getString(R.string.grace_period)
                        AppStoreAccountState.FailedToRenew, AppStoreAccountState.Expired -> context.resources.getString(R.string.expiration_day)
                    }
                }
            }.bold()

            val accountState = (maybeSubscriptionInfo.paymentPlatform as? PaymentPlatform.GooglePlay)?.accountState

            val gracePeriodViewsVisibility = if (accountState == GooglePlayAccountState.GracePeriod) {
                View.VISIBLE
            } else {
                View.GONE
            }

            paymentIssue.visibility = gracePeriodViewsVisibility
            solvePaymentIssue.visibility = gracePeriodViewsVisibility

            context.resources.apply {
                subscriptionInfo.text = spannable {
                    getString(R.string.payment_platform).bold() + " " +
                        maybeSubscriptionInfo.paymentPlatform.toReadableString(context.resources) + "\n" +
                        renewalOrExpirationText + " " + SimpleDateFormat(
                        "yyyy-MM-dd",
                        Locale.getDefault()
                    ).format(Date(maybeSubscriptionInfo.periodEnd))
                }
            }
        }
    }
}
