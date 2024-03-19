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
import com.github.michaelbull.result.unwrap
import java.io.File
import java.lang.ref.WeakReference

class MainScreenActivity : AppCompatActivity() {
    private var _binding: ActivityMainScreenBinding? = null
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
            maybeGetFilesFragment()?.refreshFiles()
        }

    private val onExport =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(false))
            model.exportImportModel.isLoadingOverlayVisible = false

            getFilesFragment().unselectFiles()
        }

    val model: StateViewModel by viewModels()
    val workspaceModel: WorkspaceViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityMainScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        val wFragment = supportFragmentManager.findFragmentByTag("Workspace")

        if(wFragment == null) {
            println("adding this workspace to fragment")
            supportFragmentManager.commit {
                setReorderingAllowed(true)
                add<WorkspaceFragment>(R.id.detail_container, "Workspace")
            }
        } else {
            println("not adding")
        }

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

        if (model.exportImportModel.isLoadingOverlayVisible) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(model.exportImportModel.isLoadingOverlayVisible))
        }

        binding.slidingPaneLayout.addPanelSlideListener(object :
                SlidingPaneLayout.PanelSlideListener {
                override fun onPanelSlide(panel: View, slideOffset: Float) {}

                override fun onPanelOpened(panel: View) {
//                    if (model.detailScreen is DetailScreen.Loading) {
//                        (supportFragmentManager.findFragmentById(R.id.detail_container) as DetailScreenLoaderFragment).addChecker()
//                    }
                }

                override fun onPanelClosed(panel: View) {}
            })

        slidingPaneLayout.lockMode = SlidingPaneLayout.LOCK_MODE_LOCKED

        model.launchActivityScreen.observe(
            this
        ) { screen ->
            when (screen) {
                is ActivityScreen.Settings -> {
                    val intent = Intent(applicationContext, SettingsActivity::class.java)

                    if (screen.scrollToPreference != null) {
                        intent.putExtra(SettingsFragment.SCROLL_TO_PREFERENCE_KEY, screen.scrollToPreference)
                    }

                    startActivity(intent)
                }
                ActivityScreen.Shares -> {
                    onShare.launch(Intent(baseContext, SharesActivity::class.java))
                }
                null -> {}
            }
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

        workspaceModel.newFolderBtnPressed.observe(this) {
            model.launchTransientScreen(TransientScreen.Create(workspaceModel.selectedFile.value ?: CoreModel.getRoot().unwrap().id, ExtendedFileType.Folder))
        }

        workspaceModel.tabTitleClicked.observe(this) {
            model.launchTransientScreen(TransientScreen.Rename(CoreModel.getFileById(workspaceModel.selectedFile.value!!).unwrap()))
        }
    }

    private fun updateMainScreenUI(update: UpdateMainScreenUI) {
        when (update) {
            is UpdateMainScreenUI.OpenFile -> {
                if(update.id != null) {
                    workspaceModel._openFile.value = Pair(update.id, false)
                    slidingPaneLayout.openPane()
                } else {
                    if(workspaceModel.selectedFile.value != null) {
                        workspaceModel._closeDocument.value = workspaceModel.selectedFile.value
                    }
                    slidingPaneLayout.closePane()
                }
            }
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
            UpdateMainScreenUI.Sync -> maybeGetFilesFragment()?.sync(false)
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

        onExport.launch(Intent.createChooser(intent, "Send multiple files."))
    }

    override fun onDestroy() {
        super.onDestroy()
        supportFragmentManager.unregisterFragmentLifecycleCallbacks(fragmentFinishedCallback)
    }

    private fun onFileDeleted(filesFragment: FilesFragment) {
        if(workspaceModel.selectedFile.value != null) {
            workspaceModel._closeDocument.value = workspaceModel.selectedFile.value
        }
    }

    override fun onBackPressed() {
        if (slidingPaneLayout.isSlideable && slidingPaneLayout.isOpen) { // if you are on a small display where only files or an editor show once at a time, you want to handle behavior a bit differently
            model.updateMainScreenUI(UpdateMainScreenUI.OpenFile(null))
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
