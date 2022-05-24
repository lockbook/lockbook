package app.lockbook.ui

import android.content.Context
import android.content.Intent
import android.util.AttributeSet
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.screen.SettingsActivity
import app.lockbook.screen.SettingsFragment
import app.lockbook.screen.UpgradeAccountActivity
import app.lockbook.util.*
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job

class UsageBarPreference(context: Context, attributeSet: AttributeSet?) : Preference(context, attributeSet) {
    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    lateinit var usageBar: ProgressBar
    lateinit var premiumUsageBar: ProgressBar
    lateinit var premiumInfoForFree: LinearLayout
    lateinit var upgradeAccount: Button
    lateinit var usageInfo: TextView

    private val alertModel get() =
        ((context as SettingsActivity).supportFragmentManager.fragments[0] as SettingsFragment).alertModel

    init {
        layoutResource = R.layout.preference_usage_bar
    }

    companion object {
        const val PAID_TIER_USAGE_BYTES: Long = 50000000000
        const val ROUND_DECIMAL_PLACES: Long = 10000
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder) {
        super.onBindViewHolder(holder)

        usageBar = holder.itemView.findViewById(R.id.usage_bar)
        premiumUsageBar = holder.itemView.findViewById(R.id.premium_usage_bar)
        premiumInfoForFree = holder.itemView.findViewById(R.id.premium_info_for_free)
        upgradeAccount = holder.itemView.findViewById(R.id.upgrade_account)
        usageInfo = holder.itemView.findViewById(R.id.usage_info)
    }

    fun setUpUsagePreference(usage: UsageMetrics, uncompressedUsage: UsageItemMetric) {
        usageBar.max = (usage.dataCap.exact / ROUND_DECIMAL_PLACES).toInt()
        usageBar.progress = (usage.serverUsage.exact / ROUND_DECIMAL_PLACES).toInt()

        if (usage.dataCap.exact != PAID_TIER_USAGE_BYTES) {
            upgradeAccount.setOnClickListener {

                context.startActivity(Intent(context, UpgradeAccountActivity::class.java))
//                        (context as SettingsActivity).overridePendingTransition(R.anim.slide_in, R.anim.slide_out)
            }
            premiumInfoForFree.visibility = View.VISIBLE

            premiumUsageBar.max = (PAID_TIER_USAGE_BYTES / ROUND_DECIMAL_PLACES).toInt()
            premiumUsageBar.progress = (usage.serverUsage.exact / ROUND_DECIMAL_PLACES).toInt()
        }

        usageInfo.text = spannable {
            context.resources.getString(R.string.settings_usage_current)
                .bold() + " " + usage.serverUsage.readable + "\n" + context.resources.getString(
                R.string.settings_usage_data_cap
            )
                .bold() + " " + usage.dataCap.readable + "\n" + context.resources.getString(
                R.string.settings_usage_uncompressed_usage
            ).bold() + " " + uncompressedUsage.readable
        }
    }
}
