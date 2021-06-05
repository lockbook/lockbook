package app.lockbook

import app.lockbook.util.*
import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import java.util.*
import kotlin.reflect.KClass

// You have to build the jni from core first to be able to run the tests.
// Next you have to add a vm option that helps java find the library:
// -ea -Djava.library.path="<path to lockbook>/lockbook/clients/android/core/src/main/jniLibs/desktop/"

fun generateAlphaString(): String =
    (1..20).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

fun generateId(): String = UUID.randomUUID().toString()

fun createRandomPath(): String {
    val path = "/tmp/${generateAlphaString()}"
    Runtime.getRuntime().exec("mkdir $path")
    return path
}

inline fun <reified T> assertType(comparableValue: Any?) {
    require(comparableValue is T) {
        "${Thread.currentThread().stackTrace[1]}: ${if (comparableValue == null) "null" else comparableValue::class.qualifiedName} is not of type ${T::class.qualifiedName}"
    }
}

inline fun <reified T> assertTypeReturn(comparableValue: Any?): T {
    require(comparableValue is T) {
        "${Thread.currentThread().stackTrace[1]}: ${if (comparableValue == null) "null" else comparableValue::class.qualifiedName} is not of type ${T::class.qualifiedName}"
    }

    return comparableValue
}

const val unrecognizedErrorTemplate = " is an unrecognized error type from "
const val obsoleteErrorTemplate = "There is an obsolete error type from "
const val stubError = "Stub"

val errorsToCheck = listOf<KClass<*>>(
    CalculateWorkError::class,
    CreateAccountError::class,
    CreateFileError::class,
    FileDeleteError::class,
    AccountExportError::class,
    GetAccountError::class,
    GetChildrenError::class,
    GetFileByIdError::class,
    GetRootError::class,
    GetStateError::class,
    GetUsageError::class,
    ImportError::class,
    InsertFileError::class,
    MigrationError::class,
    MoveFileError::class,
    ReadDocumentError::class,
    RenameFileError::class,
    SetLastSyncedError::class,
    SyncAllError::class,
    WriteToDocumentError::class,
    ExportDrawingError::class
)

val checkIfAllErrorsPresentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val jsonObject = jv.obj!!

        for (error in errorsToCheck) {
            var variantsToCheck = error.nestedClasses.filter { kClass -> kClass.simpleName != "Unexpected" }

            jsonObject.array<String>(error.simpleName!!)!!.forEach { variant ->
                val sizeBefore = variantsToCheck.size
                variantsToCheck = variantsToCheck.filter { kClass -> variant != kClass.simpleName }
                if (variantsToCheck.size == sizeBefore && variant != stubError) {
                    throw Throwable(variant + unrecognizedErrorTemplate + error.simpleName)
                }
            }

            if (variantsToCheck.isNotEmpty()) {
                throw Throwable(obsoleteErrorTemplate + error.simpleName)
            }
        }

        return Unit
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
