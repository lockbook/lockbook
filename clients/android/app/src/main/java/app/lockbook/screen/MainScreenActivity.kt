package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.View
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import androidx.fragment.app.*
import androidx.slidingpanelayout.widget.SlidingPaneLayout
import app.lockbook.R
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.model.*
import app.lockbook.ui.CreateFileDialogFragment
import app.lockbook.ui.FileInfoDialogFragment
import app.lockbook.ui.MoveFileDialogFragment
import app.lockbook.ui.RenameFileDialogFragment
import app.lockbook.util.Animate
import app.lockbook.util.FilesFragment
import app.lockbook.util.exhaustive
import timber.log.Timber
import java.io.File
import java.lang.ref.WeakReference
import java.util.ArrayList

class MainScreenActivity: AppCompatActivity() {
    private var _binding: ActivityMainScreenBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            val filesFragment = supportFragmentManager.findFragmentById(R.id.files_fragment)

            if(filesFragment is FilesFragment) {
                when(f) {
                    is MoveFileDialogFragment,
                    is RenameFileDialogFragment -> filesFragment.refreshFiles()
                    is CreateFileDialogFragment -> {
                        filesFragment.onNewFileCreated(f.newFile)
                    }
                    is FileInfoDialogFragment -> filesFragment.unselectFiles()
                }
            }
        }
    }

    val onShare =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            val filesFragment = (supportFragmentManager.findFragmentById(R.id.files_fragment) as FilesFragment)

            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(false))
            model.shareModel.isLoadingOverlayVisible = false

            filesFragment.unselectFiles()
        }

    val model: StateViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main_screen)

        _binding = ActivityMainScreenBinding.inflate(layoutInflater)

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        onBackPressedDispatcher.addCallback(this,
            TwoPaneOnBackPressedCallback(binding.slidingPaneLayout))

        if(model.shareModel.isLoadingOverlayVisible) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(model.shareModel.isLoadingOverlayVisible))
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
                when(screen) {
                    is TransientScreen.Create -> {
                        CreateFileDialogFragment().show(supportFragmentManager, CreateFileDialogFragment.CREATE_FILE_DIALOG_TAG)
                    }
                    is TransientScreen.Info -> {
                        FileInfoDialogFragment().show(supportFragmentManager, FileInfoDialogFragment.FILE_INFO_DIALOG_TAG)
                    }
                    is TransientScreen.Move -> {
                        MoveFileDialogFragment().show(supportFragmentManager, MoveFileDialogFragment.MOVE_FILE_DIALOG_TAG)
                    }
                    is TransientScreen.Rename -> {
                        RenameFileDialogFragment().show(supportFragmentManager, RenameFileDialogFragment.RENAME_FILE_DIALOG_TAG)
                    }
                    is TransientScreen.Share -> {
                        finalizeShare(screen.files)
                    }
                }
            }
        )

        model.updateMainScreenUI.observe(
            this,
            { update ->
                updateMainScreenUI(update)
            }
        )
    }

    private fun updateMainScreenUI(update: UpdateMainScreenUI) {
        when (update) {
            is UpdateMainScreenUI.NotifyError -> alertModel.notifyError(update.error)
            is UpdateMainScreenUI.ShareDocuments -> finalizeShare(update.files)
            is UpdateMainScreenUI.ShowHideProgressOverlay -> {
                val progressOverlay =
                    binding.progressOverlay.root

                if (update.show) {
                    Animate.animateVisibility(progressOverlay, View.VISIBLE, 100, 500)
                } else {
                    Animate.animateVisibility(progressOverlay, View.GONE, 0, 500)
                }
            }
        }.exhaustive
    }

    private fun finalizeShare(files: List<File>) {
        val uris = ArrayList<Uri>()

        for (file in files) {
            uris.add(
                FileProvider.getUriForFile(
                    this,
                    "app.lockbook.fileprovider",
                    file
                )
            )
        }

        val intent = Intent(Intent.ACTION_SEND_MULTIPLE)
        intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)

        val clipData = ClipData.newRawUri(null, Uri.EMPTY)
        uris.forEach { uri ->
            clipData.addItem(ClipData.Item(uri))
        }

        intent.clipData = clipData
        intent.type = "*/*"
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        intent.putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)

        onShare.launch(
            Intent.createChooser(
                intent,
                "Send multiple files."
            )
        )
    }

    override fun onDestroy() {
        super.onDestroy()
        supportFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
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
        val exit = when(val fragment = supportFragmentManager.findFragmentById(R.id.files_fragment)) {
            is FilesListFragment -> {
                fragment.onBackPressed()
            }
            else -> {
                true
            }
        }

        if(exit) {
            super.onBackPressed()
        }
    }

    fun isThisAnImport(): Boolean {
        return intent.extras?.getBoolean(IS_THIS_AN_IMPORT, false) ?: false
    }
}


class TwoPaneOnBackPressedCallback(
    private val slidingPaneLayout: SlidingPaneLayout
) : OnBackPressedCallback(
    slidingPaneLayout.isSlideable && slidingPaneLayout.isOpen
), SlidingPaneLayout.PanelSlideListener {

    init {
        slidingPaneLayout.addPanelSlideListener(this)
    }

    override fun handleOnBackPressed() {
        slidingPaneLayout.closePane()
    }

    override fun onPanelSlide(panel: View, slideOffset: Float) { }

    override fun onPanelOpened(panel: View) {
        isEnabled = true
    }

    override fun onPanelClosed(panel: View) {
        isEnabled = false
    }
}


const val IS_THIS_AN_IMPORT = "is_this_an_import"