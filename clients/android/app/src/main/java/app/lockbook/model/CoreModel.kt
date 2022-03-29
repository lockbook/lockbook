package app.lockbook.model

import app.lockbook.core.*
import app.lockbook.util.*
import com.github.michaelbull.result.Err
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

    inline fun <reified C, reified E> Json.tryParse(json: String): Result<C, CoreError<E>>
    where E : Enum<E>, E : UiCoreError = try {
        decodeFromString<IntermCoreResult<C, E>>(json).toResult()
    } catch (e: Exception) {
        Err(CoreError.Unexpected("Cannot parse json."))
    }

    private val setUpInitLoggerParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), InitLoggerError.serializer())
        }
    }

    fun setUpInitLogger(path: String): Result<Unit, CoreError<InitLoggerError>> =
        setUpInitLoggerParser.tryParse(initLogger(path))

    val getDBStateParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(State.serializer(), GetStateError.serializer())
        }
    }

    fun getDBState(config: Config): Result<State, CoreError<GetStateError>> =
        getDBStateParser.tryParse(
            getDBState(
                getDBStateParser.encodeToString(config)
            )
        )

    val migrateDBParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), MigrationError.serializer())
        }
    }

    fun migrateDB(config: Config): Result<Unit, CoreError<MigrationError>> =
        migrateDBParser.tryParse(
            migrateDB(
                migrateDBParser.encodeToString(
                    config
                )
            )
        )

    val createAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), CreateAccountError.serializer())
        }
    }

    fun createAccount(
        config: Config,
        account: String
    ): Result<Account, CoreError<CreateAccountError>> = createAccountParser.tryParse(
        createAccount(
            createAccountParser.encodeToString(config),
            account,
            getAPIURL()
        )
    )

    val importAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), ImportError.serializer())
        }
    }

    fun importAccount(config: Config, account: String): Result<Account, CoreError<ImportError>> = importAccountParser.tryParse(
        importAccount(
            importAccountParser.encodeToString(
                config
            ),
            account
        )
    )

    val exportAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(String.serializer(), AccountExportError.serializer())
        }
    }

    fun exportAccount(config: Config): Result<String, CoreError<AccountExportError>> =
        exportAccountParser.tryParse(
            exportAccount(
                exportAccountParser.encodeToString(config)
            )
        )

    val syncAllParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), SyncAllError.serializer())
        }
    }

    fun syncAll(config: Config, syncModel: SyncModel?): Result<Unit, CoreError<SyncAllError>> =
        syncAllParser.tryParse(
            if (syncModel != null) {
                syncAll(syncAllParser.encodeToString(config), syncModel)
            } else {
                backgroundSync(syncAllParser.encodeToString(config))
            }
        )

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
        writeToDocumentParser.tryParse(
            writeDocument(
                writeToDocumentParser.encodeToString(config),
                id,
                content
            )
        )

    val getRootParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), GetRootError.serializer())
        }
    }

    fun getRoot(config: Config): Result<DecryptedFileMetadata, CoreError<GetRootError>> =
        getRootParser.tryParse(
            getRoot(
                getRootParser.encodeToString(config)
            )
        )

    val getAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), GetAccountError.serializer())
        }
    }

    fun getAccount(config: Config): Result<Account, CoreError<GetAccountError>> =
        getAccountParser.tryParse(
            getAccount(
                getAccountParser.encodeToString(config)
            )
        )

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
        getUsageParser.tryParse(
            getUsage(
                getUsageParser.encodeToString(config)
            )
        )

    val getUncompressedUsageParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(UsageItemMetric.serializer(), GetUsageError.serializer())
        }
    }

    fun getUncompressedUsage(
        config: Config
    ): Result<UsageItemMetric, CoreError<GetUsageError>> =
        getUncompressedUsageParser.tryParse(
            getUncompressedUsage(getUncompressedUsageParser.encodeToString(config))
        )

    val getChildrenParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(DecryptedFileMetadata.serializer()), GetChildrenError.serializer())
        }
    }

    fun getChildren(
        config: Config,
        parentId: String
    ): Result<List<DecryptedFileMetadata>, CoreError<GetChildrenError>> =
        getChildrenParser.tryParse(
            getChildren(getChildrenParser.encodeToString(config), parentId)
        )

    val getFileByIdParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), GetFileByIdError.serializer())
        }
    }

    fun getFileById(
        config: Config,
        id: String
    ): Result<DecryptedFileMetadata, CoreError<GetFileByIdError>> =
        getFileByIdParser.tryParse(
            getFileById(getFileByIdParser.encodeToString(config), id)
        )

    val readDocumentParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(String.serializer(), ReadDocumentError.serializer())
        }
    }

    fun readDocument(
        config: Config,
        id: String
    ): Result<String, CoreError<ReadDocumentError>> =
        readDocumentParser.tryParse(
            readDocument(
                readDocumentParser.encodeToString(config),
                id
            )
        )

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
        saveDocumentToDiskParser.tryParse(
            saveDocumentToDisk(saveDocumentToDiskParser.encodeToString(config), id, location)
        )

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
        exportDrawingToDiskParser.tryParse(
            exportDrawingToDisk(
                exportDrawingToDiskParser.encodeToString(config),
                id,
                exportDrawingToDiskParser.encodeToString(format),
                location
            )
        )

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
        createFileParser.tryParse(
            createFile(
                createFileParser.encodeToString(config),
                name,
                parentId,
                createFileParser.encodeToString(fileType)
            )
        )

    val deleteFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), FileDeleteError.serializer())
        }
    }

    fun deleteFile(
        config: Config,
        id: String
    ): Result<Unit, CoreError<FileDeleteError>> =
        deleteFileParser.tryParse(
            deleteFile(
                deleteFileParser.encodeToString(
                    config
                ),
                id
            )
        )

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
        renameFileParser.tryParse(
            renameFile(
                renameFileParser.encodeToString(
                    config
                ),
                id, name
            )
        )

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
        moveFileParser.tryParse(
            moveFile(
                moveFileParser.encodeToString(
                    config
                ),
                id, parentId
            )
        )

    val calculateWorkParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(WorkCalculated.serializer(), CalculateWorkError.serializer())
        }
    }

    fun calculateWork(config: Config): Result<WorkCalculated, CoreError<CalculateWorkError>> =
        calculateWorkParser.tryParse(
            calculateWork(calculateWorkParser.encodeToString(config))
        )
}
