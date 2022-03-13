package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.viewModels
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.model.*
import app.lockbook.util.exhaustive
import kotlinx.coroutines.*
import timber.log.Timber

class ImportAccountActivity : AppCompatActivity() {
    private var _binding: ActivityImportAccountBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val model: ImportAccountViewModel by viewModels(
        factoryProducer = {
            object : ViewModelProvider.Factory {
                override fun <T : ViewModel?> create(modelClass: Class<T>): T {
                    if (modelClass.isAssignableFrom(ImportAccountViewModel::class.java))
                        return ImportAccountViewModel(application) as T
                    throw IllegalArgumentException("Unknown ViewModel class")
                }
            }
        }
    )

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityImportAccountBinding.inflate(layoutInflater)

        Timber.e("GOT TO NEXT ACTIVITY")

        model.syncModel.notifySyncProgress.observe(
            this
        ) { progress ->
            binding.importInfo.text = progress.action.toString()
        }

        model.updateImportUI.observe(
            this
        ) { updateImportUI ->
            when(updateImportUI) {
                UpdateImportUI.FinishedSync -> {
                    val intent = Intent(applicationContext, MainScreenActivity::class.java)

                    startActivity(intent)
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
