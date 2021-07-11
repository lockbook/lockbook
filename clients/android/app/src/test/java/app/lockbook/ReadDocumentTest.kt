package app.lockbook

import app.lockbook.core.readDocument
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ReadDocumentTest {
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
    fun readDocumentOk() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val document = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrap()

        CoreModel.readDocument(config, document.id).unwrap()
    }

    @Test
    fun readFolder() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        val rootFileMetadata = CoreModel.getRoot(config).unwrap()

        val folder = CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrap()

        CoreModel.readDocument(config, folder.id)
            .unwrapErrorType<ReadDocumentError.TreatedFolderAsDocument>()
    }

    @Test
    fun readDocumentDoesNotExist() {
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.readDocument(config, generateId())
            .unwrapErrorType<ReadDocumentError.FileDoesNotExist>()
    }

    @Test
    fun readDocumentUnexpectedError() {
        Klaxon().converter(readDocumentConverter)
            .parse<Result<String, ReadDocumentError>>(readDocument("", ""))
            .unwrapErrorType<ReadDocumentError.Unexpected>()
    }
}
