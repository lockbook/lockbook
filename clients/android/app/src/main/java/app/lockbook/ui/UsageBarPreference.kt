package app.lockbook.ui

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.provider.Settings.Global.getString
import android.util.AttributeSet
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import kotlinx.android.synthetic.main.preference_usage_bar.view.*
import timber.log.Timber

class UsageBarPreference(context: Context, attributeSet: AttributeSet?) : Preference(context, attributeSet) {

    val config = Config(context.filesDir.absolutePath)

    init {
        layoutResource = R.layout.preference_usage_bar
    }

    override fun onBindViewHolder(holder: PreferenceViewHolder?) {
        super.onBindViewHolder(holder)

        setUpUsagePreference(holder!!)
    }

    @SuppressLint("SetTextI18n") // temporary until I add full language support for all errors
    private fun setUpUsagePreference(holder: PreferenceViewHolder) {
        val usageInfo = holder.itemView.usage_info

        when (val getUsageResult = CoreModel.getLocalAndServerUsage(config, false)) {
            is Ok -> {
                val localAndServerUsages = getUsageResult.value
                val resources = holder.itemView.resources

                usageInfo.text = spannable {
                    resources.getString(R.string.settings_usage_current).bold() + " " + localAndServerUsages.serverUsage + "\n" + resources.getString(R.string.settings_usage_data_cap).bold() + " " + localAndServerUsages.dataCap + "\n" + resources.getString(R.string.settings_usage_uncompressed_usage).bold() + " " + localAndServerUsages.uncompressedUsage
                }

                val dataCapDot = localAndServerUsages.dataCap.indexOf(".")
                val serverUsageDot = localAndServerUsages.serverUsage.indexOf(".")

                if (dataCapDot == -1 || serverUsageDot == -1) {
                    AlertModel.errorHasOccurred((context as Activity).findViewById(android.R.id.content), "Error! Could not set up usage bar.", OnFinishAlert.DoNothingOnFinishAlert)
                }

                val dataCapNum = localAndServerUsages.dataCap.substring(0, dataCapDot).toIntOrNull()
                val serverUsageNum = localAndServerUsages.dataCap.substring(0, serverUsageDot).toIntOrNull()

                if (dataCapNum == null || serverUsageNum == null) {
                    AlertModel.errorHasOccurred((context as Activity).findViewById(android.R.id.content), "Error! Could not set up usage bar.", OnFinishAlert.DoNothingOnFinishAlert)
                } else {
                    holder.itemView.usage_bar.max = dataCapNum
                    holder.itemView.usage_bar.progress = serverUsageNum
                }
            }
            is Err -> when (val error = getUsageResult.error) {
                GetUsageError.NoAccount -> {
                    AlertModel.errorHasOccurred((context as Activity).findViewById(android.R.id.content), "Error! No account.", OnFinishAlert.DoNothingOnFinishAlert)
                    usageInfo.text = "Error! No account."
                }
                GetUsageError.CouldNotReachServer -> {
                    AlertModel.errorHasOccurred((context as Activity).findViewById(android.R.id.content), "You are offline.", OnFinishAlert.DoNothingOnFinishAlert)
                    usageInfo.text =
                        holder.itemView.resources.getString(R.string.list_files_offline_snackbar)
                }
                GetUsageError.ClientUpdateRequired -> {
                    AlertModel.errorHasOccurred((context as Activity).findViewById(android.R.id.content), "Update required.", OnFinishAlert.DoNothingOnFinishAlert)
                    usageInfo.text =
                        "Update required."
                }
                is GetUsageError.Unexpected -> {
                    AlertModel.unexpectedCoreErrorHasOccurred((context as Activity).findViewById(android.R.id.content), error.error, OnFinishAlert.DoNothingOnFinishAlert)
                    Timber.e("Unable to get usage: ${error.error}")
                }
            }.exhaustive
        }
    }

    override fun setLayoutResource(layoutResId: Int) {
        super.setLayoutResource(layoutResId)
    }
}
