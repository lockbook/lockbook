@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.ui

import android.content.Context
import android.content.Intent
import android.util.AttributeSet
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.screen.UpgradeAccountActivity
import app.lockbook.util.*
import com.google.android.material.color.MaterialColors
import com.google.android.material.progressindicator.LinearProgressIndicator
import net.lockbook.Usage

class UsageBarPreference(
    context: Context,
    attributeSet: AttributeSet?,
) : Preference(context, attributeSet) {
    lateinit var usageBar: LinearProgressIndicator
    lateinit var premiumUsageBar: LinearProgressIndicator
    lateinit var premiumInfoForFree: LinearLayout
    lateinit var upgradeAccount: Button
    lateinit var usageInfo: TextView
    lateinit var premiumUsageInfo: TextView

    init {
        layoutResource = R.layout.preference_usage_bar
    }

    companion object {
        const val PAID_TIER_USAGE_BYTES: Long = 30000000000
        const val ROUND_DECIMAL_PLACES: Long = 10000
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder) {
        super.onBindViewHolder(holder)

        usageBar = holder.itemView.findViewById(R.id.usage_bar)
        premiumUsageBar = holder.itemView.findViewById(R.id.premium_usage_bar)
        premiumInfoForFree = holder.itemView.findViewById(R.id.premium_info_for_free)
        upgradeAccount = holder.itemView.findViewById(R.id.upgrade_account)
        usageInfo = holder.itemView.findViewById(R.id.usage_info)
        premiumUsageInfo = holder.itemView.findViewById(R.id.premium_usage_info)

        getSettingsFragment().model.determineSettingsInfo.observe(getSettingsFragment()) { settingsInfo ->
            setUpUsagePreference(settingsInfo.usage)
        }
    }

    private fun setUpUsagePreference(usage: Usage) {
        val dataCapExact = usage.dataCap?.exact ?: 1
        val serverUsageExact = usage.serverUsage?.exact ?: 1
        val serverUsage = usage.serverUsage?.readable ?: ""
        val dataCap = usage.dataCap?.readable ?: ""

        val roundedDataCap = dataCapExact / ROUND_DECIMAL_PLACES
        val roundedProgress = serverUsageExact / ROUND_DECIMAL_PLACES

        usageBar.max = roundedDataCap.toInt()
        usageBar.progress = roundedProgress.toInt() * 100

        val usageRatio = roundedProgress.toFloat() / roundedDataCap
        val usageBarColor = getUsageBarColor(usageRatio)
        usageBar.setIndicatorColor(usageBarColor)

        // necessary to reset it for rendering successful billings
        premiumInfoForFree.visibility =
            if (dataCapExact != PAID_TIER_USAGE_BYTES) {
                View.VISIBLE
            } else {
                View.GONE
            }

        if (usage.dataCap?.exact != PAID_TIER_USAGE_BYTES) {
            upgradeAccount.setOnClickListener {
                getSettingsFragment().onUpgrade.launch(Intent(context, UpgradeAccountActivity::class.java))
            }

            premiumUsageBar.max = (PAID_TIER_USAGE_BYTES / ROUND_DECIMAL_PLACES).toInt()
            premiumUsageBar.progress = (serverUsageExact / ROUND_DECIMAL_PLACES).toInt()
            premiumUsageBar.setIndicatorColor(
                getUsageBarColor(serverUsageExact.toFloat() / PAID_TIER_USAGE_BYTES),
            )

            premiumUsageInfo.text = context.resources.getString(R.string.out_of_premium_gb, serverUsage)
        }

        usageInfo.text =
            spannable {
                context.resources
                    .getString(R.string.settings_usage_current)
                    .bold() + " " + serverUsage + "\n" +
                    context.resources
                        .getString(
                            R.string.settings_usage_data_cap,
                        ).bold() + " " + dataCap + "\n"
            }
    }

    private fun getUsageBarColor(usageRatio: Float): Int {
        val colorAttr =
            when {
                usageRatio < 0.8 -> {
                    com.google.android.material.R.attr.colorPrimaryFixed
                }

                else -> {
                    com.google.android.material.R.attr.colorErrorContainer
                }
            }
        val fallbackColor =
            if (usageRatio < 0.8) {
                context.getColor(R.color.md_theme_primary)
            } else {
                context.getColor(R.color.md_theme_error)
            }

        return MaterialColors.getColor(context, colorAttr, fallbackColor)
    }
}
