package app.lockbook

import app.lockbook.core.getAllErrorVariants
import app.lockbook.util.*
import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
import timber.log.Timber
import java.util.*

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

fun assertEnumType(comparableValue: Any?, enum: Any) {
    require(comparableValue == enum) {
        "${Thread.currentThread().stackTrace[1]}: ${if (comparableValue == null) "null" else (comparableValue as Enum<*>).name} is not of type ${(enum as Enum<*>).name}"
    }
}

inline fun <reified T> assertTypeReturn(comparableValue: Any?): T {
    require(comparableValue is T) {
        "${Thread.currentThread().stackTrace[1]}: ${if (comparableValue == null) "null" else comparableValue::class.qualifiedName} is not of type ${T::class.qualifiedName}"
    }

    return comparableValue
}

val getAllErrorVariantsConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        println(jv.obj?.toJsonString(prettyPrint = true))
        val jsonObject = jv.obj!!

        val accountExportJsonArray = jsonObject.array<AccountExportError>("AccountExportError")!!
        val accountExportErrors = Klaxon().parseFromJsonArray<AccountExportError>(accountExportJsonArray)!!

        val calculateWorkErrorJsonArray = jsonObject.array<CalculateWorkError>("CalculateWorkError")!!
        val calculateWorkErrorErrors = Klaxon().parseFromJsonArray<CalculateWorkError>(accountExportJsonArray)!!

        val createAccountJsonArray = jsonObject.array<CreateAccountError>("CreateAccountError")!!
        val createAccountErrors = Klaxon().parseFromJsonArray<CreateAccountError>(accountExportJsonArray)!!

        val createFileJsonArray = jsonObject.array<CreateFileError>("CreateFileError")!!
        val createFileErrors = Klaxon().parseFromJsonArray<CreateFileError>(accountExportJsonArray)!!

        val deleteFileJsonArray = jsonObject.array<DeleteFileError>("DeleteFileError")!!
        val deleteFileErrors = Klaxon().parseFromJsonArray<DeleteFileError>(accountExportJsonArray)!!

        val executeWorkJsonArray = jsonObject.array<ExecuteWorkError>("ExecuteWorkError")!!
        val executeWorkErrors = Klaxon().parseFromJsonArray<ExecuteWorkError>(accountExportJsonArray)!!

        val getAccountJsonArray = jsonObject.array<GetAccountError>("GetAccountError")!!
        val getAccountErrors = Klaxon().parseFromJsonArray<GetAccountError>(accountExportJsonArray)!!

        val getChildrenJsonArray = jsonObject.array<GetChildrenError>("GetChildrenError")!!
        val getChildrenErrors = Klaxon().parseFromJsonArray<GetChildrenError>(accountExportJsonArray)!!

        val getFileByIdJsonArray = jsonObject.array<GetFileByIdError>("GetFileByIdError")!!
        val getFileByIdErrors = Klaxon().parseFromJsonArray<GetFileByIdError>(accountExportJsonArray)!!

        val getRootJsonArray = jsonObject.array<GetRootError>("GetRootError")!!
        val getRootErrors = Klaxon().parseFromJsonArray<GetRootError>(accountExportJsonArray)!!

        val getStateJsonArray = jsonObject.array<GetStateError>("GetStateError")!!
        val getStateErrors = Klaxon().parseFromJsonArray<GetStateError>(accountExportJsonArray)!!

        val getUsageJsonArray = jsonObject.array<GetUsageError>("GetUsageError")!!
        val getUsageErrors = Klaxon().parseFromJsonArray<GetUsageError>(accountExportJsonArray)!!

        val importJsonArray = jsonObject.array<ImportError>("ImportError")!!
        val importErrors = Klaxon().parseFromJsonArray<ImportError>(accountExportJsonArray)!!

        val insertFileJsonArray = jsonObject.array<InsertFileError>("InsertFileError")!!
        val insertFileErrors = Klaxon().parseFromJsonArray<InsertFileError>(accountExportJsonArray)!!

        val migrationJsonArray = jsonObject.array<MigrationError>("MigrationError")!!
        val migrationErrors = Klaxon().parseFromJsonArray<MigrationError>(accountExportJsonArray)!!

        val moveFileJsonArray = jsonObject.array<MoveFileError>("MoveFileError")!!
        val moveFileErrors = Klaxon().parseFromJsonArray<MoveFileError>(accountExportJsonArray)!!

        val readDocumentJsonArray = jsonObject.array<ReadDocumentError>("ReadDocumentError")!!
        val readDocumentErrors = Klaxon().parseFromJsonArray<ReadDocumentError>(accountExportJsonArray)!!

        val renameFileJsonArray = jsonObject.array<RenameFileError>("RenameFileError")!!
        val renameFileErrors = Klaxon().parseFromJsonArray<RenameFileError>(accountExportJsonArray)!!

        val setLastSyncedJsonArray = jsonObject.array<SetLastSyncedError>("SetLastSyncedError")!!
        val setLastSyncedErrors = Klaxon().parseFromJsonArray<SetLastSyncedError>(accountExportJsonArray)!!
        print("${setLastSyncedErrors[1]}")

        val syncAllJsonArray = jsonObject.array<SyncAllError>("SyncAllError")!!
        val syncAllErrors = Klaxon().parseFromJsonArray<SyncAllError>(accountExportJsonArray)!!

        val writeToDocumentJsonArray = jsonObject.array<WriteToDocumentError>("WriteToDocumentError")!!
        val writeToDocumentErrors = Klaxon().parseFromJsonArray<WriteToDocumentError>(accountExportJsonArray)!!

        return listOf(ExecuteWorkError.ClientUpdateRequired)
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}

fun getErrorVariants(): List<CoreError> = Klaxon().converter(getAllErrorVariantsConverter).parse(
    getAllErrorVariants()
)!!
