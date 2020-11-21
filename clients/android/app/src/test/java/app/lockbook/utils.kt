package app.lockbook

import app.lockbook.util.*
import com.beust.klaxon.Converter
import com.beust.klaxon.JsonValue
import com.beust.klaxon.Klaxon
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

const val unrecognizedErrorTemplate = " is an unrecognized error type from "
const val obsoleteErrorTemplate = "There is an obsolete error type from "
const val stubError = "Stub"

val checkIfAllErrorsPresentConverter = object : Converter {
    override fun canConvert(cls: Class<*>): Boolean = true

    override fun fromJson(jv: JsonValue): Any? {
        val jsonObject = jv.obj!!

        print(jsonObject.toJsonString(prettyPrint = true))

        var accountExportErrors = AccountExportError::class.nestedClasses.filter { kClass -> kClass != AccountExportError.Unexpected::class }
        jsonObject.array<String>("AccountExportError")!!.forEach { error ->
            val sizeBefore = accountExportErrors.size
            accountExportErrors = accountExportErrors.filter { kClass -> error != kClass.simpleName }
            if (accountExportErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + AccountExportError::class.simpleName)
            }
        }

        if (accountExportErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + AccountExportError::class.simpleName)
        }

        var createAccountErrors = CreateAccountError::class.nestedClasses.filter { kClass -> kClass != CreateAccountError.Unexpected::class }
        jsonObject.array<String>("CreateAccountError")!!.forEach { error ->
            val sizeBefore = createAccountErrors.size
            createAccountErrors = createAccountErrors.filter { kClass -> error != kClass.simpleName }
            if (createAccountErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + CreateAccountError::class.simpleName)
            }
        }

        if (createAccountErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + CreateAccountError::class.simpleName)
        }

        var calculateWorkErrors = CalculateWorkError::class.nestedClasses.filter { kClass -> kClass != CalculateWorkError.Unexpected::class }
        jsonObject.array<String>("CalculateWorkError")!!.forEach { error ->
            val sizeBefore = calculateWorkErrors.size
            calculateWorkErrors = calculateWorkErrors.filter { kClass -> error != kClass.simpleName }
            if (calculateWorkErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + CalculateWorkError::class.simpleName)
            }
        }

        if (calculateWorkErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + CalculateWorkError::class.simpleName)
        }

        var createFileErrors = CreateFileError::class.nestedClasses.filter { kClass -> kClass != CreateFileError.Unexpected::class }
        jsonObject.array<String>("CreateFileError")!!.forEach { error ->
            val sizeBefore = createFileErrors.size
            createFileErrors = createFileErrors.filter { kClass -> error != kClass.simpleName }
            if (createFileErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + CreateFileError::class.simpleName)
            }
        }

        if (createFileErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + CreateFileError::class.simpleName)
        }

        var fileDeleteErrors = FileDeleteError::class.nestedClasses.filter { kClass -> kClass != FileDeleteError.Unexpected::class }
        jsonObject.array<String>("FileDeleteError")!!.forEach { error ->
            val sizeBefore = fileDeleteErrors.size
            fileDeleteErrors = fileDeleteErrors.filter { kClass -> error != kClass.simpleName }
            if (fileDeleteErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + FileDeleteError::class.simpleName)
            }
        }

        if (fileDeleteErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + FileDeleteError::class.simpleName)
        }

        var executeWorkErrors = ExecuteWorkError::class.nestedClasses.filter { kClass -> kClass != ExecuteWorkError.Unexpected::class }
        jsonObject.array<String>("ExecuteWorkError")!!.forEach { error ->
            val sizeBefore = executeWorkErrors.size
            executeWorkErrors = executeWorkErrors.filter { kClass -> error != kClass.simpleName }
            if (executeWorkErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + ExecuteWorkError::class.simpleName)
            }
        }

        if (executeWorkErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + ExecuteWorkError::class.simpleName)
        }

        var getAccountErrors = GetAccountError::class.nestedClasses.filter { kClass -> kClass != GetAccountError.Unexpected::class }
        jsonObject.array<String>("GetAccountError")!!.forEach { error ->
            val sizeBefore = getAccountErrors.size
            getAccountErrors = getAccountErrors.filter { kClass -> error != kClass.simpleName }
            if (getAccountErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + GetAccountError::class.simpleName)
            }
        }

        if (getAccountErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + GetAccountError::class.simpleName)
        }

        var getChildrenErrors = GetChildrenError::class.nestedClasses.filter { kClass -> kClass != GetChildrenError.Unexpected::class }
        jsonObject.array<String>("GetChildrenError")!!.forEach { error ->
            val sizeBefore = getChildrenErrors.size
            getChildrenErrors = getChildrenErrors.filter { kClass -> error != kClass.simpleName }
            if (getChildrenErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + GetChildrenError::class.simpleName)
            }
        }

        if (getChildrenErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + GetChildrenError::class.simpleName)
        }

        var getFileByIdErrors = GetFileByIdError::class.nestedClasses.filter { kClass -> kClass != GetFileByIdError.Unexpected::class }
        jsonObject.array<String>("GetFileByIdError")!!.forEach { error ->
            val sizeBefore = getFileByIdErrors.size
            getFileByIdErrors = getFileByIdErrors.filter { kClass -> error != kClass.simpleName }
            if (getFileByIdErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + GetFileByIdError::class.simpleName)
            }
        }

        if (getFileByIdErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + GetFileByIdError::class.simpleName)
        }

        var getRootErrors = GetRootError::class.nestedClasses.filter { kClass -> kClass != GetRootError.Unexpected::class }
        jsonObject.array<String>("GetRootError")!!.forEach { error ->
            val sizeBefore = getRootErrors.size
            getRootErrors = getRootErrors.filter { kClass -> error != kClass.simpleName }
            if (getRootErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + GetRootError::class.simpleName)
            }
        }

        if (getRootErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + GetRootError::class.simpleName)
        }

        var getStateErrors = GetStateError::class.nestedClasses.filter { kClass -> kClass != GetStateError.Unexpected::class }
        jsonObject.array<String>("GetStateError")!!.forEach { error ->
            val sizeBefore = getStateErrors.size
            getStateErrors = getStateErrors.filter { kClass -> error != kClass.simpleName }
            if (getStateErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + GetStateError::class.simpleName)
            }
        }

        if (getStateErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + GetStateError::class.simpleName)
        }

        var getUsageErrors = GetUsageError::class.nestedClasses.filter { kClass -> kClass != GetUsageError.Unexpected::class }
        jsonObject.array<String>("GetUsageError")!!.forEach { error ->
            val sizeBefore = getUsageErrors.size
            getUsageErrors = getUsageErrors.filter { kClass -> error != kClass.simpleName }
            if (getStateErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + GetUsageError::class.simpleName)
            }
        }

        if (getUsageErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + GetUsageError::class.simpleName)
        }

        var importErrors = ImportError::class.nestedClasses.filter { kClass -> kClass != ImportError.Unexpected::class }
        jsonObject.array<String>("ImportError")!!.forEach { error ->
            val sizeBefore = importErrors.size
            importErrors = importErrors.filter { kClass -> error != kClass.simpleName }
            if (importErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + ImportError::class.simpleName)
            }
        }

        if (importErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + ImportError::class.simpleName)
        }

        var insertFileErrors = InsertFileError::class.nestedClasses.filter { kClass -> kClass != InsertFileError.Unexpected::class }
        jsonObject.array<String>("InsertFileError")!!.forEach { error ->
            val sizeBefore = insertFileErrors.size
            insertFileErrors = insertFileErrors.filter { kClass -> error != kClass.simpleName }
            if (insertFileErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + InsertFileError::class.simpleName)
            }
        }

        if (insertFileErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + InsertFileError::class.simpleName)
        }

        var migrationErrors = MigrationError::class.nestedClasses.filter { kClass -> kClass != MigrationError.Unexpected::class }
        jsonObject.array<String>("MigrationError")!!.forEach { error ->
            val sizeBefore = migrationErrors.size
            migrationErrors = migrationErrors.filter { kClass -> error != kClass.simpleName }
            if (migrationErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + MigrationError::class.simpleName)
            }
        }

        if (migrationErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + MigrationError::class.simpleName)
        }

        var moveFileErrors = MoveFileError::class.nestedClasses.filter { kClass -> kClass != MoveFileError.Unexpected::class }
        jsonObject.array<String>("MoveFileError")!!.forEach { error ->
            val sizeBefore = moveFileErrors.size
            moveFileErrors = moveFileErrors.filter { kClass -> error != kClass.simpleName }
            if (moveFileErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + MoveFileError::class.simpleName)
            }
        }

        if (moveFileErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + MoveFileError::class.simpleName)
        }

        var readDocumentErrors = ReadDocumentError::class.nestedClasses.filter { kClass -> kClass != ReadDocumentError.Unexpected::class }
        jsonObject.array<String>("ReadDocumentError")!!.forEach { error ->
            val sizeBefore = readDocumentErrors.size
            readDocumentErrors = readDocumentErrors.filter { kClass -> error != kClass.simpleName }
            if (readDocumentErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + ReadDocumentError::class.simpleName)
            }
        }

        if (readDocumentErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + ReadDocumentError::class.simpleName)
        }

        var renameFileErrors = RenameFileError::class.nestedClasses.filter { kClass -> kClass != RenameFileError.Unexpected::class }
        jsonObject.array<String>("RenameFileError")!!.forEach { error ->
            val sizeBefore = renameFileErrors.size
            renameFileErrors = renameFileErrors.filter { kClass -> error != kClass.simpleName }
            if (renameFileErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + RenameFileError::class.simpleName)
            }
        }

        if (renameFileErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + RenameFileError::class.simpleName)
        }

        var setLastSyncedErrors = SetLastSyncedError::class.nestedClasses.filter { kClass -> kClass != SetLastSyncedError.Unexpected::class }
        jsonObject.array<String>("SetLastSyncedError")!!.forEach { error ->
            val sizeBefore = setLastSyncedErrors.size
            setLastSyncedErrors = setLastSyncedErrors.filter { kClass -> error != kClass.simpleName }
            if (setLastSyncedErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + SetLastSyncedError::class.simpleName)
            }
        }

        if (setLastSyncedErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + SetLastSyncedError::class.simpleName)
        }

        var syncAllErrors = SyncAllError::class.nestedClasses.filter { kClass -> kClass != SyncAllError.Unexpected::class }
        jsonObject.array<String>("SyncAllError")!!.forEach { error ->
            val sizeBefore = syncAllErrors.size
            syncAllErrors = syncAllErrors.filter { kClass -> error != kClass.simpleName }
            if (syncAllErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + SyncAllError::class.simpleName)
            }
        }

        if (syncAllErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + SyncAllError::class.simpleName)
        }

        var writeToDocumentErrors = WriteToDocumentError::class.nestedClasses.filter { kClass -> kClass != WriteToDocumentError.Unexpected::class }
        jsonObject.array<String>("WriteToDocumentError")!!.forEach { error ->
            val sizeBefore = writeToDocumentErrors.size
            writeToDocumentErrors = writeToDocumentErrors.filter { kClass -> error != kClass.simpleName }
            if (writeToDocumentErrors.size == sizeBefore && error != stubError) {
                throw Throwable(error + unrecognizedErrorTemplate + WriteToDocumentError::class.simpleName)
            }
        }

        if (writeToDocumentErrors.isNotEmpty()) {
            throw Throwable(obsoleteErrorTemplate + WriteToDocumentError::class.simpleName)
        }

        return Unit
    }

    override fun toJson(value: Any): String = Klaxon().toJsonString(value)
}
