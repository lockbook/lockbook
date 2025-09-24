package app.lockbook.screen

import android.content.Intent
import android.os.Bundle
import android.view.View
import androidx.activity.OnBackPressedCallback
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.model.NotifySyncDone
import app.lockbook.model.SyncModel
import app.lockbook.util.exhaustive
import app.lockbook.util.getApp

class ImportAccountActivity : AppCompatActivity() {
    private var _binding: ActivityImportAccountBinding? = null
    // This property is only valid between onCreateView and
    // onDestroyView.
    private val binding get() = _binding!!

    private val syncModel = SyncModel()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityImportAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        if (!getApp().isInImportSync) {
            getApp().isInImportSync = true
        }

        binding.importExitApp.setOnClickListener {
            onBackPressed()
        }

//        if (model.isErrorVisible) {
//            binding.importAccountProgressBar.visibility = View.GONE
//            binding.importExitApp.visibility = View.VISIBLE
//        }
        syncModel.notifySyncStepInfo.observe(
            this
        ) { stepInfo ->
            println("received sync step info")
            binding.importAccountProgressBar.max = stepInfo.total
            binding.importAccountProgressBar.progress = stepInfo.progress

            binding.importInfo.text = stepInfo.msg
        }

        syncModel.notifySyncDone.observe(
            this
        ) { updateImportUI ->
            when (updateImportUI) {
                NotifySyncDone.FinishedSync -> {
                    getApp().isInImportSync = false

                    startActivity(Intent(applicationContext, MainScreenActivity::class.java))

                    finishAffinity()
                }
                is NotifySyncDone.NotifyError -> {
                    binding.importAccountProgressBar.visibility = View.GONE
                    binding.importExitApp.visibility = View.VISIBLE

                    binding.importInfo.text = updateImportUI.error.msg
                }
            }.exhaustive
        }

        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    val intent = Intent(Intent.ACTION_MAIN)
                    intent.addCategory(Intent.CATEGORY_HOME)
                    intent.flags = Intent.FLAG_ACTIVITY_NEW_TASK
                    startActivity(intent)
                }
            }
        )
    }
}
