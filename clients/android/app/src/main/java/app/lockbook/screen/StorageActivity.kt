package app.lockbook.screen

import android.content.Intent
import android.graphics.Typeface
import android.os.Bundle
import android.text.SpannableStringBuilder
import android.text.style.ForegroundColorSpan
import android.text.style.RelativeSizeSpan
import android.text.style.StyleSpan
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.PopupMenu
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.text.inSpans
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.ConcatAdapter
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.ActivityStorageBinding
import app.lockbook.databinding.ItemStorageEmptyBinding
import app.lockbook.databinding.ItemStorageRowBinding
import app.lockbook.databinding.StorageHeaderBinding
import app.lockbook.model.StorageChange
import app.lockbook.model.StorageKind
import app.lockbook.model.StorageReport
import app.lockbook.model.StorageRow
import app.lockbook.model.StorageSize
import app.lockbook.ui.StorageInks
import app.lockbook.ui.UsageBarPreference
import app.lockbook.util.ExtensionHelper
import com.google.android.material.color.MaterialColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.Usage
import kotlin.math.abs

class StorageActivity : AppCompatActivity() {
    private lateinit var binding: ActivityStorageBinding
    private lateinit var header: StorageHeaderBinding
    private val rows = RowsAdapter()
    private val inks by lazy { StorageInks(this) }

    private var usage: Usage? = null
    private var report: StorageReport? = null
    private var outOfSpace = false
    private var tab = R.id.storage_tab_folders
    private var folder: String? = null

    private val upFolder =
        object : OnBackPressedCallback(false) {
            override fun handleOnBackPressed() {
                goUp()
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityStorageBinding.inflate(layoutInflater)
        setContentView(binding.root)
        binding.storageToolbar.setNavigationOnClickListener { finish() }
        binding.storageList.layoutManager = LinearLayoutManager(this)

        header = StorageHeaderBinding.inflate(layoutInflater, binding.storageList, false)
        header.storageTabs.addOnButtonCheckedListener { _, id, checked ->
            if (checked) {
                tab = id
                renderRows()
            }
        }
        header.storageUpgrade.setOnClickListener {
            startActivity(Intent(this, UpgradeAccountActivity::class.java))
        }

        binding.storageList.adapter = ConcatAdapter(HeaderAdapter(header.root), rows)
        onBackPressedDispatcher.addCallback(this, upFolder)
        renderRows()
    }

    override fun onResume() {
        super.onResume()
        load()
    }

    private fun load() {
        lifecycleScope.launch(Dispatchers.IO) {
            val usage = runCatching { Lb.refreshUsage() }.recoverCatching { Lb.getUsage() }.getOrNull()
            val entries = runCatching { Lb.getUsageBreakdown().toList() }.getOrDefault(emptyList())
            val files = runCatching { Lb.listMetadatas().toList() }.getOrDefault(emptyList())
            val report = files.firstOrNull { it.isRoot }?.let { StorageReport(it.id, files, entries) }
            val outOfSpace = runCatching { Lb.getStatus().outOfSpace }.getOrDefault(false)

            withContext(Dispatchers.Main) {
                this@StorageActivity.usage = usage ?: this@StorageActivity.usage
                this@StorageActivity.report = report ?: this@StorageActivity.report
                this@StorageActivity.outOfSpace = outOfSpace
                renderSummary()
                renderRows()
            }
        }
    }

    private fun renderSummary() {
        val usage = usage ?: return
        val cap = usage.dataCap
        val over = cap != null && usage.local.exact > cap.exact

        header.storageSummary.visibility = View.VISIBLE
        header.storageUsed.text =
            SpannableStringBuilder()
                .inSpans(StyleSpan(Typeface.BOLD)) { append(usage.local.readable) }
                .append(" ")
                .inSpans(RelativeSizeSpan(0.6f), ForegroundColorSpan(secondaryText())) {
                    append(cap?.let { getString(R.string.storage_of_cap, it.readable) } ?: getString(R.string.storage_used))
                }
        header.storageSide.text =
            when {
                cap == null -> getString(R.string.storage_cap_not_loaded)
                over -> getString(R.string.storage_over_by, bytes(usage.local.exact - cap.exact))
                else -> getString(R.string.storage_free, bytes(cap.exact - usage.local.exact))
            }
        header.storageSide.setTextColor(if (over) inks.over else secondaryText())
        header.storageBar.usage = usage
        header.storageLegend.text = legend(usage, over)

        val message = message(usage, over)
        header.storageMessage.visibility = if (message == null) View.GONE else View.VISIBLE
        if (message != null) {
            header.storageMessageTitle.text = message.first
            header.storageMessageDetail.text = message.second
            header.storageUpgrade.visibility = if (isPremium(usage)) View.GONE else View.VISIBLE
        }
    }

    private fun legend(
        usage: Usage,
        over: Boolean,
    ): CharSequence {
        val text = SpannableStringBuilder()

        fun item(
            color: Int,
            label: Int,
            value: String,
        ) {
            if (text.isNotEmpty()) {
                text.append("    ")
            }
            text.inSpans(ForegroundColorSpan(color)) { append("● ") }
            text.inSpans(ForegroundColorSpan(secondaryText())) { append(getString(label).replace(' ', ' ')) }
            text.append(" ").append(value.replace(' ', ' '))
        }

        item(inks.synced, R.string.storage_synced, usage.synced.readable)
        if (usage.added.exact > 0) {
            item(inks.added, R.string.storage_unsynced, "+" + usage.added.readable)
        }
        if (usage.freed.exact > 0) {
            item(inks.freed, R.string.storage_freed_on_sync, "−" + usage.freed.readable)
        }
        val cap = usage.dataCap
        if (over && cap != null) {
            item(inks.over, R.string.storage_over_cap, bytes(usage.local.exact - cap.exact))
        }
        return text
    }

    private fun message(
        usage: Usage,
        over: Boolean,
    ): Pair<String, String>? {
        val cap = usage.dataCap ?: return null
        return when {
            over -> {
                val free = bytes(usage.local.exact - cap.exact)
                val detail =
                    if (isPremium(usage)) {
                        getString(R.string.storage_over_detail, free)
                    } else {
                        getString(R.string.storage_over_detail_free_tier, free)
                    }
                getString(R.string.storage_over_title) to detail
            }

            outOfSpace -> {
                getString(R.string.storage_out_of_space_title) to getString(R.string.storage_out_of_space_detail)
            }

            usage.local.exact >= cap.exact * 0.8 -> {
                getString(R.string.storage_almost_full_title) to
                    getString(R.string.storage_almost_full_detail, bytes(cap.exact - usage.local.exact))
            }

            else -> {
                null
            }
        }
    }

    private fun renderRows() {
        val report =
            report ?: run {
                upFolder.isEnabled = false
                rows.submit(listOf(Item.Empty(getString(R.string.storage_loading))))
                return
            }
        val items =
            when (tab) {
                R.id.storage_tab_types -> types(report)
                R.id.storage_tab_largest -> largest(report)
                R.id.storage_tab_unsynced -> unsynced(report)
                else -> folders(report)
            }
        upFolder.isEnabled = tab == R.id.storage_tab_folders && focus(report) != report.root
        rows.submit(items)
    }

    private fun focus(report: StorageReport): String = folder?.takeIf { report.isFolder(it) } ?: report.root

    private fun goUp() {
        val report = report ?: return
        val trail = report.trail(focus(report))
        folder = if (trail.size > 1) trail[trail.size - 2].first else report.root
        renderRows()
    }

    private fun folders(report: StorageReport): List<Item> {
        val focus = focus(report)
        val items = mutableListOf<Item>()

        if (focus != report.root) {
            val size = report.row(focus)?.size ?: StorageSize()
            items +=
                Item.Row(
                    icon = R.drawable.ic_baseline_arrow_back_24,
                    title = report.trail(focus).drop(1).joinToString(" / ") { it.second },
                    detail = getString(R.string.storage_back),
                    value = bytes(size.local),
                    delta = delta(size),
                    onClick = { goUp() },
                )
        }

        val children = report.children(focus)
        if (children.isEmpty()) {
            items += Item.Empty(getString(R.string.storage_empty_folder))
        }
        val max = children.maxOfOrNull { it.size.local } ?: 1
        val hasFolders = children.any { it.isFolder }
        for (row in children) {
            items +=
                if (row.isFolder) {
                    fileRow(
                        row,
                        detail = plural(R.plurals.storage_items, row.count - 1),
                        max = max,
                        chevron = true,
                        onClick = {
                            folder = row.id
                            renderRows()
                        },
                        onLongClick = { view -> locateMenu(view, row.id) },
                    )
                } else {
                    fileRow(row, detail = null, max = max, chevron = if (hasFolders) false else null, onClick = { open(row.id) })
                }
        }
        return items
    }

    private fun types(report: StorageReport): List<Item> {
        if (report.kinds.isEmpty()) {
            return listOf(Item.Empty(getString(R.string.storage_empty_account)))
        }
        val max = report.kinds.maxOf { it.size.local }.coerceAtLeast(1)
        return report.kinds.map { row ->
            Item.Row(
                icon = kindIcon(row.kind),
                title = getString(kindLabel(row.kind)),
                detail =
                    plural(
                        if (row.kind == StorageKind.Folders) R.plurals.storage_items else R.plurals.storage_files,
                        row.count,
                    ),
                fraction = row.size.local.toDouble() / max,
                value = bytes(row.size.local),
                delta = delta(row.size),
            )
        }
    }

    private fun largest(report: StorageReport): List<Item> {
        if (report.largest.isEmpty()) {
            return listOf(Item.Empty(getString(R.string.storage_no_documents)))
        }
        val max =
            report.largest
                .first()
                .size.local
        return report.largest.map { row ->
            fileRow(
                row,
                detail = getString(R.string.storage_in_path, report.parentPath(row.id)),
                max = max,
                chevron = null,
                onClick = { open(row.id) },
            )
        }
    }

    private fun unsynced(report: StorageReport): List<Item> {
        if (report.changes.isEmpty()) {
            return listOf(Item.Empty(getString(R.string.storage_all_synced)))
        }
        val max = report.changes.maxOf { abs(it.size.delta) }.coerceAtLeast(1)
        return report.changes.map { row ->
            val id = row.fileId.takeIf { row.change != StorageChange.Deleted }
            Item.Row(
                icon = if (id != null) fileIcon(row.name, row.isFolder) else R.drawable.ic_outline_delete_24,
                title = if (id != null) row.name else plural(R.plurals.storage_deleted_files, row.count),
                detail =
                    if (id != null) {
                        getString(R.string.storage_change_in_path, getString(changeLabel(row.change)), report.parentPath(id))
                    } else {
                        getString(R.string.storage_deletes_pending)
                    },
                fraction = abs(row.size.delta).toDouble() / max,
                barColor = if (row.size.delta > 0) inks.added else inks.freed,
                value = signed(row.size.delta),
                onClick =
                    when {
                        id == null -> null
                        row.isFolder -> ({ locate(id) })
                        else -> ({ open(id) })
                    },
            )
        }
    }

    private fun fileRow(
        row: StorageRow,
        detail: String?,
        max: Long,
        chevron: Boolean?,
        onClick: () -> Unit,
        onLongClick: ((View) -> Unit)? = null,
    ) = Item.Row(
        icon = fileIcon(row.name, row.isFolder),
        title = row.name,
        detail = detail,
        fraction = row.size.local.toDouble() / max.coerceAtLeast(1),
        value = bytes(row.size.local),
        delta = delta(row.size),
        chevron = chevron,
        onClick = onClick,
        onLongClick = onLongClick,
    )

    private fun locateMenu(
        anchor: View,
        id: String,
    ) {
        PopupMenu(this, anchor).apply {
            menu.add(R.string.storage_locate_in_files).setOnMenuItemClickListener {
                locate(id)
                true
            }
            show()
        }
    }

    private fun open(id: String) = showInMain(MainScreenActivity.STORAGE_OPEN_FILE, id)

    private fun locate(id: String) = showInMain(MainScreenActivity.STORAGE_LOCATE_FOLDER, id)

    private fun showInMain(
        key: String,
        id: String,
    ) {
        startActivity(
            Intent(this, MainScreenActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)
                .putExtra(key, id),
        )
    }

    private fun delta(size: StorageSize): Pair<String, Int>? =
        when {
            size.delta == 0L -> null
            size.delta > 0 -> signed(size.delta) to inks.added
            else -> signed(size.delta) to secondaryText()
        }

    private fun bytes(bytes: Long): String = Lb.bytesToHuman(bytes)

    private fun signed(delta: Long): String = (if (delta >= 0) "+" else "−") + bytes(abs(delta))

    private fun plural(
        id: Int,
        count: Int,
    ): String = resources.getQuantityString(id, count, count)

    private fun secondaryText(): Int = MaterialColors.getColor(this, com.google.android.material.R.attr.colorOnSurfaceVariant, 0)

    private fun isPremium(usage: Usage): Boolean = usage.dataCap?.exact == UsageBarPreference.PAID_TIER_USAGE_BYTES

    private fun fileIcon(
        name: String,
        isFolder: Boolean,
    ): Int {
        if (isFolder) {
            return R.drawable.ic_baseline_folder_24
        }
        val ext = ExtensionHelper(name)
        return when {
            ext.isDrawing -> R.drawable.ic_outline_draw_24
            ext.isImage -> R.drawable.ic_outline_image_24
            ext.isPdf -> R.drawable.ic_outline_picture_as_pdf_24
            else -> R.drawable.ic_outline_insert_drive_file_24
        }
    }

    private fun kindIcon(kind: StorageKind): Int =
        when (kind) {
            StorageKind.Notes -> R.drawable.ic_baseline_text_format_24
            StorageKind.Drawings -> R.drawable.ic_outline_draw_24
            StorageKind.Images -> R.drawable.ic_outline_image_24
            StorageKind.Pdfs -> R.drawable.ic_outline_picture_as_pdf_24
            StorageKind.Text -> R.drawable.ic_baseline_code_24
            StorageKind.Other -> R.drawable.ic_outline_insert_drive_file_24
            StorageKind.Folders -> R.drawable.ic_baseline_folder_24
        }

    private fun kindLabel(kind: StorageKind): Int =
        when (kind) {
            StorageKind.Notes -> R.string.storage_kind_notes
            StorageKind.Drawings -> R.string.storage_kind_drawings
            StorageKind.Images -> R.string.storage_kind_images
            StorageKind.Pdfs -> R.string.storage_kind_pdfs
            StorageKind.Text -> R.string.storage_kind_text
            StorageKind.Other -> R.string.storage_kind_other
            StorageKind.Folders -> R.string.storage_kind_folders
        }

    private fun changeLabel(change: StorageChange): Int =
        when (change) {
            StorageChange.New -> R.string.storage_change_new
            StorageChange.Grew, StorageChange.Shrank -> R.string.storage_change_edited
            StorageChange.MovedOut -> R.string.storage_change_moved_out
            StorageChange.Deleted -> R.string.storage_change_deleted
        }

    private sealed interface Item {
        data class Row(
            val icon: Int,
            val title: String,
            val detail: String?,
            val fraction: Double? = null,
            val barColor: Int? = null,
            val value: String,
            val delta: Pair<String, Int>? = null,
            val chevron: Boolean? = null,
            val onClick: (() -> Unit)? = null,
            val onLongClick: ((View) -> Unit)? = null,
        ) : Item

        data class Empty(
            val message: String,
        ) : Item
    }

    private class HeaderAdapter(
        private val view: View,
    ) : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
        override fun getItemCount() = 1

        override fun onCreateViewHolder(
            parent: ViewGroup,
            viewType: Int,
        ) = object : RecyclerView.ViewHolder(view) {}

        override fun onBindViewHolder(
            holder: RecyclerView.ViewHolder,
            position: Int,
        ) {}
    }

    private inner class RowsAdapter : RecyclerView.Adapter<RecyclerView.ViewHolder>() {
        private var items: List<Item> = emptyList()

        fun submit(items: List<Item>) {
            this.items = items
            notifyDataSetChanged()
        }

        override fun getItemCount() = items.size

        override fun getItemViewType(position: Int) =
            when (items[position]) {
                is Item.Row -> R.layout.item_storage_row
                is Item.Empty -> R.layout.item_storage_empty
            }

        override fun onCreateViewHolder(
            parent: ViewGroup,
            viewType: Int,
        ): RecyclerView.ViewHolder {
            val inflater = LayoutInflater.from(parent.context)
            return if (viewType == R.layout.item_storage_row) {
                RowHolder(ItemStorageRowBinding.inflate(inflater, parent, false))
            } else {
                EmptyHolder(ItemStorageEmptyBinding.inflate(inflater, parent, false))
            }
        }

        override fun onBindViewHolder(
            holder: RecyclerView.ViewHolder,
            position: Int,
        ) {
            when (val item = items[position]) {
                is Item.Row -> (holder as RowHolder).bind(item)
                is Item.Empty -> (holder as EmptyHolder).binding.storageEmpty.text = item.message
            }
        }
    }

    private class EmptyHolder(
        val binding: ItemStorageEmptyBinding,
    ) : RecyclerView.ViewHolder(binding.root)

    private inner class RowHolder(
        private val binding: ItemStorageRowBinding,
    ) : RecyclerView.ViewHolder(binding.root) {
        fun bind(item: Item.Row) {
            binding.storageRowIcon.setImageResource(item.icon)
            binding.storageRowTitle.text = item.title
            binding.storageRowDetail.text = item.detail
            binding.storageRowDetail.visibility = if (item.detail == null) View.GONE else View.VISIBLE
            binding.storageRowBar.visibility = if (item.fraction == null) View.GONE else View.VISIBLE
            item.fraction?.let { binding.storageRowBar.set(it, item.barColor ?: inks.synced) }
            binding.storageRowValue.text = item.value
            binding.storageRowDelta.visibility = if (item.delta == null) View.GONE else View.VISIBLE
            item.delta?.let { (text, color) ->
                binding.storageRowDelta.text = text
                binding.storageRowDelta.setTextColor(color)
            }
            binding.storageRowChevron.visibility =
                when (item.chevron) {
                    null -> View.GONE
                    true -> View.VISIBLE
                    false -> View.INVISIBLE
                }

            val onClick = item.onClick
            if (onClick != null) {
                binding.root.setOnClickListener { onClick() }
            } else {
                binding.root.setOnClickListener(null)
            }
            binding.root.isClickable = onClick != null

            val onLongClick = item.onLongClick
            if (onLongClick != null) {
                binding.root.setOnLongClickListener { view ->
                    onLongClick(view)
                    true
                }
            } else {
                binding.root.setOnLongClickListener(null)
            }
            binding.root.isLongClickable = onLongClick != null
        }
    }
}
