package app.lockbook.model

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
    private fun getAPIURL(): String = System.getenv("API_URL") ?: PROD_API_URL

    private fun <O, E : Enum<E>> SerializersModuleBuilder.createPolyRelation(
        okSerializer: KSerializer<O>,
        errSerializer: KSerializer<E>
    ) {
        polymorphic(IntermCoreResult::class) {
            subclass(IntermCoreResult.CoreOk.serializer(okSerializer))
            subclass(IntermCoreResult.CoreErr.serializer(errSerializer))
        }

        polymorphic(IntermCoreError::class) {
            subclass(IntermCoreError.UiError.serializer(errSerializer))
            subclass(IntermCoreError.Unexpected.serializer())
        }
    }

    private inline fun <reified C, reified E> Json.tryParse(
        json: String,
        isNullable: Boolean = false
    ): Result<C, CoreError<E>>
            where E : Enum<E>, E : UiCoreError = try {
        decodeFromString<IntermCoreResult<C, E>>(json).toResult(isNullable)
    } catch (e: Exception) {
        Err(CoreError.Unexpected("Cannot parse json."))
    }

    val setUpInitLoggerParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), InitError.serializer())
        }
    }

    fun init(config: Config): Result<Unit, CoreError<InitError>> =
        setUpInitLoggerParser.tryParse(app.lockbook.core.init(setUpInitLoggerParser.encodeToString(config)))

    private val createAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), CreateAccountError.serializer())
        }
    }

    fun createAccount(
        account: String
    ): Result<Account, CoreError<CreateAccountError>> = createAccountParser.tryParse(
        app.lockbook.core.createAccount(
            account,
            getAPIURL()
        )
    )

    private val importAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), ImportError.serializer())
        }
    }

    fun importAccount(account: String): Result<Account, CoreError<ImportError>> =
        importAccountParser.tryParse(
            app.lockbook.core.importAccount(
                account
            )
        )

    private val exportAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(String.serializer(), AccountExportError.serializer())
        }
    }

    fun exportAccount(): Result<String, CoreError<AccountExportError>> =
        exportAccountParser.tryParse(
            app.lockbook.core.exportAccount()
        )

    private val syncAllParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), SyncAllError.serializer())
        }
    }

    fun syncAll(syncModel: SyncModel?): Result<Unit, CoreError<SyncAllError>> =
        syncAllParser.tryParse(
            if (syncModel != null) {
                app.lockbook.core.syncAll(syncModel)
            } else {
                app.lockbook.core.backgroundSync()
            }
        )

    private val writeToDocumentParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), WriteToDocumentError.serializer())
        }
    }

    fun writeToDocument(
        id: String,
        content: String
    ): Result<Unit, CoreError<WriteToDocumentError>> =
        writeToDocumentParser.tryParse(
            app.lockbook.core.writeDocument(
                id,
                content
            )
        )

    private val getRootParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), GetRootError.serializer())
        }
    }

    fun getRoot(): Result<DecryptedFileMetadata, CoreError<GetRootError>> =
        getRootParser.tryParse(
            app.lockbook.core.getRoot()
        )

    private val getAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Account.serializer(), GetAccountError.serializer())
        }
    }

    fun getAccount(): Result<Account, CoreError<GetAccountError>> =
        getAccountParser.tryParse(
            app.lockbook.core.getAccount()
        )

    fun convertToHumanDuration(
        metadataVersion: Long
    ): String = app.lockbook.core.convertToHumanDuration(metadataVersion)

    private val getUsageParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(UsageMetrics.serializer(), GetUsageError.serializer())
        }
    }

    fun getUsage(): Result<UsageMetrics, CoreError<GetUsageError>> =
        getUsageParser.tryParse(
            app.lockbook.core.getUsage()
        )

    private val getUncompressedUsageParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(UsageItemMetric.serializer(), GetUsageError.serializer())
        }
    }

    fun getUncompressedUsage(): Result<UsageItemMetric, CoreError<GetUsageError>> =
        getUncompressedUsageParser.tryParse(
            app.lockbook.core.getUncompressedUsage()
        )

    private val getChildrenParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(
                ListSerializer(DecryptedFileMetadata.serializer()),
                GetChildrenError.serializer()
            )
        }
    }

    fun getChildren(parentId: String): Result<List<DecryptedFileMetadata>, CoreError<GetChildrenError>> =
        getChildrenParser.tryParse(
            app.lockbook.core.getChildren(parentId)
        )

    private val getFileByIdParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), GetFileByIdError.serializer())
        }
    }

    fun getFileById(id: String): Result<DecryptedFileMetadata, CoreError<GetFileByIdError>> =
        getFileByIdParser.tryParse(
            app.lockbook.core.getFileById(id)
        )

    private val readDocumentParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(String.serializer(), ReadDocumentError.serializer())
        }
    }

    fun readDocument(
        id: String
    ): Result<String, CoreError<ReadDocumentError>> =
        readDocumentParser.tryParse(app.lockbook.core.readDocument(id))

    fun readDocumentBytes(
        id: String
    ): ByteArray? = app.lockbook.core.readDocumentBytes(id)

    private val saveDocumentToDiskParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), SaveDocumentToDiskError.serializer())
        }
    }

    fun saveDocumentToDisk(
        id: String,
        location: String
    ): Result<Unit, CoreError<SaveDocumentToDiskError>> =
        saveDocumentToDiskParser.tryParse(
            app.lockbook.core.saveDocumentToDisk(id, location)
        )

    private val exportDrawingToDiskParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), ExportDrawingToDiskError.serializer())
        }
    }

    fun exportDrawingToDisk(
        id: String,
        format: SupportedImageFormats,
        location: String
    ): Result<Unit, CoreError<ExportDrawingToDiskError>> =
        exportDrawingToDiskParser.tryParse(
            app.lockbook.core.exportDrawingToDisk(
                id,
                exportDrawingToDiskParser.encodeToString(format),
                location
            )
        )

    private val createFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(DecryptedFileMetadata.serializer(), CreateFileError.serializer())
        }
    }

    fun createFile(
        parentId: String,
        name: String,
        fileType: FileType
    ): Result<DecryptedFileMetadata, CoreError<CreateFileError>> =
        createFileParser.tryParse(
            app.lockbook.core.createFile(
                name,
                parentId,
                createFileParser.encodeToString(fileType)
            )
        )

    private val deleteFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), FileDeleteError.serializer())
        }
    }

    fun deleteFile(
        id: String
    ): Result<Unit, CoreError<FileDeleteError>> =
        deleteFileParser.tryParse(
            app.lockbook.core.deleteFile(id)
        )

    private val renameFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), RenameFileError.serializer())
        }
    }

    fun renameFile(
        id: String,
        name: String
    ): Result<Unit, CoreError<RenameFileError>> =
        renameFileParser.tryParse(
            app.lockbook.core.renameFile(id, name)
        )

    private val moveFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), MoveFileError.serializer())
        }
    }

    fun moveFile(
        id: String,
        parentId: String
    ): Result<Unit, CoreError<MoveFileError>> =
        moveFileParser.tryParse(
            app.lockbook.core.moveFile(id, parentId)
        )

    private val calculateWorkParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(WorkCalculated.serializer(), CalculateWorkError.serializer())
        }
    }

    fun calculateWork(): Result<WorkCalculated, CoreError<CalculateWorkError>> =
        calculateWorkParser.tryParse(
            app.lockbook.core.calculateWork()
        )

    private val exportFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), ExportFileError.serializer())
        }
    }

    fun exportFile(id: String, destination: String, edit: Boolean): Result<Unit, CoreError<ExportFileError>> =
        exportFileParser.tryParse(
            app.lockbook.core.exportFile(id, destination, edit)
        )

    private val upgradeAccountGooglePlayParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), UpgradeAccountAndroid.serializer())
        }
    }

    fun upgradeAccountGooglePlay(purchaseToken: String, accountID: String): Result<Boolean, CoreError<UpgradeAccountAndroid>> =
        upgradeAccountGooglePlayParser.tryParse(
            app.lockbook.core.upgradeAccountGooglePlay(purchaseToken, accountID)
        )

    private val cancelSubscriptionParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), CancelSubscriptionError.serializer())
        }
    }

    fun cancelSubscription(): Result<Boolean, CoreError<CancelSubscriptionError>> =
        cancelSubscriptionParser.tryParse(
            app.lockbook.core.cancelSubscription()
        )

    private val getSubscriptionInfoParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(SubscriptionInfo.serializer(), GetSubscriptionInfoError.serializer())
            ignoreUnknownKeys = true
        }
    }

    fun getSubscriptionInfo(): Result<SubscriptionInfo?, CoreError<GetSubscriptionInfoError>> =
        getSubscriptionInfoParser.tryParse(
            app.lockbook.core.getSubscriptionInfo(),
            true
        )
}
