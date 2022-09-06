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
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
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
            val filesFragment = maybeGetFilesFragment() ?: return

            when (f) {
                is MoveFileDialogFragment,
                is RenameFileDialogFragment -> filesFragment.refreshFiles()
                is CreateFileDialogFragment -> filesFragment.onNewFileCreated(f.newFile)
                is FileInfoDialogFragment -> filesFragment.unselectFiles()
                is DeleteFilesDialogFragment -> onFileDeleted(filesFragment)
            }
        }
    }

    private val onShare =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(false))
            model.shareModel.isLoadingOverlayVisible = false

            getFilesFragment().unselectFiles()
        }

    val model: StateViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityMainScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)

        toggleTransparentLockbookLogo(model.detailsScreen)

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        (application as App).billingClientLifecycle.apply {
            this@MainScreenActivity.lifecycle.addObserver(this)
            billingEvent.observe(this@MainScreenActivity) { billingEvent ->
                when (billingEvent) {
                    is BillingEvent.SuccessfulPurchase -> {
                        model.confirmSubscription(billingEvent.purchaseToken, billingEvent.accountId)
                    }
                    is BillingEvent.NotifyError,
                    BillingEvent.NotifyUnrecoverableError -> {}
                }.exhaustive
            }
        }

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
                is TransientScreen.ShareExport -> {
                    finalizeShare(screen.files)
                }
                is TransientScreen.Delete -> {
                    DeleteFilesDialogFragment().show(
                        supportFragmentManager,
                        DeleteFilesDialogFragment.DELETE_FILES_DIALOG_FRAGMENT
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
            UpdateMainScreenUI.ShowSubscriptionConfirmed -> {
                alertModel.notifySuccessfulPurchaseConfirm()
            }
            UpdateMainScreenUI.ShowSearch -> navHost().navController.navigate(R.id.action_files_to_search)
            UpdateMainScreenUI.ShowFiles -> navHost().navController.popBackStack()
        }
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

    private fun onFileDeleted(filesFragment: FilesFragment) {
        val openedFile = model.detailsScreen?.getUsedFile()?.id
        if (openedFile != null) {
            val isDeletedFileOpen = (model.transientScreen as TransientScreen.Delete).files.any { file -> file.id == openedFile }

            if (isDeletedFileOpen) {
                launchDetailsScreen(null)
            }
        }

        filesFragment.refreshFiles()
    }

    private fun launchDetailsScreen(screen: DetailsScreen?) {
        supportFragmentManager.commit {
            setReorderingAllowed(true)
            doOnDetailsExit(screen)
            toggleTransparentLockbookLogo(screen)

            when (screen) {
                is DetailsScreen.Loading -> replace<DetailsScreenLoaderFragment>(R.id.detail_container)
                is DetailsScreen.TextEditor -> replace<TextEditorFragment>(R.id.detail_container)
                is DetailsScreen.Drawing -> replace<DrawingFragment>(R.id.detail_container)
                is DetailsScreen.ImageViewer -> replace<ImageViewerFragment>(R.id.detail_container)
                is DetailsScreen.PdfViewer -> replace<PdfViewerFragment>(R.id.detail_container)
                is DetailsScreen.Share -> add<ShareFileFragment>(R.id.detail_container)
                null -> {
                    maybeGetFilesFragment()?.syncBasedOnPreferences()
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

    private fun toggleTransparentLockbookLogo(screen: DetailsScreen?) {
        if (screen != null && binding.lockbookBackdrop.visibility == View.VISIBLE) {
            binding.lockbookBackdrop.visibility = View.GONE
        } else if (screen == null && binding.lockbookBackdrop.visibility == View.GONE) {
            binding.lockbookBackdrop.visibility = View.VISIBLE
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
        } else if (maybeGetSearchFilesFragment() != null) {
            updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
        } else if (maybeGetFilesFragment()?.onBackPressed() == true) {
            super.onBackPressed()
        }
    }

    fun syncImportAccount() {
        startActivity(Intent(this, ImportAccountActivity::class.java))
        finishAffinity()
    }
}
