package app.lockbook.model

import net.lockbook.File
import net.lockbook.UsageEntry
import kotlin.math.abs

data class StorageSize(
    val synced: Long = 0,
    val local: Long = 0,
) {
    val delta: Long get() = local - synced

    operator fun plus(other: StorageSize) = StorageSize(synced + other.synced, local + other.local)
}

enum class StorageKind {
    Notes,
    Drawings,
    Images,
    Pdfs,
    Text,
    Other,
    Folders,
    ;

    companion object {
        private val imageExtensions =
            setOf(
                "avif",
                "bmp",
                "dds",
                "exr",
                "ff",
                "gif",
                "hdr",
                "ico",
                "jpeg",
                "jpg",
                "png",
                "pnm",
                "qoi",
                "tga",
                "tiff",
                "webp",
                "cr2",
                "heic",
                "heif",
            )

        private val textExtensions =
            setOf(
                "txt",
                "c",
                "h",
                "cpp",
                "hpp",
                "cs",
                "css",
                "go",
                "html",
                "java",
                "js",
                "json",
                "jsx",
                "kt",
                "lua",
                "py",
                "rb",
                "rs",
                "sh",
                "sql",
                "swift",
                "toml",
                "ts",
                "tsx",
                "xml",
                "yaml",
                "yml",
                "zig",
                "csv",
                "log",
            )

        fun of(file: File): StorageKind {
            if (file.type != File.FileType.Document) {
                return Folders
            }
            val ext = file.name.substringAfterLast('.', "").lowercase()
            return when {
                ext == "md" -> Notes
                ext == "svg" || ext == "draw" -> Drawings
                ext == "pdf" -> Pdfs
                ext in imageExtensions -> Images
                ext in textExtensions -> Text
                else -> Other
            }
        }
    }
}

data class StorageKindRow(
    val kind: StorageKind,
    val count: Int,
    val size: StorageSize,
)

data class StorageRow(
    val id: String,
    val name: String,
    val isFolder: Boolean,
    val count: Int,
    val size: StorageSize,
)

enum class StorageChange {
    New,
    Grew,
    Shrank,
    MovedOut,
    Deleted,
}

data class StorageChangeRow(
    val fileId: String?,
    val name: String,
    val isFolder: Boolean,
    val change: StorageChange,
    val count: Int,
    val size: StorageSize,
)

class StorageReport(
    val root: String,
    files: List<File>,
    entries: List<UsageEntry>,
) {
    private class Node(
        val name: String,
        val parent: String,
        val isFolder: Boolean,
    )

    private val nodes = HashMap<String, Node>()
    private val childIds = HashMap<String, MutableList<String>>()
    private val sizes = HashMap<String, StorageSize>()
    private val totals = HashMap<String, Pair<StorageSize, Int>>()

    val kinds: List<StorageKindRow>
    val largest: List<StorageRow>
    val changes: List<StorageChangeRow>

    init {
        val types = HashMap<String, StorageKind>()
        for (file in files) {
            nodes[file.id] = Node(file.name, file.parent, file.type == File.FileType.Folder)
            types[file.id] = StorageKind.of(file)
            if (file.id != root) {
                childIds.getOrPut(file.parent) { mutableListOf() }.add(file.id)
            }
        }

        val kindRows = HashMap<StorageKind, StorageKindRow>()
        var deleted = StorageSize()
        var deletedCount = 0
        val changeRows = mutableListOf<StorageChangeRow>()

        for (entry in entries) {
            val size = StorageSize(entry.synced, entry.local)
            sizes[entry.id] = size
            val node = nodes[entry.id]

            if (node != null || entry.local > 0) {
                val kind = types[entry.id] ?: StorageKind.Folders
                val row = kindRows[kind] ?: StorageKindRow(kind, 0, StorageSize())
                kindRows[kind] = row.copy(count = row.count + 1, size = row.size + size)
                rollUp(entry.id, size, visible = node != null)
            } else {
                deletedCount += 1
                deleted += size
                rollUp(entry.id, size, visible = false)
            }

            if (node != null && entry.synced != entry.local) {
                val change =
                    when {
                        entry.synced == 0L || (!node.isFolder && entry.synced <= METADATA_FEE) -> StorageChange.New
                        entry.local == 0L -> StorageChange.MovedOut
                        entry.local > entry.synced -> StorageChange.Grew
                        else -> StorageChange.Shrank
                    }
                changeRows.add(StorageChangeRow(entry.id, node.name, node.isFolder, change, 1, size))
            }
        }

        if (deletedCount > 0) {
            changeRows.add(StorageChangeRow(null, "", false, StorageChange.Deleted, deletedCount, deleted))
        }
        changes = changeRows.sortedByDescending { abs(it.size.delta) }

        kinds = kindRows.values.filter { it.size.local > 0 }.sortedByDescending { it.size.local }

        largest =
            nodes
                .filterValues { !it.isFolder }
                .keys
                .mapNotNull { row(it) }
                .filter { it.size.local > 0 }
                .sortedByDescending { it.size.local }
                .take(LARGEST_COUNT)
    }

    private fun rollUp(
        id: String,
        size: StorageSize,
        visible: Boolean,
    ) {
        var at = if (visible) id else root
        var seen = 0
        while (true) {
            val (total, count) = totals[at] ?: (StorageSize() to 0)
            totals[at] = (total + size) to (count + 1)
            if (at == root || seen > nodes.size) {
                break
            }
            seen += 1
            val parent = nodes[at]?.parent
            at = if (parent != null && nodes.containsKey(parent)) parent else root
        }
    }

    fun row(id: String): StorageRow? {
        val node = nodes[id] ?: return null
        if (node.isFolder) {
            val (size, count) = totals[id] ?: return null
            return StorageRow(id, node.name, true, count, size)
        }
        val size = sizes[id] ?: return null
        return StorageRow(id, node.name, false, 1, size)
    }

    fun children(folder: String): List<StorageRow> =
        childIds[folder]
            .orEmpty()
            .mapNotNull { row(it) }
            .filter { it.size.local > 0 || it.size.synced > 0 }
            .sortedWith(compareByDescending<StorageRow> { it.size.local }.thenBy { it.name })

    fun trail(folder: String): List<Pair<String, String>> {
        val trail = mutableListOf<Pair<String, String>>()
        var at = folder
        while (true) {
            val node = nodes[at] ?: break
            trail.add(at to node.name)
            if (at == root || trail.size > nodes.size) {
                break
            }
            at = node.parent
        }
        return trail.reversed()
    }

    fun parentPath(id: String): String {
        val parent = nodes[id]?.parent ?: return ""
        val trail = trail(parent)
        if (trail.size <= 1) {
            return "/"
        }
        return trail.drop(1).joinToString("/") { it.second }
    }

    fun isFolder(id: String): Boolean = nodes[id]?.isFolder ?: false

    companion object {
        const val METADATA_FEE: Long = 1000
        const val LARGEST_COUNT = 12
    }
}
