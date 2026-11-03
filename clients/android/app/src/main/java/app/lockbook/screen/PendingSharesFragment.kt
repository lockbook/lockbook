@file:Suppress("ktlint:standard:no-wildcard-imports")

package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.widget.PopupMenu
import androidx.core.view.updatePadding
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.asLiveData
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.R
import app.lockbook.databinding.FragmentPendingSharesBinding
import app.lockbook.databinding.FragmentTabBinding
import app.lockbook.model.*
import app.lockbook.model.MoveFileViewModel.Companion.PARENT_ID
import app.lockbook.ui.*
import app.lockbook.util.*
import com.google.android.material.tabs.TabLayout
import com.google.android.material.tabs.TabLayoutMediator
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import net.lockbook.File
import net.lockbook.File.FileType
import java.util.Locale
import kotlin.collections.component1
import kotlin.collections.component2
import kotlin.collections.emptyList
import kotlin.collections.map
import kotlin.getValue

class PendingSharesFragment : Fragment() {
    lateinit var binding: FragmentPendingSharesBinding

    private var tabMediator: TabLayoutMediator? = null
    val idsAndFiles =
        FileMetadataStore.snapshots
            .map { snapshot -> snapshot?.pendingShares.orEmpty().associateBy { it.id } }
            .distinctUntilChanged()
            .asLiveData()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        binding = FragmentPendingSharesBinding.inflate(inflater, container, false)

        val tabLayout = binding.tabLayout
        val viewPager = binding.viewPager

        tabLayout.addOnTabSelectedListener(
            object : TabLayout.OnTabSelectedListener {
                override fun onTabSelected(tab: TabLayout.Tab?) {
                    (activity as? MainScreenActivity)?.showBottomNavigation()
                }

                override fun onTabUnselected(tab: TabLayout.Tab?) {}

                override fun onTabReselected(tab: TabLayout.Tab?) {
                    tab ?: return
                    (childFragmentManager.findFragmentByTag("f${tab.position}") as? TabFragment)
                        ?.setTabDefaultFiles()
                }
            },
        )

        idsAndFiles.observe(
            viewLifecycleOwner,
            { it ->
                val sharers =
                    it.values
                        .sortedByDescending { file -> file.lastModified }
                        .flatMap { file ->
                            file.shares.map { share -> share.sharedBy.capitalized() }
                        }.distinct()
                        .toMutableList()

                if (sharers.isEmpty()) {
                    binding.pendingSharesEmptyState.visibility = View.VISIBLE
                    binding.tabLayout.visibility = View.GONE
                    binding.viewPager.visibility = View.GONE
                } else {
                    binding.pendingSharesEmptyState.visibility = View.GONE
                    binding.tabLayout.visibility = View.VISIBLE
                    binding.viewPager.visibility = View.VISIBLE

                    sharers.add(0, "All")
                    val existsTabChange =
                        sharers.size != tabLayout.tabCount ||
                            (0 until tabLayout.tabCount).any { i ->
                                tabLayout.getTabAt(i)?.text?.toString() != sharers[i]
                            }

                    if (existsTabChange) {
                        tabMediator?.detach() // Detach old one first
                        val adapter = TabPagerAdapter(this, sharers)
                        viewPager.adapter = adapter
                        tabMediator =
                            TabLayoutMediator(tabLayout, viewPager) { tab, position ->
                                tab.text = sharers[position]
                            }.apply { attach() }
                    }
                }
            },
        )

        return binding.root
    }

    fun onBackPressed(): Boolean =
        (childFragmentManager.findFragmentByTag("f${binding.viewPager.currentItem}") as? TabFragment)
            ?.onBackPressed() ?: false
}

class TabPagerAdapter(
    activity: PendingSharesFragment,
    val tabs: List<String>,
) : FragmentStateAdapter(activity) {
    override fun getItemCount(): Int = tabs.size

    override fun createFragment(position: Int): Fragment = TabFragment.newInstance(tabs[position])
}

class TabFragment : Fragment() {
    private var _binding: FragmentTabBinding? = null
    val binding get() = _binding!!

    private val mainScreenModel: MainScreenViewModel by activityViewModels()

    var currentParent: File? = null

    private lateinit var sharedFilesAdapter: SharedFilesAdapter
    private var originalListPaddingBottom = 0

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?,
    ): View {
        _binding = FragmentTabBinding.inflate(inflater, container, false)

        val pendingSharesFragment = parentFragment as? PendingSharesFragment

        pendingSharesFragment?.idsAndFiles?.observe(
            viewLifecycleOwner,
            { files ->
                currentParent = files[currentParent?.id]
                setFilesGroupedByDate()
            },
        )

        sharedFilesAdapter =
            SharedFilesAdapter(
                onFileClick = ::openSharedFile,
                onPendingMenuClick = ::showPendingShareMenu,
            )
        binding.sharedFilesList.layoutManager = LinearLayoutManager(requireContext())
        binding.sharedFilesList.adapter = sharedFilesAdapter
        binding.sharedFilesList.itemAnimator = null
        originalListPaddingBottom = binding.sharedFilesList.paddingBottom
        (requireActivity() as MainScreenActivity).configureBottomNavigationFor(binding.sharedFilesList) { height ->
            if (_binding == null) return@configureBottomNavigationFor
            binding.sharedFilesList.updatePadding(bottom = originalListPaddingBottom + height)
        }

        val backPressedCallback =
            object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (onBackPressed()) {
                        // If onBackPressed() returns true, it means the fragment handled the
                        // back press (e.g., navigated up a folder), so we do nothing more.
                        return
                    }

                    isEnabled = false
                    requireActivity().onBackPressedDispatcher.onBackPressed()
                }
            }
        requireActivity().onBackPressedDispatcher.addCallback(viewLifecycleOwner, backPressedCallback)

        return binding.root
    }

    private fun showPendingShareMenu(
        view: View,
        item: File,
    ) {
        val popup = PopupMenu(view.context, view)

        popup.menuInflater.inflate(R.menu.menu_pending_shares_file_item, popup.menu)

        popup.setOnMenuItemClickListener { menuItem ->
            when (menuItem.itemId) {
                R.id.accept_share -> {
                    mainScreenModel.navigate(MainNavigationAction.CreateLink(item.id))
                    true
                }

                R.id.refuse_share -> {
                    DeleteSharedDialogFragment.newInstance(arrayListOf(item)).show(
                        parentFragmentManager, // Use this instead of requireActivity()...
                        DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT,
                    )
                    true
                }

                else -> {
                    false
                }
            }
        }

        popup.show()
    }

    private fun openSharedFile(item: File) {
        if (item.type == FileType.Folder) {
            (activity as? MainScreenActivity)?.showBottomNavigation()
            if (item.id == PARENT_ID) {
                promoteCurrentParent()
            } else {
                currentParent = item
            }
            setFilesGroupedByDate()
        } else {
            mainScreenModel.navigate(MainNavigationAction.OpenDocument(item.id, newFile = false))
        }
    }

    override fun onDestroyView() {
        binding.sharedFilesList.adapter = null
        super.onDestroyView()
        _binding = null
    }

    fun onBackPressed(): Boolean =
        if (currentParent == null) {
            false
        } else {
            (activity as? MainScreenActivity)?.showBottomNavigation()
            promoteCurrentParent()
            setFilesGroupedByDate()
            true
        }

    fun setTabDefaultFiles() {
        if (currentParent != null) {
            (activity as? MainScreenActivity)?.showBottomNavigation()
        }
        currentParent = null
        setFilesGroupedByDate()
    }

    fun promoteCurrentParent() {
        val pendingSharesFragment = parentFragment as? PendingSharesFragment

        val grandparent =
            pendingSharesFragment?.idsAndFiles?.value?.get(currentParent?.parent)
        currentParent = grandparent
    }

    fun setFilesGroupedByDate() {
        val sharer = arguments?.getString("tab_name") ?: ""
        val isAllTab = sharer == "All"

        val files =
            (parentFragment as? PendingSharesFragment)?.idsAndFiles?.value?.map { (_, file) -> file }?.filter { file ->
                if (currentParent == null) {
                    if (isAllTab) {
                        !file.shares.isEmpty()
                    } else {
                        file.shares.any { share -> share.sharedBy.capitalized() == sharer }
                    }
                } else {
                    file.parent == currentParent?.id
                }
            } ?: emptyList()

        val rows = mutableListOf<SharedListItem>()
        if (currentParent != null) {
            val parent = File()
            parent.id = PARENT_ID
            parent.parent = ""
            parent.type = FileType.Folder
            parent.name = "..."
            parent.lastModified = 0
            parent.lastModifiedBy = ""
            parent.shares = emptyArray()
            rows +=
                SharedListItem.FileItem(
                    file = parent,
                    positionInSection = 0,
                    sectionSize = 1,
                )
        }

        fun addSection(
            period: RecentPeriod,
            sectionFiles: List<File>,
        ) {
            if (sectionFiles.isEmpty()) {
                return
            }
            rows += SharedListItem.Section(period, collapsed = false)
            sectionFiles.forEachIndexed { index, file ->
                rows +=
                    SharedListItem.FileItem(
                        file = file,
                        subtitle =
                            if (currentParent == null && isAllTab && file.shares.isNotEmpty()) {
                                getString(R.string.shared_by, file.shares[0].sharedBy)
                            } else {
                                null
                            },
                        showPendingMenu = currentParent == null,
                        positionInSection = index,
                        sectionSize = sectionFiles.size,
                    )
            }
        }

        groupFilesByRecentPeriod(files).forEach { group ->
            addSection(group.period, group.files)
        }
        sharedFilesAdapter.submitSharedFiles(rows)
    }

    companion object {
        fun newInstance(tabName: String): TabFragment {
            val fragment = TabFragment()
            val args = Bundle()
            args.putString("tab_name", tabName)
            fragment.arguments = args
            return fragment
        }
    }
}

fun String.capitalized(): String =
    this.replaceFirstChar {
        if (it.isLowerCase()) {
            it.titlecase(Locale.getDefault())
        } else {
            it.toString()
        }
    }
