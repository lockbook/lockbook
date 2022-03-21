package app.lockbook

import app.lockbook.util.*
import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrapError
import java.util.*
import kotlin.reflect.KClass

// You have to build the jni from core first to be able to run the tests.
// Next you have to add a vm option that helps java find the library:
// -ea -Djava.library.path="<path to lockbook>/lockbook/clients/android/core/src/main/jniLibs/desktop/"

fun generateAlphaString(): String =
    (1..20).map { (('A'..'Z') + ('a'..'z')).random() }.joinToString("")

fun generateId(): String = UUID.randomUUID().toString()

fun generateFakeRandomPath() = "/tmp/${System.currentTimeMillis()}"

fun createRandomPath(): String {
    val path = "/tmp/${generateAlphaString()}"
    Runtime.getRuntime().exec("mkdir $path")
    return path
}

fun Result<*, CoreError<UiCoreError>>?.unwrapErrorType(enumType: UiCoreError): UiCoreError {
    val error = this?.unwrapError()
    require(error is CoreError.UiError && error.content == enumType) {
        "${Thread.currentThread().stackTrace[1]}: ${if (error == null) "null" else error::class.qualifiedName} is not of type ${enumType::class.qualifiedName}"
    }

    return error.content
}

const val unrecognizedErrorTemplate = " is an unrecognized error type from "
const val obsoleteErrorTemplate = "There is an obsolete error type from "

val errorsToCheck = listOf<KClass<*>>(
    CalculateWorkError::class,
    CreateAccountError::class,
    CreateFileError::class,
    FileDeleteError::class,
    AccountExportError::class,
    GetAccountError::class,
    GetFileByIdError::class,
    GetRootError::class,
    GetUsageError::class,
    ImportError::class,
    MigrationError::class,
    MoveFileError::class,
    ReadDocumentError::class,
    RenameFileError::class,
    SyncAllError::class,
    WriteToDocumentError::class,
)

val checkIfAllErrorsPresentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any {
        val jsonObject = jv.obj!!

        for (error in errorsToCheck) {
            var variantsToCheck = error.nestedClasses.filter { kClass -> kClass.simpleName != "Unexpected" }

            jsonObject.array<String>(error.simpleName!!)!!.forEach { variant ->
                val sizeBefore = variantsToCheck.size
                variantsToCheck = variantsToCheck.filter { kClass -> variant != kClass.simpleName }
                if (variantsToCheck.size == sizeBefore && variant != "Stub") {
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
