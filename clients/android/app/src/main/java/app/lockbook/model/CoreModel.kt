package app.lockbook.model

import app.lockbook.core.*
import app.lockbook.util.*
import com.github.michaelbull.result.Result
import kotlinx.serialization.KSerializer
import kotlinx.serialization.PolymorphicSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.modules.SerializersModule
import kotlinx.serialization.modules.SerializersModuleBuilder
import kotlinx.serialization.modules.polymorphic
import kotlinx.serialization.modules.subclass

object CoreModel {
    private const val PROD_API_URL = "https://api.prod.lockbook.net"
    fun getAPIURL(): String = System.getenv("API_URL") ?: PROD_API_URL

    private val serializationModule = SerializersModule {
//        polymorphic(IntermCoreResult::class) {
//            subclass(IntermCoreResult.Ok.serializer(Account.serializer()))
//            subclass(IntermCoreResult.Err.serializer(GetAccountError.serializer()))
//        }
//
//        fun <O> SerializersModuleBuilder.createPolyRelation(serializer: KSerializer<O>) {
//            polymorphic(IntermCoreResult::class) {
//                subclass(IntermCoreResult.Ok.serializer(serializer))
//                subclass(IntermCoreResult.Err.serializer(PolymorphicSerializer(Any::class)))
//            }
//        }
//
//        // Init Logger
//        // Migrate DB
//        // Sync All
//        // Write To Document Error
//        // Save Document To Disk
//        // Export Drawing To Disk
//        // Delete File
//        // Rename File
//        // Move File
//        createPolyRelation(Unit.serializer())
//
//        // Get DB State
//        createPolyRelation(State.serializer())
//
//        // Create Account
//        // Import Account
//        // Get Account
//        createPolyRelation(Account.serializer())
//
//        // Export Account
//        // Read Document
//        createPolyRelation(String.serializer())
//
//        // Get Root
//        // Get File By Id
//        // Create File
//        createPolyRelation(DecryptedFileMetadata.serializer())
//
//        // Get Usage
//        createPolyRelation(UsageMetrics.serializer())
//
//        // Get Uncompressed Usage
//        createPolyRelation(UsageItemMetric.serializer())
//
//        // Get Children
//        createPolyRelation(ListSerializer(DecryptedFileMetadata.serializer()))
//
//        // Calculate Work
//        createPolyRelation(WorkCalculated.serializer())
    }

    val jsonParser = Json {
        serializersModule = serializationModule
        isLenient = true
    }

    fun setUpInitLogger(path: String): Result<Unit, CoreError<InitLoggerError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, InitLoggerError>>(initLogger(path))
            .toResult()

    fun getDBState(config: Config): Result<State, CoreError<GetStateError>> =
        jsonParser.decodeFromString<IntermCoreResult<State, GetStateError>>(
            getDBState(
                jsonParser.encodeToString(
                    config
                )
            )
        ).toResult()

    fun migrateDB(config: Config): Result<Unit, CoreError<MigrationError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, MigrationError>>(
            migrateDB(
                jsonParser.encodeToString(
                    config
                )
            )
        ).toResult()

    fun createAccount(
        config: Config,
        account: String
    ): Result<Account, CoreError<CreateAccountError>> {
        val a = createAccount(
            jsonParser.encodeToString(config),
            account,
            getAPIURL()
        )

        println("Here $a")

        return jsonParser.decodeFromString<IntermCoreResult<Account, CreateAccountError>>(
            a
        ).toResult()
    }


    fun importAccount(config: Config, account: String): Result<Unit, CoreError<ImportError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, ImportError>>(
            importAccount(
                jsonParser.encodeToString(
                    config
                ), account
            )
        ).toResult()

    fun exportAccount(config: Config): Result<String, CoreError<AccountExportError>> =
        jsonParser.decodeFromString<IntermCoreResult<String, AccountExportError>>(
            exportAccount(
                jsonParser.encodeToString(config)
            )
        ).toResult()

    fun syncAll(config: Config, syncModel: SyncModel?): Result<Unit, CoreError<SyncAllError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, SyncAllError>>(
            if (syncModel != null) {
                syncAll(jsonParser.encodeToString(config), syncModel)
            } else {
                backgroundSync(jsonParser.encodeToString(config))
            }
        ).toResult()

    fun writeToDocument(
        config: Config,
        id: String,
        content: String
    ): Result<Unit, CoreError<WriteToDocumentError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, WriteToDocumentError>>(
            writeDocument(
                jsonParser.encodeToString(config),
                id,
                content
            )
        ).toResult()

    fun getRoot(config: Config): Result<DecryptedFileMetadata, CoreError<GetRootError>> =
        jsonParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetRootError>>(
            getRoot(
                jsonParser.encodeToString(config)
            )
        ).toResult()

    fun getAccount(config: Config): Result<Account, CoreError<GetAccountError>> =
        jsonParser.decodeFromString<IntermCoreResult<Account, GetAccountError>>(
            getAccount(
                jsonParser.encodeToString(config)
            )
        ).toResult()

    fun convertToHumanDuration(
        metadataVersion: Long
    ): String = app.lockbook.core.convertToHumanDuration(metadataVersion)

    fun getUsage(
        config: Config
    ): Result<UsageMetrics, CoreError<GetUsageError>> =
        jsonParser.decodeFromString<IntermCoreResult<UsageMetrics, GetUsageError>>(
            getUsage(
                jsonParser.encodeToString(config)
            )
        ).toResult()


    fun getUncompressedUsage(
        config: Config
    ): Result<UsageItemMetric, CoreError<GetUsageError>> =
        jsonParser.decodeFromString<IntermCoreResult<UsageItemMetric, GetUsageError>>(
            getUncompressedUsage(jsonParser.encodeToString(config))
        ).toResult()

    fun getChildren(
        config: Config,
        parentId: String
    ): Result<List<DecryptedFileMetadata>, CoreError<GetChildrenError>> =
        jsonParser.decodeFromString<IntermCoreResult<List<DecryptedFileMetadata>, GetChildrenError>>(
            getChildren(jsonParser.encodeToString(config), parentId)
        ).toResult()


    fun getFileById(
        config: Config,
        id: String
    ): Result<DecryptedFileMetadata, CoreError<GetFileByIdError>> =
        jsonParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetFileByIdError>>(
            getFileById(jsonParser.encodeToString(config), id)
        ).toResult()


    fun readDocument(
        config: Config,
        id: String
    ): Result<String, CoreError<ReadDocumentError>> =
        jsonParser.decodeFromString<IntermCoreResult<String, ReadDocumentError>>(
            readDocument(
                jsonParser.encodeToString(config),
                id
            )
        ).toResult()

    fun saveDocumentToDisk(
        config: Config,
        id: String,
        location: String
    ): Result<Unit, CoreError<SaveDocumentToDiskError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, SaveDocumentToDiskError>>(
            saveDocumentToDisk(jsonParser.encodeToString(config), id, location)
        ).toResult()


    fun exportDrawingToDisk(
        config: Config,
        id: String,
        format: SupportedImageFormats,
        location: String
    ): Result<Unit, CoreError<ExportDrawingToDiskError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, ExportDrawingToDiskError>>(
            exportDrawingToDisk(
                jsonParser.encodeToString(config),
                id,
                jsonParser.encodeToString(format),
                location
            )
        ).toResult()

    fun createFile(
        config: Config,
        parentId: String,
        name: String,
        fileType: FileType
    ): Result<DecryptedFileMetadata, CoreError<CreateFileError>> =
        jsonParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, CreateFileError>>(
            createFile(
                jsonParser.encodeToString(config),
                name,
                parentId,
                jsonParser.encodeToString(fileType)
            )
        ).toResult()

    fun deleteFile(
        config: Config,
        id: String
    ): Result<Unit, CoreError<FileDeleteError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, FileDeleteError>>(
            deleteFile(
                jsonParser.encodeToString(
                    config
                ), id
            )
        ).toResult()


    fun renameFile(
        config: Config,
        id: String,
        name: String
    ): Result<Unit, CoreError<RenameFileError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, RenameFileError>>(
            renameFile(
                jsonParser.encodeToString(
                    config
                ), id, name
            )
        ).toResult()

    fun moveFile(
        config: Config,
        id: String,
        parentId: String
    ): Result<Unit, CoreError<MoveFileError>> =
        jsonParser.decodeFromString<IntermCoreResult<Unit, MoveFileError>>(
            moveFile(
                jsonParser.encodeToString(
                    config
                ), id, parentId
            )
        ).toResult()

    fun calculateWork(config: Config): Result<WorkCalculated, CoreError<CalculateWorkError>> =
        jsonParser.decodeFromString<IntermCoreResult<WorkCalculated, CalculateWorkError>>(
            calculateWork(jsonParser.encodeToString(config))
        ).toResult()
}
