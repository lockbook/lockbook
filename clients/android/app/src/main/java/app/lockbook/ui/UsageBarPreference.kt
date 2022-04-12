package app.lockbook.ui

import android.content.Context
import android.content.Intent
import android.util.AttributeSet
import android.view.View
import android.widget.Button
import android.widget.ProgressBar
import android.widget.TextView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.model.CoreModel
import app.lockbook.screen.SettingsActivity
import app.lockbook.screen.SettingsFragment
import app.lockbook.screen.UpgradeAccountActivity
import app.lockbook.util.*
import com.github.michaelbull.result.getOrElse
import kotlinx.coroutines.*


class UsageBarPreference(context: Context, attributeSet: AttributeSet?) : Preference(context, attributeSet) {
    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    private val alertModel by lazy {
        ((context as SettingsActivity).supportFragmentManager.fragments[0] as SettingsFragment).alertModel
    }

    init {
        layoutResource = R.layout.preference_usage_bar
    }

    companion object {
        const val PAID_TIER_USAGE_BYTES: Long = 50000000000
        const val ROUND_DECIMAL_PLACES: Long = 10000
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder) {
        super.onBindViewHolder(holder)

        setUpUsagePreference(holder)
    }

    private fun setUpUsagePreference(holder: PreferenceViewHolder) {
        uiScope.launch {
            withContext(Dispatchers.IO) {
                val usageInfo = holder.itemView.findViewById<TextView>(R.id.usage_info)

                val usage = CoreModel.getUsage().getOrElse { error ->
                    showError(error.toLbError(context.resources), usageInfo)
                    return@withContext
                }

                val resources = holder.itemView.resources
                val usageBar = holder.itemView.findViewById<ProgressBar>(R.id.usage_bar)

                usageBar.max = (usage.dataCap.exact / ROUND_DECIMAL_PLACES).toInt()
                usageBar.progress = (usage.serverUsage.exact / ROUND_DECIMAL_PLACES).toInt()

                val premiumUsageBar = holder.itemView.findViewById<ProgressBar>(R.id.premium_usage_bar)
                val premiumUsageInfo = holder.itemView.findViewById<TextView>(R.id.premium_usage_info)

                if(usage.dataCap.exact != PAID_TIER_USAGE_BYTES) {
                    premiumUsageBar.max = (PAID_TIER_USAGE_BYTES / ROUND_DECIMAL_PLACES).toInt()
                    premiumUsageBar.progress = (usage.serverUsage.exact / ROUND_DECIMAL_PLACES).toInt()

                    holder.itemView.findViewById<Button>(R.id.upgrade_account).setOnClickListener {
                        context.startActivity(Intent(context, UpgradeAccountActivity::class.java))
//                        (context as SettingsActivity).overridePendingTransition(R.anim.slide_in, R.anim.slide_out)
                    }
                } else {
                    premiumUsageBar.visibility = View.GONE
                    premiumUsageInfo.visibility = View.GONE
                }

                val uncompressedUsage = CoreModel.getUncompressedUsage().getOrElse { error ->
                    showError(error.toLbError(context.resources), usageInfo)
                    return@withContext
                }

                withContext(Dispatchers.Main) {
                    usageInfo.text = spannable {
                        resources.getString(R.string.settings_usage_current)
                            .bold() + " " + usage.serverUsage.readable + "\n" + resources.getString(
                            R.string.settings_usage_data_cap
                        )
                            .bold() + " " + usage.dataCap.readable + "\n" + resources.getString(
                            R.string.settings_usage_uncompressed_usage
                        ).bold() + " " + uncompressedUsage.readable
                    }
                }
            }
        }
    }

    private suspend fun showError(
        lbError: LbError,
        usageInfo: TextView
    ) {
        alertModel.notifyError(lbError)
        withContext(Dispatchers.Main) {
            usageInfo.text = if (lbError.kind == LbErrorKind.User) {
                lbError.msg
            } else {
                getString(context.resources, R.string.basic_error)
            }
        }
    }
}
