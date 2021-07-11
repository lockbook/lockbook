package app.lockbook

import app.lockbook.core.deleteFile
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import org.junit.*

class DeleteFileTest {
    var config = Config(createRandomPath())

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        config = Config(createRandomPath())
    }

    @Test
    fun deleteFileOk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.deleteFile(config, document.id).unwrap()

        CoreModel.deleteFile(config, folder.id).unwrap()
    }

    @Test
    fun deleteFileNoFileWithThatId() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.deleteFile(config, generateId()).unwrapErrorType<FileDeleteError.FileDoesNotExist>()
    }

    @Test
    fun deleteFileCannotDeleteRoot() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        CoreModel.deleteFile(config, rootFileMetadata.id)
            .unwrapErrorType<FileDeleteError.CannotDeleteRoot>()
    }

    @Test
    fun deleteFileUnexpectedError() {
        Klaxon().converter(deleteFileConverter)
            .parse<Result<Unit, FileDeleteError>>(deleteFile("", ""))
            .unwrapErrorType<FileDeleteError.Unexpected>()
    }
}
