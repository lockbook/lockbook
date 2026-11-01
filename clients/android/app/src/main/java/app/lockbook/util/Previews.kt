package app.lockbook.util

import android.content.Context
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Handler
import android.os.Looper
import android.view.ContextThemeWrapper
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import app.lockbook.R
import app.lockbook.workspace.Workspace
import com.google.android.material.color.DynamicColors
import net.lockbook.Lb
import net.lockbook.LbError
import java.io.File
import java.util.concurrent.Executors

/**
 * Pinned documents drawn by the workspace for the landing screen as page thumbnails: light and
 * dark, filed by id beside the content version they show, drawn again only when that version
 * changes.
 */
object Previews {
    const val WIDTH_DP = 160f
    const val HEIGHT_DP = 120f

    private val worker = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private val shown = mutableMapOf<String, Bitmap>()

    private val _updated = MutableLiveData(0)

    /** Bumps whenever a preview is drawn or dropped. */
    val updated: LiveData<Int>
        get() = _updated

    /** The preview of `id` in the given appearance, once drawn for its current content. */
    fun get(
        id: String,
        dark: Boolean,
    ): Bitmap? = synchronized(shown) { shown[key(id, dark)] }

    /**
     * Draws the pins whose previews are missing or behind their documents and drops the previews
     * of documents no longer pinned. Call on the UI thread; it draws nothing while the workspace
     * isn't up.
     */
    fun refresh(
        context: Context,
        pins: List<String>,
    ) {
        // the page as it reads on this phone, drawn at the tile's pixels
        val page =
            context.resources.configuration.smallestScreenWidthDp
                .toFloat()
        val scale = context.resources.displayMetrics.density * WIDTH_DP / page
        // content stays inside the card's rounded corners
        val padding = context.resources.getDimension(R.dimen.pinned_file_corner_radius) / scale
        worker.execute {
            val dir = File(context.cacheDir, "previews").apply { mkdirs() }
            prune(dir, pins.toSet())
            val versions = pins.mapNotNull { id -> version(id)?.let { id to it } }.toMap()
            val stale = versions.filter { (id, version) -> stored(dir, id) != version }
            versions.keys.filter { it !in stale }.forEach { load(dir, it) }
            if (stale.isEmpty()) {
                main.post { _updated.value = (_updated.value ?: 0) + 1 }
                return@execute
            }
            main.post {
                val obj = WorkspaceView.wgpuObj
                if (obj == Long.MAX_VALUE) return@post
                val themes =
                    listOf(false, true).associateWith { dark ->
                        WorkspaceThemeHelper.materialTheme(context.inNightMode(dark), dark)
                    }
                val jobs =
                    stale.keys.flatMap { id ->
                        themes.map { (dark, theme) ->
                            Triple(
                                id,
                                dark,
                                Workspace.previewDocument(obj, id, "", page, page * HEIGHT_DP / WIDTH_DP, scale, padding, theme),
                            )
                        }
                    }
                worker.execute { render(dir, jobs, stale) }
            }
        }
    }

    private fun render(
        dir: File,
        jobs: List<Triple<String, Boolean, Long>>,
        versions: Map<String, String>,
    ) {
        val drawn = mutableMapOf<String, Int>()
        for ((id, dark, job) in jobs) {
            if (job == 0L) continue
            val png = Workspace.renderPreview(job) ?: continue
            image(dir, id, dark).writeBytes(png)
            drawn[id] = (drawn[id] ?: 0) + 1
        }
        // a version stands for both appearances
        for (id in drawn.filterValues { it == 2 }.keys) {
            versions[id]?.let { File(dir, "$id.version").writeText(it) }
            load(dir, id)
        }
        main.post { _updated.value = (_updated.value ?: 0) + 1 }
    }

    private fun load(
        dir: File,
        id: String,
    ) {
        for (dark in listOf(false, true)) {
            val bitmap = BitmapFactory.decodeFile(image(dir, id, dark).path) ?: continue
            synchronized(shown) { shown[key(id, dark)] = bitmap }
        }
    }

    private fun prune(
        dir: File,
        pins: Set<String>,
    ) {
        for (file in dir.listFiles().orEmpty()) {
            val id = file.name.take(36)
            if (id !in pins) {
                file.delete()
                synchronized(shown) { shown.keys.removeAll { it.startsWith(id) } }
            }
        }
    }

    private fun version(id: String): String? =
        try {
            Lb.documentVersion(id)
        } catch (err: LbError) {
            null
        }

    private fun stored(
        dir: File,
        id: String,
    ): String? = File(dir, "$id.version").takeIf { it.exists() }?.readText()

    private fun image(
        dir: File,
        id: String,
        dark: Boolean,
    ) = File(dir, "$id-${if (dark) "dark" else "light"}.png")

    /** `this` with its resources and theme resolved in light or dark. */
    private fun Context.inNightMode(dark: Boolean): Context {
        val config = Configuration(resources.configuration)
        val night = if (dark) Configuration.UI_MODE_NIGHT_YES else Configuration.UI_MODE_NIGHT_NO
        config.uiMode = (config.uiMode and Configuration.UI_MODE_NIGHT_MASK.inv()) or night
        return DynamicColors.wrapContextIfAvailable(ContextThemeWrapper(createConfigurationContext(config), R.style.AppTheme))
    }

    private fun key(
        id: String,
        dark: Boolean,
    ) = "$id-$dark"
}
