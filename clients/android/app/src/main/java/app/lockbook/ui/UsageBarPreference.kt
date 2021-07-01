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
import app.lockbook.screen.SettingsActivity
import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Ok
import timber.log.Timber

class UsageBarPreference(context: Context, attributeSet: AttributeSet?) : Preference(context, attributeSet) {

    val config = Config(context.filesDir.absolutePath)

    private val alertModel by lazy {
        AlertModel(context as SettingsActivity)
    }

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

        when (val getUsageResult = CoreModel.getLocalAndServerUsage(config, true)) {
            is Ok -> {
                val localAndServerUsages = getUsageResult.value
                val resources = holder.itemView.resources
                Timber.e("${localAndServerUsages.uncompressedUsage} ${localAndServerUsages.serverUsage} ${localAndServerUsages.dataCap}")

                val dataCapDot = localAndServerUsages.dataCap.indexOf(" ")
                val serverUsageDot = localAndServerUsages.serverUsage.indexOf(" ")
                val uncompressedUsageDot = localAndServerUsages.uncompressedUsage.indexOf(" ")

                if (dataCapDot == -1 || serverUsageDot == -1 || uncompressedUsageDot == -1) {
                    alertModel.notifyBasicError()
                }

                val dataCapNum = localAndServerUsages.dataCap.substring(0, dataCapDot).toLongOrNull()
                val serverUsageNum = localAndServerUsages.dataCap.substring(0, serverUsageDot).toLongOrNull()
                val uncompressedUsageNum = localAndServerUsages.uncompressedUsage.substring(0, uncompressedUsageDot).toLongOrNull()

                if (dataCapNum == null || serverUsageNum == null || uncompressedUsageNum == null) {
                    alertModel.notifyBasicError()
                } else {
                    val usageBar = holder.itemView.findViewById<ProgressBar>(R.id.usage_bar)

                    usageBar.max = dataCapNum.toInt()
                    usageBar.progress = serverUsageNum.toInt()

                    usageInfo.text = spannable {
                        resources.getString(R.string.settings_usage_current).bold() + " " + CoreModel.makeBytesReadable(serverUsageNum) + "\n" + resources.getString(R.string.settings_usage_data_cap).bold() + " " + CoreModel.makeBytesReadable(dataCapNum) + "\n" + resources.getString(R.string.settings_usage_uncompressed_usage).bold() + " " + CoreModel.makeBytesReadable(uncompressedUsageNum)
                    }
                }
            }
            is Err -> {
                val lbError = getUsageResult.error.toLbError()
                alertModel.notifyError(lbError)
                if(lbError.kind == LbErrorKind.User) {
                    usageInfo.text = lbError.msg
                }
            }
        }
    }
}
