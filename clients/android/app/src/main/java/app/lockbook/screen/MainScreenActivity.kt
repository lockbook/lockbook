package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import androidx.core.os.bundleOf
import androidx.fragment.app.*
import app.lockbook.R
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.databinding.ActivityNewAccountBinding
import app.lockbook.databinding.SplashScreenBinding
import app.lockbook.model.*
import java.io.File
import java.lang.ref.WeakReference

class MainScreenActivity: AppCompatActivity() {
    private var _binding: ActivityMainScreenBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    private val model: StateViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main_screen)

        _binding = ActivityMainScreenBinding.inflate(layoutInflater)

        supportFragmentManager.commit {
            setReorderingAllowed(true)
            add<FilesFragment>(R.id.files_fragment)
        }

        model.launchDetailsScreen.observe(
            this,
            { screen ->
                launchDetailsScreen(screen)
            }
        )

        model.launchTransientScreen.observe(
            this,
            { screen ->
                screen.show(this)
            }
        )

        model.updateMainScreenUI.observe(
            this,
            { update ->
                when(update) {
                    is UpdateMainScreenUI.NotifyError -> alertModel.notifyError(update.error)
                }
            }
        )
    }

    private fun launchDetailsScreen(screen: DetailsScreen) {
        supportFragmentManager.commit {
            setReorderingAllowed(true)
            when(screen) {
                DetailsScreen.Blank -> replace<Fragment>(R.id.detail_container)
                is DetailsScreen.TextEditor -> replace<TextEditorFragment>(R.id.detail_container)
                is DetailsScreen.Drawing -> replace<DrawingFragment>(R.id.detail_container)
            }

            if(binding.slidingPaneLayout.isOpen) {
                setTransition(FragmentTransaction.TRANSIT_FRAGMENT_FADE)
            }
        }

        binding.slidingPaneLayout.open()
    }

    override fun onBackPressed() {
        val fragment = (supportFragmentManager.findFragmentById(R.id.files_fragment) as FilesFragment)

        if(fragment.onBackPressed()) {
            super.onBackPressed()
        }
    }
}