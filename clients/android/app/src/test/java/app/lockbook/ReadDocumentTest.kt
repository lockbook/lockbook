package app.lockbook

import app.lockbook.core.readDocument
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<String>(
            CoreModel.readDocument(config, document.id).component1()
        )
    }

    @Test
    fun readFolder() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<ReadDocumentError.TreatedFolderAsDocument>(
            CoreModel.readDocument(config, folder.id).component2()
        )
    }

    @Test
    fun readDocumentDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<ReadDocumentError.FileDoesNotExist>(
            CoreModel.readDocument(config, generateId()).component2()
        )
    }

    @Test
    fun readDocumentUnexpectedError() {
        assertType<ReadDocumentError.Unexpected>(
            Klaxon().converter(readDocumentConverter).parse<Result<String, ReadDocumentError>>(readDocument("", ""))?.component2()
        )
    }
}
