package app.lockbook.model

import net.lockbook.File
import net.lockbook.Lb
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

internal object FileCreation {
    fun defaultBaseName(): String = SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date())

    fun suggestBaseName(
        parentId: String,
        type: NewFileType,
    ): String {
        val name = Lb.nextName(parentId, type.completeName(defaultBaseName()))
        return type.extension?.let(name::removeSuffix) ?: name
    }

    fun create(
        parentId: String,
        type: NewFileType,
        baseName: String,
    ): File = Lb.createFile(type.completeName(baseName), parentId, type.isDocument)
}
