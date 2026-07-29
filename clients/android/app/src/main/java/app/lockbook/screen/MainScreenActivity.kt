@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.content.ClipData
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.view.View
import android.view.WindowManager
import androidx.activity.BackEventCompat
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
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.LbStatus
import java.io.File
import java.lang.ref.WeakReference

class MainScreenActivity : AppCompatActivity() {
    private var _binding: ActivityMainScreenBinding? = null
    val binding get() = _binding!!
    private lateinit var workspaceShell: AdaptiveWorkspaceShell

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    companion object {
        private const val SIDEBAR_SEARCH_FRAGMENT_TAG = "SidebarSearch"
        private const val WORKSPACE_FRAGMENT_TAG = "Workspace"
    }

    private val fragmentFinishedCallback =
        object : FragmentManager.FragmentLifecycleCallbacks() {
            override fun onFragmentDestroyed(
                fm: FragmentManager,
                f: Fragment,
            ) {
                val filesFragment = maybeGetFilesFragment() ?: return

                when (f) {
                    is MoveFileDialogFragment,
                    is RenameFileDialogFragment,
                    -> {
                        filesFragment.reloadFiles()
                    }

                    is CreateFileDialogFragment -> {
                        filesFragment.onNewFileCreated(f.newFile)
                    }

                    is FileInfoDialogFragment -> {
                        filesFragment.unselectFiles()
                    }

                    is DeleteFilesDialogFragment -> {
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        enableEdgeToEdge(
            navigationBarStyle =
                SystemBarStyle.auto(
                    Color.TRANSPARENT,
                    Color.TRANSPARENT,
                ),
        )

        _binding = ActivityMainScreenBinding.inflate(layoutInflater)
        setContentView(binding.root)
        workspaceShell = binding.root
        workspaceShell.setOnModeChangedListener { mode ->
            window?.setSoftInputMode(
                if (mode == WorkspaceShellMode.SidebarOnly) {
                    WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN
                } else {
                    WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING
                },
            )
            (maybeGetFilesFragment() as? FilesListFragment)?.updateOpenDetailButtonVisibility()
            (supportFragmentManager.findFragmentByTag(WORKSPACE_FRAGMENT_TAG) as? WorkspaceFragment)
                ?.let { workspaceFragment ->
                    workspaceFragment.updateRestoreSplitButtonVisibility(mode == WorkspaceShellMode.DetailOnly)
                    workspaceFragment.updateWorkspaceSearchButtonVisibility(mode != WorkspaceShellMode.Split)
                }
        }
        ensureWorkspaceFragment()

        ThemeMode.affirmThemeModeFromSaved(baseContext)

        subscribeToLbEvents()

        supportFragmentManager.registerFragmentLifecycleCallbacks(
            fragmentFinishedCallback,
            false,
        )

        (application as App).billingClientLifecycle.apply {
            this@MainScreenActivity.lifecycle.addObserver(this)
            billingEvent.observe(this@MainScreenActivity) { billingEvent ->
                when (billingEvent) {
                    is BillingEvent.SuccessfulPurchase -> {
                        model.confirmSubscription(billingEvent.purchaseToken, billingEvent.accountId)
                    }

                    is BillingEvent.NotifyError -> {
                        alertModel.notifyError(billingEvent.error)
                    }

                    is BillingEvent.NotifyUnrecoverableError -> {
                        alertModel.notifyBasicError()
                    }

                    is BillingEvent.NotifyErrorMsg -> {
                        alertModel.notify(billingEvent.error)
                    }
                }.exhaustive
            }
        }

        if (model.exportImportModel.isLoadingOverlayVisible) {
            updateMainScreenUI(UpdateMainScreenUI.ShowHideProgressOverlay(model.exportImportModel.isLoadingOverlayVisible))
        }

        model.launchActivityScreen.observe(
            this,
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
            this,
        ) { screen ->
            when (screen) {
                is TransientScreen.Create -> {
                    CreateFileDialogFragment().show(
                        supportFragmentManager,
                        CreateFileDialogFragment.TAG,
                    )
                }

                is TransientScreen.Info -> {
                    FileInfoDialogFragment().show(
                        supportFragmentManager,
                        FileInfoDialogFragment.TAG,
                    )
                }

                is TransientScreen.Move -> {
                    MoveFileDialogFragment().show(
                        supportFragmentManager,
                        MoveFileDialogFragment.TAG,
                    )
                }

                is TransientScreen.Rename -> {
                    RenameFileDialogFragment().show(
                        supportFragmentManager,
                        RenameFileDialogFragment.TAG,
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

                        workspaceShell.showDetail()
                    }
                }

                is TransientScreen.Delete -> {
                    DeleteFilesDialogFragment().show(
                        supportFragmentManager,
                        DeleteFilesDialogFragment.DELETE_FILES_DIALOG_FRAGMENT,
                    )
                }
            }.exhaustive
        }

        model.updateMainScreenUI.observe(
            this,
        ) { update ->
            updateMainScreenUI(update)
        }

        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    println(
                        "LB_DEBUG back detail=${supportFragmentManager.findFragmentById(R.id.detail_container)?.javaClass?.simpleName} " +
                            "shell=${workspaceShell.mode} detailVisible=${workspaceShell.isDetailVisible} " +
                            "search=${maybeGetSearchFilesFragment() != null} sidebarSearch=${isSidebarSearchVisible()}",
                    )
                    val detailFragment = supportFragmentManager.findFragmentById(R.id.detail_container)
                    if (detailFragment != null && detailFragment !is WorkspaceFragment) {
                        println("LB_DEBUG back -> pop detail backstack")
                        model.updateMainScreenUI(UpdateMainScreenUI.PopBackstackToWorkspace)
                    } else if (workspaceShell.isDetailVisible) {
                        println("LB_DEBUG back -> request workspace back")
                        workspaceModel.requestWorkspaceBack()
                    } else if (isSidebarSearchVisible()) {
                        println("LB_DEBUG back -> show detail from sidebar search")
                        updateMainScreenUI(UpdateMainScreenUI.ShowDetailFromSearch())
                    } else if (maybeGetSearchFilesFragment() != null) {
                        println("LB_DEBUG back -> show files")
                        updateMainScreenUI(UpdateMainScreenUI.ShowFiles)
                    } else if (maybeGetFilesFragment() == null || maybeGetFilesFragment()?.onBackPressed() == true) {
                        println("LB_DEBUG back -> default back")
                        isEnabled = false // Disable this callback to allow normal back behavior
                        onBackPressedDispatcher.onBackPressed()
                    }
                }

                override fun handleOnBackStarted(backEvent: BackEventCompat) {
                    workspaceModel.notifyBackGestureStarted()
                    super.handleOnBackStarted(backEvent)
                }
            },
        )

        val navController = navHost().navController
        binding.bottomNavigation.setupWithNavController(navController)
        navController.addOnDestinationChangedListener { _, destination, _ ->
            binding.bottomNavigation.isVisible =
                destination.id in
                setOf(
                    R.id.filesListFragment,
                    R.id.pendingSharesFragment,
                )
        }
    }

    override fun onResume() {
        super.onResume()
        intent.extras?.getString(ShareReceiverActivity.IMPORTED_FILE_KEY)?.let { dest ->
            workspaceModel._openFile.postValue(Pair(dest, false))
            showWorkspaceDetail()
            intent.removeExtra(ShareReceiverActivity.IMPORTED_FILE_KEY)
        }
    }

    private fun updateMainScreenUI(update: UpdateMainScreenUI) {
        println("LB_DEBUG updateMainScreenUI ${update.javaClass.simpleName}")
        when (update) {
            is UpdateMainScreenUI.OpenFile -> {
                if (update.id != null) {
                    workspaceModel._openFile.value = Pair(update.id, true)
                    showWorkspaceDetail()
                } else {
                    if (workspaceModel.currentTab.value != null) {
                        workspaceModel._closeFile.value = workspaceModel.currentTab.value?.id
                    }
                }
            }

            is UpdateMainScreenUI.OpenFileFromSearch -> {
                workspaceModel._openFile.value = Pair(update.id, false)
                if (isSidebarSearchVisible()) {
                    showSidebarFiles()
                    if (update.restoreFocusedDetail) {
                        workspaceShell.focusDetail()
                    } else {
                        showWorkspaceDetail()
                    }
                } else {
                    navHost().navController.popBackStack()
                }
            }

            UpdateMainScreenUI.ShowSidebar -> {
                workspaceShell.showSidebar()
            }

            UpdateMainScreenUI.ShowDetail -> {
                showWorkspaceDetail()
            }

            UpdateMainScreenUI.FocusDetail -> {
                workspaceShell.focusDetail()
            }

            UpdateMainScreenUI.ShowSplitOrSidebar -> {
                workspaceShell.showSplitOrSidebar()
            }

            is UpdateMainScreenUI.NotifyError -> {
                alertModel.notifyError(update.error)
            }

            is UpdateMainScreenUI.ShareDocuments -> {
                finalizeShare(update.files)
            }

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
                val detailFragment = supportFragmentManager.findFragmentById(R.id.detail_container)
                if (detailFragment != null && detailFragment !is WorkspaceFragment) {
                    supportFragmentManager.popBackStack(WorkspaceFragment.BACKSTACK_TAG, FragmentManager.POP_BACK_STACK_INCLUSIVE)
                }
            }

            is UpdateMainScreenUI.ShowSearch -> {
                val args =
                    Bundle().apply {
                        putBoolean(SearchDocumentsFragment.ARG_RETURN_TO_WORKSPACE, update.returnToWorkspace)
                        putBoolean(SearchDocumentsFragment.ARG_RESTORE_FOCUSED_DETAIL, workspaceShell.isDetailOnly)
                    }

                if (update.returnToWorkspace) {
                    showSidebarSearch(args)
                } else {
                    val navController = navHost().navController
                    if (navController.currentDestination?.id != R.id.searchFilesFragment) {
                        navController.navigate(R.id.searchFilesFragment, args)
                    }
                }
            }

            UpdateMainScreenUI.ShowFiles -> {
                navHost().navController.popBackStack()
            }

            is UpdateMainScreenUI.ShowDetailFromSearch -> {
                if (isSidebarSearchVisible()) {
                    showSidebarFiles()
                } else {
                    navHost().navController.popBackStack()
                }
                if (update.restoreFocusedDetail) {
                    workspaceShell.focusDetail()
                } else {
                    showWorkspaceDetail()
                }
            }
        }
    }

    fun isShowingSidebarOnly(): Boolean = workspaceShell.isSidebarOnly

    fun isFocusingDetail(): Boolean = workspaceShell.isDetailOnly

    fun isShowingSplit(): Boolean = workspaceShell.isSplit

    private fun ensureWorkspaceFragment() {
        if (supportFragmentManager.findFragmentByTag(WORKSPACE_FRAGMENT_TAG) != null) {
            return
        }

        supportFragmentManager.commitNow {
            setReorderingAllowed(true)
            add<WorkspaceFragment>(R.id.detail_container, WORKSPACE_FRAGMENT_TAG)
        }
    }

    private fun showWorkspaceDetail() {
        workspaceShell.showDetail()
    }

    private fun showSidebarSearch(args: Bundle) {
        val existingSearchFragment = supportFragmentManager.findFragmentByTag(SIDEBAR_SEARCH_FRAGMENT_TAG)
        if (existingSearchFragment == null) {
            supportFragmentManager.commitNow {
                setReorderingAllowed(true)
                add<SearchDocumentsFragment>(R.id.sidebar_search_container, SIDEBAR_SEARCH_FRAGMENT_TAG, args)
            }
        }

        binding.sidebarFilesContainer.visibility = View.GONE
        binding.sidebarSearchContainer.visibility = View.VISIBLE
        workspaceShell.showSidebar()
    }

    private fun showSidebarFiles() {
        binding.sidebarSearchContainer.visibility = View.GONE
        binding.sidebarFilesContainer.visibility = View.VISIBLE
        binding.bottomNavigation.isVisible = isBottomNavigationDestination()
    }

    private fun isSidebarSearchVisible(): Boolean = binding.sidebarSearchContainer.isVisible

    private fun isBottomNavigationDestination(): Boolean =
        navHost().navController.currentDestination?.id in
            setOf(
                R.id.filesListFragment,
                R.id.pendingSharesFragment,
            )

    private fun finalizeShare(files: List<File>) {
        val uris = ArrayList<Uri>()

        for (file in files) {
            uris.add(
                FileProvider.getUriForFile(
                    this,
                    "app.lockbook.fileprovider",
                    file,
                ),
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
                val lbEvent =
                    withContext(Dispatchers.IO) {
                        Lb.subscribe(Lb.eventsReceiver)
                    }

                lbEvent?.let { event ->
                    val status: LbStatus = Lb.getStatus()
                    fileTreeViewModel.hydrateStatusUpdate(status, event)
                }
            }
        }
    }
}
