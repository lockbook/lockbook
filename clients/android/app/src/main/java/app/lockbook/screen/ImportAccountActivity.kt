package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.model.ImportAccountViewModel
import app.lockbook.model.UpdateImportUI
import app.lockbook.util.exhaustive

class ImportAccountActivity : AppCompatActivity() {
    private var _binding: ActivityImportAccountBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val model: ImportAccountViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityImportAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        model.syncModel.notifySyncStepInfo.observe(
            this
        ) { stepInfo ->
            binding.importAccountProgressBar.max = stepInfo.total
            binding.importAccountProgressBar.progress = stepInfo.progress

            binding.importInfo.text = stepInfo.action.toMessage()
        }

        model.updateImportUI.observe(
            this
        ) { updateImportUI ->
            when (updateImportUI) {
                UpdateImportUI.FinishedSync -> {
                    startActivity(Intent(applicationContext, MainScreenActivity::class.java))

                    finishAffinity()
                }
                is UpdateImportUI.NotifyError -> {
                    binding.importAccountProgressBar.visibility = View.GONE
                    binding.importInfo.text = updateImportUI.error.msg
                }
            }.exhaustive
        }
    }
}
