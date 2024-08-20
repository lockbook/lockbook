package app.lockbook.model

import app.lockbook.util.*
import com.github.michaelbull.result.Err
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.map
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
        Err(CoreError.Unexpected("Cannot parse json: ${e.message}"))
    }

    val setUpInitLoggerParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), InitError.serializer())
        }
    }

    fun init(config: Config): Result<Unit, CoreError<InitError>> =
        setUpInitLoggerParser.tryParse(app.lockbook.core.init(setUpInitLoggerParser.encodeToString(config)))

    fun getPtr(): Long = app.lockbook.core.getCorePtr()

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
            createPolyRelation(File.serializer(), GetRootError.serializer())
        }
    }

    fun getRoot(): Result<File, CoreError<GetRootError>> =
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
        timeStamp: Long
    ): String = app.lockbook.core.convertToHumanDuration(timeStamp)

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
                ListSerializer(File.serializer()),
                GetChildrenError.serializer()
            )
        }
    }

    fun getChildren(parentId: String): Result<List<File>, CoreError<GetChildrenError>> =
        getChildrenParser.tryParse(
            app.lockbook.core.getChildren(parentId)
        )

    private val getFileByIdParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(File.serializer(), GetFileByIdError.serializer())
        }
    }

    fun getFileById(id: String): Result<File, CoreError<GetFileByIdError>> =
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

    private val createFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(File.serializer(), CreateFileError.serializer())
        }
    }

    fun createFile(
        parentId: String,
        name: String,
        fileType: FileType
    ): Result<File, CoreError<CreateFileError>> =
        createFileParser.tryParse(
            app.lockbook.core.createFile(
                name,
                parentId,
                createFileParser.encodeToString(fileType)
            )
        )

    private val createLinkParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), CreateFileError.serializer())
        }
    }

    fun createLink(
        name: String,
        id: String,
        parentId: String
    ): Result<Unit, CoreError<CreateFileError>> =
        createLinkParser.tryParse(
            app.lockbook.core.createLink(
                name,
                id,
                parentId
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
            createPolyRelation(app.lockbook.util.SyncStatus.serializer(), CalculateWorkError.serializer())
        }
    }

    fun calculateWork(): Result<app.lockbook.util.SyncStatus, CoreError<CalculateWorkError>> =
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
            createPolyRelation(Unit.serializer(), UpgradeAccountGooglePlayError.serializer())
        }
    }

    fun upgradeAccountGooglePlay(purchaseToken: String, accountId: String): Result<Boolean, CoreError<UpgradeAccountGooglePlayError>> =
        upgradeAccountGooglePlayParser.tryParse(
            app.lockbook.core.upgradeAccountGooglePlay(purchaseToken, accountId)
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

    private val getLocalChangesParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(String.serializer()), Empty.serializer())
        }
    }

    fun getLocalChanges(): Result<HashSet<String>, CoreError<Empty>> =
        getLocalChangesParser.tryParse<List<String>, Empty>(
            app.lockbook.core.getLocalChanges()
        ).map { it.toHashSet() }

    private val listMetadatasParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(File.serializer()), Empty.serializer())
        }
    }

    fun listMetadatas(): Result<List<File>, CoreError<Empty>> =
        listMetadatasParser.tryParse(
            app.lockbook.core.listMetadatas()
        )

    private val startSearchParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), Empty.serializer())
        }
    }

    fun startSearch(searchDocumentsViewModel: SearchDocumentsViewModel): Result<Unit, CoreError<Empty>> =
        startSearchParser.tryParse(
            app.lockbook.core.startSearch(searchDocumentsViewModel)
        )

    private val searchParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), Empty.serializer())
        }
    }

    fun search(query: String): Result<Unit, CoreError<Empty>> =
        searchParser.tryParse(
            app.lockbook.core.search(query)
        )

    private val endSearchParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), Empty.serializer())
        }
    }

    fun endSearch(): Result<Unit, CoreError<Empty>> =
        endSearchParser.tryParse(
            app.lockbook.core.endSearch()
        )

    private val stopCurrentSearchParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), Empty.serializer())
        }
    }

    private val shareFileParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(Unit.serializer(), ShareFileError.serializer())
        }
    }

    fun shareFile(id: String, username: String, mode: ShareMode): Result<Unit, CoreError<ShareFileError>> =
        shareFileParser.tryParse(
            app.lockbook.core.shareFile(id, username, shareFileParser.encodeToString(mode))
        )

    private val getPendingSharesParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(File.serializer()), Empty.serializer())
        }
    }

    fun getPendingShares(): Result<List<File>, CoreError<Empty>> =
        getPendingSharesParser.tryParse(
            app.lockbook.core.getPendingShares()
        )

    private val deletePendingSharesParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(File.serializer()), DeletePendingShareError.serializer())
        }
    }

    fun deletePendingShare(id: String): Result<List<File>, CoreError<DeletePendingShareError>> =
        deletePendingSharesParser.tryParse(
            app.lockbook.core.deletePendingShare(id)
        )

    private val suggestedDocsParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(String.serializer()), Empty.serializer())
        }
    }

    fun suggestedDocs(): Result<List<String>, CoreError<Empty>> =
        suggestedDocsParser.tryParse(
            app.lockbook.core.suggestedDocs()
        )

    private val deleteAccountParser = Json {
        serializersModule = SerializersModule {
            createPolyRelation(ListSerializer(Unit.serializer()), DeleteAccountError.serializer())
        }
    }

    fun deleteAccount(): Result<Unit, CoreError<DeleteAccountError>> =
        deleteAccountParser.tryParse(
            app.lockbook.core.deleteAccount()
        )

    fun logout() = app.lockbook.core.logout()
}
