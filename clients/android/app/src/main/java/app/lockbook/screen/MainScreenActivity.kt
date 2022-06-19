package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.View
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import androidx.fragment.app.*
import androidx.slidingpanelayout.widget.SlidingPaneLayout
import app.lockbook.R
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.Animate
import app.lockbook.util.FilesFragment
import app.lockbook.util.exhaustive
import java.io.File
import java.lang.ref.WeakReference

class MainScreenActivity : AppCompatActivity() {
    private var _binding: ActivityMainScreenBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!
    private val slidingPaneLayout get() = binding.slidingPaneLayout

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    private val fragmentFinishedCallback = object : FragmentManager.FragmentLifecycleCallbacks() {
        override fun onFragmentDestroyed(fm: FragmentManager, f: Fragment) {
            val filesFragment =
                supportFragmentManager.findFragmentById(R.id.files_fragment) as? FilesFragment
                    ?: return

            when (f) {
                is MoveFileDialogFragment,
                is RenameFileDialogFragment -> filesFragment.refreshFiles()
                is CreateFileDialogFragment -> filesFragment.onNewFileCreated(f.newFile)
                is FileInfoDialogFragment -> filesFragment.unselectFiles()
                is DeleteFilesDialogFragment -> filesFragment.refreshFiles()
            }
        }
    }

    private val onShare =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            val filesFragment =
                (supportFragmentManager.findFragmentById(R.id.files_fragment) as FilesFragment)

            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(false))
            model.shareModel.isLoadingOverlayVisible = false

            filesFragment.unselectFiles()
        }

    val model: StateViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityMainScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)
        supportActionBar?.hide()

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        if (model.shareModel.isLoadingOverlayVisible) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(model.shareModel.isLoadingOverlayVisible))
        }

        binding.slidingPaneLayout.addPanelSlideListener(object :
                SlidingPaneLayout.PanelSlideListener {
                override fun onPanelSlide(panel: View, slideOffset: Float) {}

                override fun onPanelOpened(panel: View) {
                    if (model.detailsScreen is DetailsScreen.Loading) {
                        (supportFragmentManager.findFragmentById(R.id.detail_container) as DetailsScreenLoaderFragment).addChecker()
                    }
                }

                override fun onPanelClosed(panel: View) {}
            })

        slidingPaneLayout.lockMode = SlidingPaneLayout.LOCK_MODE_LOCKED

        model.launchDetailsScreen.observe(
            this
        ) { screen ->
            launchDetailsScreen(screen)
        }

        model.launchTransientScreen.observe(
            this
        ) { screen ->
            when (screen) {
                is TransientScreen.Create -> {
                    CreateFileDialogFragment().show(
                        supportFragmentManager,
                        CreateFileDialogFragment.CREATE_FILE_DIALOG_TAG
                    )
                }
                is TransientScreen.Info -> {
                    FileInfoDialogFragment().show(
                        supportFragmentManager,
                        FileInfoDialogFragment.FILE_INFO_DIALOG_TAG
                    )
                }
                is TransientScreen.Move -> {
                    MoveFileDialogFragment().show(
                        supportFragmentManager,
                        MoveFileDialogFragment.MOVE_FILE_DIALOG_TAG
                    )
                }
                is TransientScreen.Rename -> {
                    RenameFileDialogFragment().show(
                        supportFragmentManager,
                        RenameFileDialogFragment.RENAME_FILE_DIALOG_TAG
                    )
                }
                is TransientScreen.Share -> {
                    finalizeShare(screen.files)
                }
                is TransientScreen.Delete -> {
                    DeleteFilesDialogFragment().show(
                        supportFragmentManager,
                        DeleteFilesDialogFragment.DELETE_FILEs_DIALOG_FRAGMENT
                    )
                }
            }.exhaustive
        }

        model.updateMainScreenUI.observe(
            this
        ) { update ->
            updateMainScreenUI(update)
        }
    }

    private fun updateMainScreenUI(update: UpdateMainScreenUI) {
        when (update) {
            is UpdateMainScreenUI.NotifyError -> alertModel.notifyError(update.error)
            is UpdateMainScreenUI.ShareDocuments -> finalizeShare(update.files)
            is UpdateMainScreenUI.ShowHideProgressOverlay -> {
                if (update.show) {
                    Animate.animateVisibility(binding.progressOverlay, View.VISIBLE, 100, 500)
                } else {
                    Animate.animateVisibility(binding.progressOverlay, View.GONE, 0, 500)
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

        onShare.launch(Intent.createChooser(intent, "Send multiple files."))
    }

    override fun onDestroy() {
        super.onDestroy()
        supportFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
    }

    private fun launchDetailsScreen(screen: DetailsScreen?) {
        supportFragmentManager.commit {
            setReorderingAllowed(true)
            doOnDetailsExit(screen)

            when (screen) {
                is DetailsScreen.Loading -> replace<DetailsScreenLoaderFragment>(R.id.detail_container)
                is DetailsScreen.TextEditor -> replace<TextEditorFragment>(R.id.detail_container)
                is DetailsScreen.Drawing -> replace<DrawingFragment>(R.id.detail_container)
                is DetailsScreen.ImageViewer -> replace<ImageViewerFragment>(R.id.detail_container)
                is DetailsScreen.PdfViewer -> replace<PdfViewerFragment>(R.id.detail_container)
                null -> {
                    (supportFragmentManager.findFragmentById(R.id.files_fragment) as FilesFragment).syncBasedOnPreferences()
                    supportFragmentManager.findFragmentById(R.id.detail_container)?.let {
                        remove(it)
                    }
                }
            }.exhaustive

            if (slidingPaneLayout.isOpen) {
                setTransition(FragmentTransaction.TRANSIT_FRAGMENT_FADE)
            }
        }

        if (screen == null) {
            slidingPaneLayout.closePane()
        } else {
            slidingPaneLayout.openPane()
            binding.detailContainer.requestFocus()
        }
    }

    private fun doOnDetailsExit(newScreen: DetailsScreen?) {
        (supportFragmentManager.findFragmentById(R.id.detail_container) as? DrawingFragment)?.let { fragment ->
            fragment.binding.drawingView.stopThread()
            fragment.saveOnExit()
        }
        (supportFragmentManager.findFragmentById(R.id.detail_container) as? TextEditorFragment)?.saveOnExit()
        (supportFragmentManager.findFragmentById(R.id.detail_container) as? PdfViewerFragment)?.deleteLocalPdfInstance()
        (supportFragmentManager.findFragmentById(R.id.detail_container) as? DetailsScreenLoaderFragment)?.let { fragment ->
            if (newScreen !is DetailsScreen.PdfViewer) {
                fragment.deleteDownloadedFileIfExists()
            }
        }
    }

    override fun onBackPressed() {
        if (slidingPaneLayout.isSlideable && slidingPaneLayout.isOpen) { // if you are on a small display where only files or an editor show once at a time, you want to handle behavior a bit differently
            launchDetailsScreen(null)
        } else if ((supportFragmentManager.findFragmentById(R.id.files_fragment) as FilesFragment).onBackPressed()) {
            super.onBackPressed()
        }
    }

    fun isThisANewAccount(): Boolean {
        return intent.extras?.getBoolean(IS_THIS_A_NEW_ACCOUNT, false) ?: false
    }

    fun syncImportAccount() {
        startActivity(Intent(this, ImportAccountActivity::class.java))
        finishAffinity()
    }
}

const val IS_THIS_A_NEW_ACCOUNT = "is_this_a_new_account"
