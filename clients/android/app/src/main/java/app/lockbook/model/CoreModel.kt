package app.lockbook.model

import app.lockbook.core.*
import app.lockbook.util.*
import com.github.michaelbull.result.Result
import kotlinx.serialization.KSerializer
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

    private fun <O, E : Enum<E>> SerializersModuleBuilder.createPolyRelation(okSerializer: KSerializer<O>, errSerializer: KSerializer<E>) {
        polymorphic(IntermCoreResult::class) {
            subclass(IntermCoreResult.CoreOk.serializer(okSerializer))
            subclass(IntermCoreResult.CoreErr.serializer(errSerializer))
        }

        polymorphic(IntermCoreError::class) {
            subclass(IntermCoreError.UiError.serializer(errSerializer))
            subclass(IntermCoreError.Unexpected.serializer())
        }
    }

    val setUpInitLoggerParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), InitLoggerError.serializer())
        }
    }

    fun setUpInitLogger(path: String): Result<Unit, CoreError<InitLoggerError>> =
        setUpInitLoggerParser.decodeFromString<IntermCoreResult<Unit, InitLoggerError>>(initLogger(path))
            .toResult()

    val getDBStateParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(State.serializer(), GetStateError.serializer())
        }
    }

    fun getDBState(config: Config): Result<State, CoreError<GetStateError>> =
        getDBStateParser.decodeFromString<IntermCoreResult<State, GetStateError>>(
            getDBState(
                getDBStateParser.encodeToString(
                    config
                )
            )
        ).toResult()

    val migrateDBParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), MigrationError.serializer())
        }
    }

    fun migrateDB(config: Config): Result<Unit, CoreError<MigrationError>> =
        migrateDBParser.decodeFromString<IntermCoreResult<Unit, MigrationError>>(
            migrateDB(
                migrateDBParser.encodeToString(
                    config
                )
            )
        ).toResult()

    val createAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), CreateAccountError.serializer())
        }
    }

    fun createAccount(
        config: Config,
        account: String
    ): Result<Account, CoreError<CreateAccountError>> = createAccountParser.decodeFromString<IntermCoreResult<Account, CreateAccountError>>(
        createAccount(
            createAccountParser.encodeToString(config),
            account,
            getAPIURL()
        )
    ).toResult()

    val importAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), ImportError.serializer())
        }
    }

    fun importAccount(config: Config, account: String): Result<Account, CoreError<ImportError>> {
        val a = importAccount(
            importAccountParser.encodeToString(
                config
            ),
            account
        )

        print(a)
        return importAccountParser.decodeFromString<IntermCoreResult<Account, ImportError>>(
            a
        ).toResult()
    }

    val exportAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(String.serializer(), AccountExportError.serializer())
        }
    }

    fun exportAccount(config: Config): Result<String, CoreError<AccountExportError>> =
        exportAccountParser.decodeFromString<IntermCoreResult<String, AccountExportError>>(
            exportAccount(
                exportAccountParser.encodeToString(config)
            )
        ).toResult()

    val syncAllParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), SyncAllError.serializer())
        }
    }

    fun syncAll(config: Config, syncModel: SyncModel?): Result<Unit, CoreError<SyncAllError>> =
        syncAllParser.decodeFromString<IntermCoreResult<Unit, SyncAllError>>(
            if (syncModel != null) {
                syncAll(syncAllParser.encodeToString(config), syncModel)
            } else {
                backgroundSync(syncAllParser.encodeToString(config))
            }
        ).toResult()

    val writeToDocumentParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), WriteToDocumentError.serializer())
        }
    }

    fun writeToDocument(
        config: Config,
        id: String,
        content: String
    ): Result<Unit, CoreError<WriteToDocumentError>> =
        writeToDocumentParser.decodeFromString<IntermCoreResult<Unit, WriteToDocumentError>>(
            writeDocument(
                writeToDocumentParser.encodeToString(config),
                id,
                content
            )
        ).toResult()

    val getRootParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), GetRootError.serializer())
        }
    }

    fun getRoot(config: Config): Result<DecryptedFileMetadata, CoreError<GetRootError>> =
        getRootParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetRootError>>(
            getRoot(
                getRootParser.encodeToString(config)
            )
        ).toResult()

    val getAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), GetAccountError.serializer())
        }
    }

    fun getAccount(config: Config): Result<Account, CoreError<GetAccountError>> =
        getAccountParser.decodeFromString<IntermCoreResult<Account, GetAccountError>>(
            getAccount(
                getAccountParser.encodeToString(config)
            )
        ).toResult()

    fun convertToHumanDuration(
        metadataVersion: Long
    ): String = app.lockbook.core.convertToHumanDuration(metadataVersion)

    val getUsageParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(UsageMetrics.serializer(), GetUsageError.serializer())
        }
    }

    fun getUsage(
        config: Config
    ): Result<UsageMetrics, CoreError<GetUsageError>> =
        getUsageParser.decodeFromString<IntermCoreResult<UsageMetrics, GetUsageError>>(
            getUsage(
                getUsageParser.encodeToString(config)
            )
        ).toResult()

    val getUncompressedUsageParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(UsageItemMetric.serializer(), GetUsageError.serializer())
        }
    }

    fun getUncompressedUsage(
        config: Config
    ): Result<UsageItemMetric, CoreError<GetUsageError>> =
        getUncompressedUsageParser.decodeFromString<IntermCoreResult<UsageItemMetric, GetUsageError>>(
            getUncompressedUsage(getUncompressedUsageParser.encodeToString(config))
        ).toResult()

    val getChildrenParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(DecryptedFileMetadata.serializer()), GetChildrenError.serializer())
        }
    }

    fun getChildren(
        config: Config,
        parentId: String
    ): Result<List<DecryptedFileMetadata>, CoreError<GetChildrenError>> =
        getChildrenParser.decodeFromString<IntermCoreResult<List<DecryptedFileMetadata>, GetChildrenError>>(
            getChildren(getChildrenParser.encodeToString(config), parentId)
        ).toResult()

    val getFileByIdParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), GetFileByIdError.serializer())
        }
    }

    fun getFileById(
        config: Config,
        id: String
    ): Result<DecryptedFileMetadata, CoreError<GetFileByIdError>> =
        getFileByIdParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, GetFileByIdError>>(
            getFileById(getFileByIdParser.encodeToString(config), id)
        ).toResult()

    val readDocumentParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(String.serializer(), ReadDocumentError.serializer())
        }
    }

    fun readDocument(
        config: Config,
        id: String
    ): Result<String, CoreError<ReadDocumentError>> =
        readDocumentParser.decodeFromString<IntermCoreResult<String, ReadDocumentError>>(
            readDocument(
                readDocumentParser.encodeToString(config),
                id
            )
        ).toResult()

    val saveDocumentToDiskParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), SaveDocumentToDiskError.serializer())
        }
    }

    fun saveDocumentToDisk(
        config: Config,
        id: String,
        location: String
    ): Result<Unit, CoreError<SaveDocumentToDiskError>> =
        saveDocumentToDiskParser.decodeFromString<IntermCoreResult<Unit, SaveDocumentToDiskError>>(
            saveDocumentToDisk(saveDocumentToDiskParser.encodeToString(config), id, location)
        ).toResult()

    val exportDrawingToDiskParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), ExportDrawingToDiskError.serializer())
        }
    }

    fun exportDrawingToDisk(
        config: Config,
        id: String,
        format: SupportedImageFormats,
        location: String
    ): Result<Unit, CoreError<ExportDrawingToDiskError>> =
        exportDrawingToDiskParser.decodeFromString<IntermCoreResult<Unit, ExportDrawingToDiskError>>(
            exportDrawingToDisk(
                exportDrawingToDiskParser.encodeToString(config),
                id,
                exportDrawingToDiskParser.encodeToString(format),
                location
            )
        ).toResult()

    val createFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), CreateFileError.serializer())
        }
    }

    fun createFile(
        config: Config,
        parentId: String,
        name: String,
        fileType: FileType
    ): Result<DecryptedFileMetadata, CoreError<CreateFileError>> =
        createFileParser.decodeFromString<IntermCoreResult<DecryptedFileMetadata, CreateFileError>>(
            createFile(
                createFileParser.encodeToString(config),
                name,
                parentId,
                createFileParser.encodeToString(fileType)
            )
        ).toResult()

    val deleteFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), FileDeleteError.serializer())
        }
    }

    fun deleteFile(
        config: Config,
        id: String
    ): Result<Unit, CoreError<FileDeleteError>> =
        deleteFileParser.decodeFromString<IntermCoreResult<Unit, FileDeleteError>>(
            deleteFile(
                deleteFileParser.encodeToString(
                    config
                ),
                id
            )
        ).toResult()

    val renameFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), RenameFileError.serializer())
        }
    }

    fun renameFile(
        config: Config,
        id: String,
        name: String
    ): Result<Unit, CoreError<RenameFileError>> =
        renameFileParser.decodeFromString<IntermCoreResult<Unit, RenameFileError>>(
            renameFile(
                renameFileParser.encodeToString(
                    config
                ),
                id, name
            )
        ).toResult()

    val moveFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), MoveFileError.serializer())
        }
    }

    fun moveFile(
        config: Config,
        id: String,
        parentId: String
    ): Result<Unit, CoreError<MoveFileError>> =
        moveFileParser.decodeFromString<IntermCoreResult<Unit, MoveFileError>>(
            moveFile(
                moveFileParser.encodeToString(
                    config
                ),
                id, parentId
            )
        ).toResult()

    val calculateWorkParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(WorkCalculated.serializer(), CalculateWorkError.serializer())
        }
    }

    fun calculateWork(config: Config): Result<WorkCalculated, CoreError<CalculateWorkError>> =
        calculateWorkParser.decodeFromString<IntermCoreResult<WorkCalculated, CalculateWorkError>>(
            calculateWork(calculateWorkParser.encodeToString(config))
        ).toResult()
}
