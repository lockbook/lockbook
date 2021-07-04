package app.lockbook.ui

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.util.AttributeSet
import android.widget.ProgressBar
import android.widget.TextView
import androidx.preference.Preference
import androidx.preference.PreferenceViewHolder
import app.lockbook.R
import app.lockbook.model.AlertModel
import app.lockbook.model.CoreModel
import app.lockbook.model.OnFinishAlert
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.andThen
import com.github.michaelbull.result.map
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
        val usageInfo = holder.itemView.findViewById<TextView>(R.id.usage_info)

        val getUsageResult = CoreModel.getUsage(config).andThen { usage ->
            val resources = holder.itemView.resources

            val usageBar = holder.itemView.findViewById<ProgressBar>(R.id.usage_bar)

            usageBar.max = usage.dataCap.exact
            usageBar.progress = usage.serverUsage.exact

            CoreModel.getUncompressedUsage(config).map { uncompressedUsage ->
                usageInfo.text = spannable {
                    resources.getString(R.string.settings_usage_current).bold() + " " + usage.serverUsage.readable + "\n" + resources.getString(R.string.settings_usage_data_cap).bold() + " " + usage.dataCap.readable + "\n" + resources.getString(R.string.settings_usage_uncompressed_usage).bold() + " " + uncompressedUsage.readable
                }
            }
        }

        if (getUsageResult is Err) {
            when (val error = getUsageResult.error) {
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
}
