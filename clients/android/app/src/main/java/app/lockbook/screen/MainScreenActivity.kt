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
import androidx.core.view.GravityCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.forEach
import androidx.core.view.isVisible
import androidx.core.view.updatePadding
import androidx.fragment.app.*
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.lockbook.App
import app.lockbook.R
import app.lockbook.billing.BillingEvent
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.model.*
import app.lockbook.ui.*
import app.lockbook.util.*
import com.google.android.material.button.MaterialButton
import com.google.android.material.dialog.MaterialAlertDialogBuilder
import com.google.android.material.textview.MaterialTextView
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
    private lateinit var fileSelectionBottomBarController: FileSelectionBottomBarController

    private var isFileSelectionActive = false

    internal val fileSelectionBottomBarView: View
        get() = binding.fileSelectionBottomBar.root

    internal val fileActionSnackbarAnchorView: View
        get() = binding.createFileFab

    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    companion object {
        private const val SIDEBAR_SEARCH_FRAGMENT_TAG = "SidebarSearch"
        private const val FILES_FRAGMENT_TAG = "Files"
        private const val RECENT_FILES_FRAGMENT_TAG = "RecentFiles"
        private const val PENDING_SHARES_FRAGMENT_TAG = "PendingShares"
        private const val CREATE_LINK_FRAGMENT_TAG = "CreateLink"
        private const val WORKSPACE_FRAGMENT_TAG = "Workspace"
    }

    private val fragmentFinishedCallback =
        object : FragmentManager.FragmentLifecycleCallbacks() {
            override fun onFragmentDestroyed(
                fm: FragmentManager,
                f: Fragment,
            ) {
                val filesFragment = currentFilesFragment()

                when (f) {
                    is MoveFileDialogFragment,
                    is RenameFileDialogFragment,
                    -> {
                        fileTreeViewModel.reloadFiles()
                    }

                    is CreateFileDialogFragment -> {
                        filesFragment?.onNewFileCreated(f.newFile)
                    }

                    is FileInfoDialogFragment -> {}

                    is DeleteFilesDialogFragment -> {
                        fileTreeViewModel.reloadFiles()
                    }
                }
                if (f is MoveFileDialogFragment ||
                    f is RenameFileDialogFragment ||
                    f is FileInfoDialogFragment ||
                    f is DeleteFilesDialogFragment
                ) {
                    filesFragment?.unselectFiles()
                }
            }
        }

    private val onExport =
        registerForActivityResult(ActivityResultContracts.StartActivityForResult()) {
            mainScreenModel.showProgressOverlay(false)
            mainScreenModel.exportImportModel.isLoadingOverlayVisible = false

            currentFilesFragment()?.unselectFiles()
        }

    val mainScreenModel: MainScreenViewModel by viewModels()
    val workspaceModel: WorkspaceViewModel by viewModels()
    private val fileSelectionModel: FileSelectionViewModel by viewModels()

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
        ViewCompat.setOnApplyWindowInsetsListener(fileSelectionBottomBarView) { view, windowInsets ->
            view.updatePadding(
                bottom = windowInsets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom,
            )
            windowInsets
        }
        fileSelectionBottomBarController =
            FileSelectionBottomBarController(
                root = fileSelectionBottomBarView,
                onAction = ::dispatchFileSelectionAction,
                onClearSelection = fileSelectionModel::clear,
                onVisibilityChanged = ::setFileSelectionActive,
            )
        binding.createFileFab.setOnClickListener {
            mainScreenModel.launchTransientScreen(
                TransientScreen.Create(fileTreeViewModel.fileModel.parent.id),
            )
        }
        workspaceShell = binding.workspaceShell
        setUpSearch()
        setUpNavigationDrawer()
        workspaceShell.setOnModeChangedListener { mode ->
            window?.setSoftInputMode(
                if (mode == WorkspaceShellMode.SidebarOnly) {
                    WindowManager.LayoutParams.SOFT_INPUT_ADJUST_PAN
                } else {
                    WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING
                },
            )
            updateSearchBarDetailAction()
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
                    BillingEvent.SuccessfulPurchase -> {
                        handleMainUiEffect(MainUiEffect.ShowSubscriptionConfirmed)
                    }

                    is BillingEvent.GooglePlayPurchase -> {
                        mainScreenModel.confirmSubscription(billingEvent.purchaseToken, billingEvent.accountId)
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

        if (mainScreenModel.exportImportModel.isLoadingOverlayVisible) {
            handleMainUiEffect(MainUiEffect.ShowHideProgressOverlay(mainScreenModel.exportImportModel.isLoadingOverlayVisible))
        }

        mainScreenModel.launchActivityScreen.observe(
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

        mainScreenModel.launchTransientScreen.observe(
            this,
        ) { screen ->
            when (screen) {
                is TransientScreen.Create -> {
                    CreateFileBottomSheetFragment
                        .newInstance(
                            initialParentId = screen.parentId,
                            focusedFolderId = fileTreeViewModel.fileModel.parent.id,
                        ).show(
                            supportFragmentManager,
                            CreateFileBottomSheetFragment.TAG,
                        )
                }

                is TransientScreen.Info -> {
                    FileInfoDialogFragment.newInstance(screen.file.id).show(
                        supportFragmentManager,
                        FileInfoDialogFragment.TAG,
                    )
                }

                is TransientScreen.Share -> {
                    ShareFileBottomSheetFragment.newInstance(screen.file.id).show(
                        supportFragmentManager,
                        ShareFileBottomSheetFragment.TAG,
                    )
                }

                is TransientScreen.Move -> {
                    MoveFileDialogFragment.newInstance(screen.files.map { it.id }).show(
                        supportFragmentManager,
                        MoveFileDialogFragment.TAG,
                    )
                }

                is TransientScreen.Rename -> {
                    RenameFileDialogFragment.newInstance(screen.file.id).show(
                        supportFragmentManager,
                        RenameFileDialogFragment.TAG,
                    )
                }

                is TransientScreen.ShareExport -> {
                    finalizeShare(screen.files)
                }

                is TransientScreen.Delete -> {
                    DeleteFilesDialogFragment.newInstance(screen.files.map { it.id }).show(
                        supportFragmentManager,
                        DeleteFilesDialogFragment.DELETE_FILES_DIALOG_FRAGMENT,
                    )
                }
            }.exhaustive
        }

        mainScreenModel.mainUiEffect.observe(
            this,
        ) { effect ->
            handleMainUiEffect(effect)
        }

        lifecycleScope.launch {
            repeatOnLifecycle(Lifecycle.State.STARTED) {
                launch {
                    mainScreenModel.navigationState.collect(::renderNavigation)
                }
                launch {
                    mainScreenModel.navigationEffects.collect(::handleNavigationEffect)
                }
                launch {
                    fileSelectionModel.uiState.collect(fileSelectionBottomBarController::render)
                }
            }
        }

        onBackPressedDispatcher.addCallback(
            this,
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (binding.mainDrawer.isDrawerOpen(GravityCompat.START)) {
                        binding.mainDrawer.closeDrawer(GravityCompat.START)
                        return
                    }

                    if (mainScreenModel.navigate(MainNavigationAction.Back)) {
                        return
                    }

                    if (workspaceShell.isDetailOnly) {
                        workspaceModel.requestWorkspaceBack()
                    } else if (shouldExitFromCurrentSidebar()) {
                        isEnabled = false // Disable this callback to allow normal back behavior
                        onBackPressedDispatcher.onBackPressed()
                    }
                }

                override fun handleOnBackStarted(backEvent: BackEventCompat) {
                    if (workspaceShell.isDetailOnly) {
                        workspaceModel.notifyBackGestureStarted()
                    }
                    super.handleOnBackStarted(backEvent)
                }
            },
        )

        binding.bottomNavigation.setOnItemSelectedListener { item ->
            if (isFileSelectionActive) return@setOnItemSelectedListener false
            val destination =
                when (item.itemId) {
                    R.id.filesListFragment -> SidebarRoot.Files
                    R.id.recentFilesFragment -> SidebarRoot.Recents
                    R.id.pendingSharesFragment -> SidebarRoot.PendingShares
                    else -> return@setOnItemSelectedListener false
                }
            mainScreenModel.navigate(MainNavigationAction.SelectSidebar(destination))
            true
        }
    }

    internal fun setFileSelectionActive(isActive: Boolean) {
        isFileSelectionActive = isActive
        binding.sidebarSearchBar.isEnabled = !isActive
        binding.bottomNavigation.isEnabled = !isActive
        binding.bottomNavigation.menu.forEach { item ->
            item.isEnabled = !isActive
        }
        binding.sidebarSearchBar
            .menu
            .findItem(R.id.menu_files_list_open_ws)
            ?.isEnabled = !isActive
    }

    private fun dispatchFileSelectionAction(
        action: FileSelectionAction,
        files: List<net.lockbook.File>,
    ) {
        when (val fragment = currentFilesFragment()) {
            is FilesListFragment -> fragment.dispatchSelectionAction(action, files)
            is RecentFilesFragment -> fragment.dispatchSelectionAction(action, files)
            else -> Unit
        }
    }

    fun openNavigationDrawer() {
        binding.mainDrawer.openDrawer(GravityCompat.START)
    }

    private fun setUpSearch() {
        binding.sidebarSearchBar.setNavigationOnClickListener {
            if (!isFileSelectionActive) openNavigationDrawer()
        }
        configureSidebarSearchLauncher()
        binding.sidebarSearchBar.setOnMenuItemClickListener { item ->
            if (item.itemId == R.id.menu_files_list_open_ws && !isFileSelectionActive) {
                mainScreenModel.navigate(MainNavigationAction.FocusDetail)
                true
            } else {
                false
            }
        }
        binding.sidebarSearchBar.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ ->
            updateSearchBarDetailAction()
        }
        updateSearchBarDetailAction()
    }

    internal fun configureSidebarSearchLauncher() {
        binding.sidebarSearchBar.setOnClickListener {
            if (!isFileSelectionActive) {
                mainScreenModel.navigate(MainNavigationAction.OpenSearch(SearchPresentation.SidebarMorph))
            }
        }
    }

    private fun updateSearchBarDetailAction() {
        binding.sidebarSearchBar
            .menu
            .findItem(R.id.menu_files_list_open_ws)
            ?.isVisible = !isFocusingDetail()
    }

    private fun setUpNavigationDrawer() {
        val header = binding.navigationView.getHeaderView(0)
        val lastSynced = header.findViewById<MaterialTextView>(R.id.filesListLastSynced)
        val localDirty = header.findViewById<MaterialTextView>(R.id.filesListLocalDirty)
        val serverDirty = header.findViewById<MaterialTextView>(R.id.filesListServerDirty)
        var localDirtyCount = 0
        var serverDirtyCount = 0

        fun updateDirtyFileStatuses() {
            localDirty.isVisible = localDirtyCount != 0
            serverDirty.isVisible = serverDirtyCount != 0
        }

        updateDirtyFileStatuses()
        fileTreeViewModel.syncStatus.observe(this) {
            lastSynced.text = getString(R.string.last_sync, it)
        }
        fileTreeViewModel.dirtyLocally.observe(this) {
            localDirtyCount = it.size
            localDirty.text =
                resources.getQuantityString(R.plurals.files_to_push, localDirtyCount, localDirtyCount)
            updateDirtyFileStatuses()
        }
        fileTreeViewModel.pushingFiles.observe(this) {
            serverDirtyCount = it.size
            serverDirty.text =
                resources.getQuantityString(R.plurals.files_to_pull, serverDirtyCount, serverDirtyCount)
            updateDirtyFileStatuses()
        }

        header.findViewById<MaterialButton>(R.id.set_theme).setOnClickListener {
            var selected = ThemeMode.getSavedThemeIndex(this)

            MaterialAlertDialogBuilder(this)
                .setTitle("Choose your theme")
                .setSingleChoiceItems(ThemeMode.getThemeModes(this), selected) { _, new ->
                    selected = new
                }.setPositiveButton("Apply") { _, _ ->
                    ThemeMode.saveAndSetThemeIndex(this, selected)
                }.setNegativeButton(R.string.cancel) { dialog, _ ->
                    dialog.dismiss()
                }.show()
        }

        header.findViewById<MaterialButton>(R.id.launch_settings).setOnClickListener {
            mainScreenModel.launchActivityScreen(ActivityScreen.Settings())
            binding.mainDrawer.closeDrawer(GravityCompat.START)
        }
    }

    override fun onResume() {
        super.onResume()
        intent.extras?.getString(ShareReceiverActivity.IMPORTED_FILE_KEY)?.let { dest ->
            mainScreenModel.navigate(MainNavigationAction.OpenDocument(dest, newFile = false))
            intent.removeExtra(ShareReceiverActivity.IMPORTED_FILE_KEY)
        }
    }

    private fun handleMainUiEffect(effect: MainUiEffect) {
        when (effect) {
            is MainUiEffect.NotifyError -> {
                alertModel.notifyError(effect.error)
            }

            is MainUiEffect.ShareDocuments -> {
                finalizeShare(effect.files)
            }

            is MainUiEffect.ShowHideProgressOverlay -> {
                if (effect.show) {
                    Animate.animateVisibility(binding.progressOverlay, View.VISIBLE, 100, 500)
                } else {
                    Animate.animateVisibility(binding.progressOverlay, View.GONE, 0, 500)
                }
            }

            MainUiEffect.ShowSubscriptionConfirmed -> {
                alertModel.notifySuccessfulPurchaseConfirm()
            }
        }
    }

    fun isFocusingDetail(): Boolean = workspaceShell.isDetailOnly

    fun isShowingSplit(): Boolean = workspaceShell.isSplit

    private fun renderNavigation(state: MainNavigationState) {
        renderSidebar(state.sidebar)
        renderPaneIntent(state.paneIntent)
        renderSearchOverlay(state.searchOverlay)
    }

    private fun handleNavigationEffect(effect: MainNavigationEffect) {
        when (effect) {
            is MainNavigationEffect.OpenDocument -> {
                workspaceModel.openFile(
                    OpenFileRequest(
                        id = effect.id,
                        newFile = effect.newFile,
                        presentation = OpenFilePresentation.Preserve,
                    ),
                )
            }
        }
    }

    private fun renderPaneIntent(intent: PaneIntent) {
        when (intent) {
            PaneIntent.Automatic -> workspaceShell.showAutomatic()
            PaneIntent.Sidebar -> workspaceShell.showSidebar()
            PaneIntent.Detail -> workspaceShell.showDetail()
            PaneIntent.FocusDetail -> workspaceShell.focusDetail()
            PaneIntent.SplitOrSidebar -> workspaceShell.showSplitOrSidebar()
        }
    }

    private fun renderSidebar(destination: SidebarDestination) {
        val targetTag =
            when (destination) {
                SidebarDestination.Files -> FILES_FRAGMENT_TAG
                SidebarDestination.Recents -> RECENT_FILES_FRAGMENT_TAG
                SidebarDestination.PendingShares -> PENDING_SHARES_FRAGMENT_TAG
                is SidebarDestination.CreateLink -> CREATE_LINK_FRAGMENT_TAG
            }
        val existingTarget =
            supportFragmentManager.findFragmentByTag(targetTag)?.takeUnless { fragment ->
                destination is SidebarDestination.CreateLink &&
                    fragment.arguments?.getString(CreateLinkFragment.CREATE_LINK_FILE_ID_KEY) != destination.fileId
            }
        val target =
            existingTarget
                ?: when (destination) {
                    SidebarDestination.Files -> {
                        FilesListFragment()
                    }

                    SidebarDestination.Recents -> {
                        RecentFilesFragment()
                    }

                    SidebarDestination.PendingShares -> {
                        PendingSharesFragment()
                    }

                    is SidebarDestination.CreateLink -> {
                        CreateLinkFragment().apply {
                            arguments =
                                Bundle().apply {
                                    putString(CreateLinkFragment.CREATE_LINK_FILE_ID_KEY, destination.fileId)
                                }
                        }
                    }
                }
        val sidebarFragments = supportFragmentManager.fragments.filter { it.id == R.id.files_container }

        supportFragmentManager.commitNow {
            setReorderingAllowed(true)
            sidebarFragments.forEach { fragment ->
                when {
                    fragment === target -> {
                        show(fragment)
                        setMaxLifecycle(fragment, Lifecycle.State.RESUMED)
                    }

                    fragment.tag == FILES_FRAGMENT_TAG ||
                        fragment.tag == RECENT_FILES_FRAGMENT_TAG ||
                        fragment.tag == PENDING_SHARES_FRAGMENT_TAG
                    -> {
                        hide(fragment)
                        setMaxLifecycle(fragment, Lifecycle.State.STARTED)
                    }

                    else -> {
                        remove(fragment)
                    }
                }
            }

            if (!target.isAdded) {
                add(R.id.files_container, target, targetTag)
                setMaxLifecycle(target, Lifecycle.State.RESUMED)
            }
        }

        val rootDestinationId =
            when (destination) {
                SidebarDestination.Files -> {
                    R.id.filesListFragment
                }

                SidebarDestination.Recents -> {
                    R.id.recentFilesFragment
                }

                SidebarDestination.PendingShares -> {
                    R.id.pendingSharesFragment
                }

                is SidebarDestination.CreateLink -> {
                    null
                }
            }
        val showsRootChrome = destination !is SidebarDestination.CreateLink
        binding.sidebarSearchAppBar.isVisible = showsRootChrome
        binding.bottomNavigation.isVisible = showsRootChrome
        binding.createFileFab.isVisible = showsRootChrome
        rootDestinationId?.let { itemId ->
            if (binding.bottomNavigation.selectedItemId != itemId) {
                binding.bottomNavigation.selectedItemId = itemId
            }
        }
    }

    private fun renderSearchOverlay(searchOverlay: SearchOverlay?) {
        if (searchOverlay != null) {
            ensureSearchFragment(searchOverlay.presentation)
        } else {
            removeSearchFragment()
        }
    }

    private fun ensureSearchFragment(presentation: SearchPresentation) {
        val existing = supportFragmentManager.findFragmentByTag(SIDEBAR_SEARCH_FRAGMENT_TAG)
        if (
            existing?.id == R.id.search_overlay_container &&
            (existing as? SearchDocumentsFragment)?.presentation == presentation
        ) {
            return
        }

        supportFragmentManager.commitNow {
            setReorderingAllowed(true)
            existing?.let(::remove)
            add(
                R.id.search_overlay_container,
                SearchDocumentsFragment.newInstance(presentation),
                SIDEBAR_SEARCH_FRAGMENT_TAG,
            )
        }
    }

    private fun removeSearchFragment() {
        supportFragmentManager
            .findFragmentByTag(SIDEBAR_SEARCH_FRAGMENT_TAG)
            ?.takeIf { it.id == R.id.search_overlay_container }
            ?.let { fragment ->
                supportFragmentManager.commitNow {
                    setReorderingAllowed(true)
                    remove(fragment)
                }
            }
    }

    private fun shouldExitFromCurrentSidebar(): Boolean =
        when (mainScreenModel.navigationState.value.sidebar) {
            SidebarDestination.Files -> {
                maybeGetFilesFragment()?.onBackPressed() ?: true
            }

            SidebarDestination.Recents -> {
                currentFilesFragment()?.onBackPressed() ?: true
            }

            SidebarDestination.PendingShares -> {
                !(
                    (supportFragmentManager.findFragmentByTag(PENDING_SHARES_FRAGMENT_TAG) as? PendingSharesFragment)
                        ?.onBackPressed() ?: false
                )
            }

            is SidebarDestination.CreateLink,
            -> {
                true
            }
        }

    private fun currentFilesFragment(): FilesFragment? =
        when (mainScreenModel.navigationState.value.sidebar) {
            SidebarDestination.Files -> {
                maybeGetFilesFragment()
            }

            SidebarDestination.Recents -> {
                supportFragmentManager.findFragmentByTag(RECENT_FILES_FRAGMENT_TAG) as? RecentFilesFragment
            }

            else -> {
                null
            }
        }

    private fun ensureWorkspaceFragment() {
        if (supportFragmentManager.findFragmentByTag(WORKSPACE_FRAGMENT_TAG) != null) {
            return
        }

        supportFragmentManager.commitNow {
            setReorderingAllowed(true)
            add<WorkspaceFragment>(R.id.detail_container, WORKSPACE_FRAGMENT_TAG)
        }
    }

    private fun finalizeShare(files: List<File>) {
        val uris = ArrayList<Uri>()

        for (file in files) {
            uris.add(
                FileProvider.getUriForFile(
                    this,
                    "$packageName.fileprovider",
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
