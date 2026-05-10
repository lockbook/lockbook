package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.view.View
import android.view.WindowManager
import androidx.activity.OnBackPressedCallback
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import androidx.core.view.isVisible
import androidx.fragment.app.*
import androidx.lifecycle.lifecycleScope
import androidx.navigation.ui.setupWithNavController
import androidx.slidingpanelayout.widget.SlidingPaneLayout
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
import app.lockbook.workspace.LbStatus
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import net.lockbook.Lb
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
                is RenameFileDialogFragment -> filesFragment.reloadFiles()
                is CreateFileDialogFragment -> filesFragment.onNewFileCreated(f.newFile)
                is FileInfoDialogFragment -> filesFragment.unselectFiles()
                is DeleteFilesDialogFragment -> {
                    if (workspaceModel.currentTab.value != null) {
                        workspaceModel._closeFile.value = workspaceModel.currentTab.value?.id
                    }

                    filesFragment.reloadFiles()
                }
            }
            filesFragment.unselectFiles()
        }
    }

    private val onExport =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(false))
            model.exportImportModel.isLoadingOverlayVisible = false

            getFilesFragment().unselectFiles()
        }

    val model: StateViewModel by viewModels()
    val workspaceModel: WorkspaceViewModel by viewModels()

    private val fileTreeViewModel: FileTreeViewModel by viewModels()

    val jsonParser = Json {
        ignoreUnknownKeys = true
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        enableEdgeToEdge(
            navigationBarStyle = SystemBarStyle.auto(
                Color.TRANSPARENT,
                Color.TRANSPARENT
            )
        )

        _binding = ActivityMainScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)

        ThemeMode.affirmThemeModeFromSaved(baseContext)

        subscribeToLbEvents()

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false
        )

        val wFragment = supportFragmentManager.findFragmentByTag("Workspace")

        if (wFragment == null) {
            supportFragmentManager.commit {
                setReorderingAllowed(true)
                add<WorkspaceFragment>(R.id.detail_container, "Workspace")
            }
        }

        (application as App).billingClientLifecycle.apply {
            this@MainScreenActivity.lifecycle.addObserver(this)
            billingEvent.observe(this@MainScreenActivity) { billingEvent ->
                when (billingEvent) {
                    is BillingEvent.SuccessfulPurchase -> {
                        model.confirmSubscription(billingEvent.purchaseToken, billingEvent.accountId)
                    }
                    is BillingEvent.NotifyError -> alertModel.notifyError(billingEvent.error)
                    is BillingEvent.NotifyUnrecoverableError -> alertModel.notifyBasicError()
                    is BillingEvent.NotifyErrorMsg -> alertModel.notify(billingEvent.error)
                }.exhaustive
            }
        }

        if (model.exportImportModel.isLoadingOverlayVisible) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(model.exportImportModel.isLoadingOverlayVisible))
        }

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
                        CreateFileDialogFragment.TAG
                    )
                }
                is TransientScreen.Info -> {
                    FileInfoDialogFragment().show(
                        supportFragmentManager,
                        FileInfoDialogFragment.TAG
                    )
                }
                is TransientScreen.Move -> {
                    MoveFileDialogFragment().show(
                        supportFragmentManager,
                        MoveFileDialogFragment.TAG
                    )
                }
                is TransientScreen.Rename -> {
                    RenameFileDialogFragment().show(
                        supportFragmentManager,
                        RenameFileDialogFragment.TAG
                    )
                }
                is TransientScreen.ShareExport -> {
                    finalizeShare(screen.files)
                }
                is TransientScreen.ShareFile -> {
                    supportFragmentManager.commit {
                        add<ShareFileFragment>(R.id.detail_container, ShareFileFragment.TAG)
                        setTransition(FragmentTransaction.TRANSIT_FRAGMENT_FADE)
                        addToBackStack(WorkspaceFragment.BACKSTACK_TAG)

                        slidingPaneLayout.openPane()
                    }
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

        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (supportFragmentManager.findFragmentById(R.id.detail_container) !is WorkspaceFragment) {
                        model.updateMainScreenUI(UpdateMainScreenUI.PopBackstackToWorkspace)
                    } else if (slidingPaneLayout.isSlideable && slidingPaneLayout.isOpen) { // if you are on a small display where only files or an editor show once at a time, you want to handle behavior a bit differently
                        workspaceModel.requestWorkspaceBack()
                    } else if (maybeGetSearchFilesFragment() != null) {
                        updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
                    } else if (maybeGetFilesFragment() == null || maybeGetFilesFragment()?.onBackPressed() == true) {
                        isEnabled = false // Disable this callback to allow normal back behavior
                        onBackPressedDispatcher.onBackPressed()
                    }
                }
            }
        )

        slidingPaneLayout.addPanelSlideListener(object : SlidingPaneLayout.SimplePanelSlideListener() {
            override fun onPanelOpened(panel: View) {
                window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
            }
            override fun onPanelClosed(panel: View) {
                window?.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN)
            }
            override fun onPanelSlide(panel: View, slideOffset: Float) {
            }
        })

        val navController = navHost().navController
        binding.bottomNavigation.setupWithNavController(navController)
    }

    override fun onResume() {
        super.onResume()
        intent.extras?.getString(ShareReceiverActivity.IMPORTED_FILE_KEY)?.let { dest ->
            workspaceModel._openFile.postValue(Pair(dest, false))
            intent.removeExtra(ShareReceiverActivity.IMPORTED_FILE_KEY)
        }
    }

    private fun updateMainScreenUI(update: UpdateMainScreenUI) {
        when (update) {
            is UpdateMainScreenUI.OpenFile -> {
                if (update.id != null) {
                    workspaceModel._openFile.value = Pair(update.id, false)
                } else {
                    if (workspaceModel.currentTab.value != null) {
                        workspaceModel._closeFile.value = workspaceModel.currentTab.value?.id
                    }
                }
            }
            is UpdateMainScreenUI.OpenFileFromSearch -> {
                workspaceModel._openFile.value = Pair(update.id, false)
                navHost().navController.popBackStack()
            }
            is UpdateMainScreenUI.CloseWorkspacePane -> {
                slidingPaneLayout.closePane()
            }
            is UpdateMainScreenUI.OpenWorkspacePane -> {
                slidingPaneLayout.openPane()
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
            UpdateMainScreenUI.PopBackstackToWorkspace -> {
                if (supportFragmentManager.findFragmentById(R.id.detail_container) !is WorkspaceFragment) {
                    supportFragmentManager.popBackStack(WorkspaceFragment.BACKSTACK_TAG, FragmentManager.POP_BACK_STACK_INCLUSIVE)
                }
            }
            UpdateMainScreenUI.ShowSearch -> navHost().navController.navigate(R.id.action_files_to_search)
            UpdateMainScreenUI.ShowFiles -> navHost().navController.popBackStack()
            UpdateMainScreenUI.ToggleBottomViewNavigation -> {
                binding.bottomNavigation.visibility = if (binding.bottomNavigation.isVisible) {
                    View.GONE
                } else {
                    View.VISIBLE
                }
            }
            UpdateMainScreenUI.CloseSlidingPane -> {
                slidingPaneLayout.closePane()
            }
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

    fun syncImportAccount() {
        startActivity(Intent(this, ImportAccountActivity::class.java))
        finishAffinity()
    }

    private fun subscribeToLbEvents() {
        lifecycleScope.launch {
            while (true) {
                val lbEvent = withContext(Dispatchers.IO) {
                    Lb.subscribe(Lb.eventsReceiver)
                }

                lbEvent?.let { event ->
                    val status: LbStatus = jsonParser.decodeFromString(Lb.getStatus())
                    fileTreeViewModel.hydrateStatusUpdate(status, event)
                }
            }
        }
    }
}
