package app.lockbook.screen

import android.Manifest
import android.app.AlertDialog
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.View
import android.widget.Toast
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.annotation.RequiresApi
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import app.lockbook.databinding.ActivityImportAccountBinding
import app.lockbook.model.ImportAccountViewModel
import app.lockbook.model.UpdateImportUI
import app.lockbook.util.exhaustive
import app.lockbook.util.getApp

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

        if (!getApp().isInImportSync) {
            getApp().isInImportSync = true
        }

        binding.importExitApp.setOnClickListener {
            onBackPressed()
        }

        if (model.isErrorVisible) {
            binding.importAccountProgressBar.visibility = View.GONE
            binding.importExitApp.visibility = View.VISIBLE
        }

        model.syncModel.notifySyncStepInfo.observe(
            this
        ) { stepInfo ->
            binding.importAccountProgressBar.max = stepInfo.total
            binding.importAccountProgressBar.progress = stepInfo.progress

            binding.importInfo.text = stepInfo.msg
        }

        model.updateImportUI.observe(
            this
        ) { updateImportUI ->
            when (updateImportUI) {
                UpdateImportUI.FinishedSync -> {
                    getApp().isInImportSync = false

                    startActivity(Intent(applicationContext, MainScreenActivity::class.java))

                    finishAffinity()
                }
                is UpdateImportUI.NotifyError -> {
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
