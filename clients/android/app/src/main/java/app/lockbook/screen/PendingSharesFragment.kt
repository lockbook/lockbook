package app.lockbook.screen

import android.annotation.SuppressLint
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.widget.PopupMenu
import androidx.fragment.app.Fragment
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.MutableLiveData
import androidx.navigation.fragment.findNavController
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.R
import app.lockbook.databinding.FragmentPendingSharesBinding
import app.lockbook.databinding.FragmentTabBinding
import app.lockbook.model.*
import app.lockbook.model.MoveFileViewModel.Companion.PARENT_ID
import app.lockbook.ui.*
import app.lockbook.util.*
import com.afollestad.recyclical.datasource.emptyDataSource
import com.afollestad.recyclical.setup
import com.afollestad.recyclical.withItem
import com.google.android.material.tabs.TabLayout
import com.google.android.material.tabs.TabLayoutMediator
import kotlinx.coroutines.*
import net.lockbook.File
import net.lockbook.File.FileType
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference
import java.util.Locale
import kotlin.collections.component1
import kotlin.collections.component2
import kotlin.collections.emptyList
import kotlin.collections.map
import kotlin.getValue

class PendingSharesFragment : Fragment() {
    lateinit var binding: FragmentPendingSharesBinding

    private val uiScope = CoroutineScope(Dispatchers.Main + Job())

    private val alertModel by lazy {
        AlertModel(WeakReference(requireActivity()), view)
    }

    private var tabMediator: TabLayoutMediator? = null
    val idsAndFiles = MutableLiveData<Map<String, File>>()

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        binding = FragmentPendingSharesBinding.inflate(inflater, container, false)

        populatePendingShares()

        val tabLayout = binding.tabLayout
        val viewPager = binding.viewPager

        tabLayout.addOnTabSelectedListener(object : TabLayout.OnTabSelectedListener {
            override fun onTabSelected(tab: TabLayout.Tab?) {}
            override fun onTabUnselected(tab: TabLayout.Tab?) {}
            override fun onTabReselected(tab: TabLayout.Tab?) {

                tab ?: return
                if (tab.position == 0) {
                    populatePendingShares()
                }
                (childFragmentManager.findFragmentByTag("f${tab.position}") as? TabFragment)
                    ?.setTabDefaultFiles()
            }
        })

        idsAndFiles.observe(
            viewLifecycleOwner,
            { it ->
                println("3asba: observing ids and files. it's now: ${it.size}")
                val sharers = it.values
                    .sortedByDescending { file -> file.lastModified }
                    .flatMap { file ->
                        file.shares.map { share -> share.sharedBy.capitalized() }
                    }
                    .distinct()
                    .toMutableList()

                if (sharers.isEmpty()) {
                    binding.pendingSharesEmptyState.visibility = View.VISIBLE
                    binding.tabsContainer.visibility = View.GONE
                } else {
                    binding.pendingSharesEmptyState.visibility = View.GONE
                    binding.tabsContainer.visibility = View.VISIBLE

                    sharers.add(0, "All")
                    val existsTabChange = sharers.size != tabLayout.tabCount ||
                        (0 until tabLayout.tabCount).any { i ->
                            tabLayout.getTabAt(i)?.text?.toString() != sharers[i]
                        }

                    if (existsTabChange) {
                        tabMediator?.detach() // Detach old one first
                        val adapter = TabPagerAdapter(this, sharers)
                        viewPager.adapter = adapter
                        tabMediator = TabLayoutMediator(tabLayout, viewPager) { tab, position ->
                            tab.text = sharers[position]
                        }.apply { attach() }
                    }
                }
            }
        )

        return binding.root
    }

    override fun onResume() {
        super.onResume()
        populatePendingShares()
    }

    private fun populatePendingShares() {
        uiScope.launch(Dispatchers.IO) {

            try {
                val pendingShares = Lb.getPendingShareFiles().toList()

                withContext(Dispatchers.Main) {

                    println("3asba: populating pending shares with ${pendingShares.size}")

                    idsAndFiles.value = pendingShares.associateBy { item -> item.id }
                }
            } catch (err: LbError) {
                alertModel.notifyError(err)
            }
        }
    }

    fun handleShareRejected(deletedFileId: String) {
        val currentMap = idsAndFiles.value?.toMutableMap() ?: return

        currentMap.remove(deletedFileId)
        uiScope.launch {
            idsAndFiles.value = currentMap
        }
    }
}

class TabPagerAdapter(
    activity: PendingSharesFragment,
    val tabs: List<String>
) : FragmentStateAdapter(activity) {

    override fun getItemCount(): Int = tabs.size

    override fun createFragment(position: Int): Fragment {
        return TabFragment.newInstance(tabs[position])
    }
}

class TabFragment : Fragment() {
    private var _binding: FragmentTabBinding? = null
    private val binding get() = _binding!!

    private val activityModel: StateViewModel by activityViewModels()

    var currentParent: File? = null

    /** Could be a file or a string that denotes the date group in which the file is **/
    var files = emptyDataSource()

    @SuppressLint("SetTextI18n")
    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _binding = FragmentTabBinding.inflate(inflater, container, false)

        val pendingSharesFragment = parentFragment as? PendingSharesFragment
        val sharer = arguments?.getString("tab_name") ?: ""
        val isAllTab = sharer == "All"

        pendingSharesFragment?.idsAndFiles?.observe(
            viewLifecycleOwner,
            { it ->
                setTabDefaultFiles()
            }
        )

        binding.sharedFilesList.setup {

            withDataSource(files)
            withItem<File, SharedFileViewHolder>(R.layout.pending_shares_file_item) {
                onBind(::SharedFileViewHolder) { _, item ->
                    name.text = item.name
                    if (currentParent == null && isAllTab && item.shares.isNotEmpty()) {
                        owner.text = "by: " + item.shares[0]?.sharedBy
                        owner.visibility = View.VISIBLE
                    } else {
                        owner.visibility = View.GONE
                    }

                    if (currentParent == null) {
                        openMenu.visibility = View.VISIBLE
                    } else {
                        openMenu.visibility = View.GONE
                    }

                    openMenu.setOnClickListener { view ->
                        val popup = PopupMenu(view.context, view)

                        popup.menuInflater.inflate(R.menu.menu_pending_shares_file_item, popup.menu)

                        popup.setOnMenuItemClickListener { menuItem ->
                            when (menuItem.itemId) {
                                R.id.accept_share -> {

                                    val bundle = Bundle()
                                    bundle.putString(CreateLinkFragment.CREATE_LINK_FILE_ID_KEY, item.id)
                                    val parentNavController = requireParentFragment().findNavController()

                                    parentNavController.navigate(R.id.action_create_link, bundle)
                                    true
                                }
                                R.id.refuse_share -> {
                                    DeleteSharedDialogFragment.newInstance(arrayListOf(item)).show(
                                        parentFragmentManager, // Use this instead of requireActivity()...
                                        DeleteSharedDialogFragment.DELETE_SHARED_DIALOG_FRAGMENT
                                    )
                                    true
                                }
                                else -> false
                            }
                        }

                        popup.show()
                    }
                    icon.setImageResource(item.getIconResource())
                }
                onClick { _ ->
                    if (item.type == FileType.Folder) {
                        if (item.id == PARENT_ID) {
                            promoteCurrentParent()
                        } else {
                            currentParent = item
                        }
                        setFilesGroupedByDate()
                    } else {
                        activityModel.updateMainScreenUI(UpdateMainScreenUI.OpenFile(item.id))
                    }
                }
            }
            withItem<String, SeparatorViewHolder>(R.layout.share_seperator_item) {
                onBind(::SeparatorViewHolder) { _, item ->
                    date.text = item
                }
            }
        }

        parentFragmentManager.setFragmentResultListener(DeleteSharedDialogFragment.DELETE_SHARE_REQUEST_KEY, this) { _, bundle ->
            val deletedFileId = bundle.getString(DeleteSharedDialogFragment.DELETE_SHARE_BUNDLE_KEY)
            println("3asba: recieved deleted file id $deletedFileId")
            deletedFileId?.let { id ->
                pendingSharesFragment?.handleShareRejected(id)
            }
        }

        val backPressedCallback = object : OnBackPressedCallback(true) {
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

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        val sharedFilesList = binding.sharedFilesList

        (activity as? BottomNavProvider)?.doWhenBottomNavMeasured { bottomNavHeight ->
            sharedFilesList.setPadding(
                sharedFilesList.paddingLeft,
                sharedFilesList.paddingTop,
                sharedFilesList.paddingRight,
                bottomNavHeight
            )
        }
    }

    override fun onDestroyView() {
        super.onDestroyView()
        _binding = null
    }

    fun onBackPressed(): Boolean {
        return if (currentParent == null) {
            false
        } else {
            promoteCurrentParent()
            setFilesGroupedByDate()
            true
        }
    }

    fun setTabDefaultFiles() {
        currentParent = null
        setFilesGroupedByDate()
    }

    fun promoteCurrentParent() {
        val pendingSharesFragment = parentFragment as? PendingSharesFragment

        val grandparent =
            pendingSharesFragment?.idsAndFiles?.value?.get(currentParent?.parent)
        currentParent = grandparent
    }
    /**
     * group a list of files into 4 buckets:
     * - this week
     * - this month
     * - this year
     * - older
     */
    fun setFilesGroupedByDate() {
        val sharer = arguments?.getString("tab_name") ?: ""
        val isAllTab = sharer == "All"

        val files = (parentFragment as? PendingSharesFragment)?.idsAndFiles?.value?.map { (_, file) -> file }?.filter { file ->
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

        val calendar = java.util.Calendar.getInstance()

        val oneWeekAgo = calendar.clone() as java.util.Calendar
        oneWeekAgo.add(java.util.Calendar.DAY_OF_YEAR, -7)
        val oneWeekAgoMillis = oneWeekAgo.timeInMillis
        val oneMonthAgo = calendar.clone() as java.util.Calendar
        oneMonthAgo.add(java.util.Calendar.MONTH, -1)
        val oneMonthAgoMillis = oneMonthAgo.timeInMillis

        calendar.set(java.util.Calendar.MONTH, 0)
        calendar.set(java.util.Calendar.DAY_OF_MONTH, 1)
        calendar.set(java.util.Calendar.HOUR_OF_DAY, 0)
        calendar.set(java.util.Calendar.MINUTE, 0)
        calendar.set(java.util.Calendar.SECOND, 0)
        calendar.set(java.util.Calendar.MILLISECOND, 0)
        val startOfYear = calendar.timeInMillis

        val lastWeek = files.filter { it.lastModified > oneWeekAgoMillis }
            .sortedByDescending { it.lastModified }
        val lastMonth = files.filter { it.lastModified in oneMonthAgoMillis..oneWeekAgoMillis }
            .sortedByDescending { it.lastModified }
        val earlierThisYear = files.filter { it.lastModified in startOfYear..oneMonthAgoMillis }
            .sortedByDescending { it.lastModified }
        val older = files.filter { it.lastModified < startOfYear }
            .sortedByDescending { it.lastModified }

        val filesGroupedByDate: MutableList<Any> = mutableListOf()
        if (lastWeek.isNotEmpty()) {
            filesGroupedByDate.add("This week")
            filesGroupedByDate.addAll(lastWeek)
        }
        if (lastMonth.isNotEmpty()) {
            filesGroupedByDate.add("This month")
            filesGroupedByDate.addAll(lastMonth)
        }
        if (earlierThisYear.isNotEmpty()) {
            filesGroupedByDate.add("This year")
            filesGroupedByDate.addAll(earlierThisYear)
        }
        if (older.isNotEmpty()) {
            filesGroupedByDate.add("Older")
            filesGroupedByDate.addAll(older)
        }

        if (currentParent != null) {
            val parent = File()
            parent.id = PARENT_ID
            parent.type = FileType.Folder
            parent.name = "..."

            val i = if (filesGroupedByDate.isEmpty()) 0 else 1

            filesGroupedByDate.add(i, parent)
        }

        this.files.set(filesGroupedByDate)
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

fun String.capitalized(): String {
    return this.replaceFirstChar {
        if (it.isLowerCase())
            it.titlecase(Locale.getDefault())
        else it.toString()
    }
}
