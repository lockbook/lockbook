package app.lockbook

import app.lockbook.core.writeDocument
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class WriteToDocumentTest {
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
    fun writeToDocumentOk() {
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

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.writeContentToDocument(config, document.id, "").component1()
        )
    }

    @Test
    fun writeToDocumentFileDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<WriteToDocumentError.FileDoesNotExist>(
            CoreModel.writeContentToDocument(config, generateId(), "").component2()
        )
    }

    @Test
    fun writeToDocumentFolderTreatedAsDocument() {
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

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<WriteToDocumentError.FolderTreatedAsDocument>(
            CoreModel.writeContentToDocument(config, folder.id, "").component2()
        )
    }

    @Test
    fun writeToDocumentUnexpectedError() {
        assertType<WriteToDocumentError.Unexpected>(
            Klaxon().converter(writeDocumentConverter).parse<Result<Unit, WriteToDocumentError>>(writeDocument("", "", ""))?.component2()
        )
    }
}
