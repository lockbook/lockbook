package app.lockbook

import app.lockbook.core.writeDocument
import app.lockbook.utils.*
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
            this::writeToDocumentOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::writeToDocumentOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::writeToDocumentOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<Unit>(
            this::writeToDocumentOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::writeToDocumentOk.name,
            CoreModel.writeContentToDocument(config, document.id, "").component1()
        )
    }

    @Test
    fun writeToDocumentFileDoesNotExist() {
        assertType<Unit>(
            this::writeToDocumentFileDoesNotExist.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<WriteToDocumentError.FileDoesNotExist>(
            this::writeToDocumentFileDoesNotExist.name,
            CoreModel.writeContentToDocument(config, generateId(), "").component2()
        )
    }

    @Test
    fun writeToDocumentFolderTreatedAsDocument() {
        assertType<Unit>(
            this::writeToDocumentFolderTreatedAsDocument.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::writeToDocumentFolderTreatedAsDocument.name,
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::writeToDocumentFolderTreatedAsDocument.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::writeToDocumentFolderTreatedAsDocument.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<WriteToDocumentError.FolderTreatedAsDocument>(
            this::writeToDocumentFolderTreatedAsDocument.name,
            CoreModel.writeContentToDocument(config, folder.id, "").component2()
        )
    }

    @Test
    fun writeToDocumentUnexpectedError() {
        val writeResult: Result<Unit, WriteToDocumentError>? =
            Klaxon().converter(writeDocumentConverter).parse(writeDocument("", "", ""))

        assertType<WriteToDocumentError.UnexpectedError>(
            this::writeToDocumentUnexpectedError.name,
            writeResult?.component2()
        )
    }
}
