package app.lockbook.ui

import android.content.Context
import android.content.Intent
import android.content.res.ColorStateList
import android.util.AttributeSet
import android.view.View
import android.widget.LinearLayout
import androidx.core.content.ContextCompat
import androidx.preference.PreferenceManager
import app.lockbook.App
import app.lockbook.R
import app.lockbook.databinding.OutOfSpaceBinding
import app.lockbook.model.CoreModel
import app.lockbook.screen.SettingsActivity
import app.lockbook.util.Animate
import app.lockbook.util.getString
import com.github.michaelbull.result.unwrap
import kotlinx.coroutines.*

class OutOfSpaceView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : LinearLayout(context, attrs) {
    private var _binding: OutOfSpaceBinding? = null
    private val binding get() = _binding!!

    private var _usageProgress: Double? = null
    private val usageProgress get() = _usageProgress!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.IO + job)

    private val pref by lazy { PreferenceManager.getDefaultSharedPreferences(context) }

    init {
        _binding =
            OutOfSpaceBinding.bind(inflate(context, R.layout.out_of_space, this@OutOfSpaceView))

        binding.outOfSpaceCancelButton.setOnClickListener {
            Animate.animateVisibility(binding.root, View.GONE, 0, 200)
            val ids = if (usageProgress > 0.95) {
                listOf(R.string.hide_out_of_space_95, R.string.hide_out_of_space_80)
            } else {
                listOf(R.string.hide_out_of_space_80)
            }

            ids.forEach { id ->
                pref.edit().putBoolean(getString(resources, id), true).apply()
            }
        }

        binding.root.setOnClickListener {
            context.startActivity(Intent(context, SettingsActivity::class.java))
        }

        updateBasedOnUsage(context)
    }

    fun updateBasedOnUsage(context: Context) {
        uiScope.launch {
            val usage = CoreModel.getUsage(App.config).unwrap()
            _usageProgress = usage.serverUsage.exact.toDouble() / usage.dataCap.exact

            val hideViewAt95Usage = pref.getBoolean(
                getString(resources, R.string.hide_out_of_space_95),
                false
            )

            val hideViewAt80Usage = pref.getBoolean(
                getString(resources, R.string.hide_out_of_space_80),
                false
            )

            withContext(Dispatchers.Main) {
                if (usageProgress > 0.95 && !hideViewAt95Usage) {
                    binding.outOfSpaceProgressBar.progress = (usageProgress * 100).toInt()
                    binding.outOfSpaceProgressBar.progressTintList =
                        ColorStateList.valueOf(ContextCompat.getColor(context, R.color.red))
                    Animate.animateVisibility(this@OutOfSpaceView, View.VISIBLE, 255, 200)
                } else if (usageProgress > 0.8 && !hideViewAt80Usage) {
                    binding.outOfSpaceProgressBar.progress = (usageProgress * 100).toInt()
                    binding.outOfSpaceProgressBar.progressTintList =
                        ColorStateList.valueOf(ContextCompat.getColor(context, R.color.yellow))
                    Animate.animateVisibility(this@OutOfSpaceView, View.VISIBLE, 255, 200)
                } else if (usageProgress <= 0.7) {
                    if (hideViewAt80Usage) {
                        pref.edit().putBoolean(getString(resources, R.string.hide_out_of_space_80), false).apply()
                    }

                    if (hideViewAt95Usage) {
                        pref.edit().putBoolean(getString(resources, R.string.hide_out_of_space_95), false).apply()
                    }
                }
            }
        }
    }
}
