package app.lockbook

import app.lockbook.core.getAllErrorVariants
import app.lockbook.util.*
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json
import org.junit.BeforeClass
import org.junit.Test

class AllErrorVariantsTest {
    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lb_external_interface")
        }
    }

    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun checkIfAllErrorsPresent() {
        json.decodeFromString<CheckAllErrorsPresent>(getAllErrorVariants())
    }

    @Serializable
    data class CheckAllErrorsPresent(
        @SerialName("GetUsageError")
        val getUsageErrors: List<GetUsageError>,

        @SerialName("GetStateError")
        val getStateErrors: List<GetStateError> = listOf(),

        @SerialName("CreateAccountError")
        val createAccountErrors: List<CreateAccountError>,

        @SerialName("ImportError")
        val importErrors: List<ImportError>,

        @SerialName("AccountExportError")
        val accountExportErrors: List<AccountExportError>,

        @SerialName("GetAccountError")
        val getAccountErrors: List<GetAccountError>,

        @SerialName("GetRootError")
        val getRootErrors: List<GetRootError>,

        @SerialName("WriteToDocumentError")
        val writeToDocumentErrors: List<WriteToDocumentError>,

        @SerialName("CreateFileError")
        val createFileErrors: List<CreateFileError>,

        @SerialName("GetChildrenError")
        val getChildrenErrors: List<GetChildrenError> = listOf(),

        @SerialName("GetFileByIdError")
        val getFileByIdErrors: List<GetFileByIdError>,

        @SerialName("FileDeleteError")
        val fileDeleteErrors: List<FileDeleteError>,

        @SerialName("ReadDocumentError")
        val readDocumentErrors: List<ReadDocumentError>,

        @SerialName("RenameFileError")
        val renameFileErrors: List<RenameFileError>,

        @SerialName("MoveFileError")
        val moveFileErrors: List<MoveFileError>,

        @SerialName("SyncAllError")
        val syncAllErrors: List<SyncAllError>,

        @SerialName("CalculateWorkError")
        val calculateWorkErrors: List<CalculateWorkError>
    )
}
