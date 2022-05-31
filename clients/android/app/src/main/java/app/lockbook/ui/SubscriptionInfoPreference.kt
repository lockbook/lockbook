package app.lockbook.ui

import android.content.Context
import android.util.AttributeSet
import android.widget.TextView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.getSettingsFragment
import app.lockbook.util.*
import java.text.SimpleDateFormat
import java.util.*

class SubscriptionInfoPreference(context: Context, attributeSet: AttributeSet?) : Preference(context, attributeSet) {

    private lateinit var subscriptionInfo: TextView

    init {
        layoutResource = R.layout.preference_subscription_info
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder) {
        super.onBindViewHolder(holder)

        subscriptionInfo = holder.itemView.findViewById(R.id.subscription_info)
        getSettingsFragment().model.determineSettingsInfo.observe(getSettingsFragment()) { settingsInfo ->
            setUpSubscriptionInfoPreference(settingsInfo.subscriptionInfo)
        }
    }


    private fun setUpSubscriptionInfoPreference(maybeSubscriptionInfo: SubscriptionInfo?) {
        if(maybeSubscriptionInfo != null) {
            val renewalOrExpirationText = if((maybeSubscriptionInfo.paymentPlatform as? PaymentPlatform.GooglePlay)?.accountState == GooglePlayAccountState.Canceled) {
                context.resources.getString(R.string.expiration_day)
            } else {
                context.resources.getString(R.string.next_renewal_day)
            }.bold()

            context.resources.apply {
                subscriptionInfo.text = spannable {
                    getString(R.string.payment_platform).bold() + " " +
                            maybeSubscriptionInfo.paymentPlatform.javaClass.simpleName + "\n" +
                            renewalOrExpirationText + " " + SimpleDateFormat("yyyy-MM-dd", Locale.getDefault()).format(Date(maybeSubscriptionInfo.periodEnd))
                }
            }
        }
    }
}
