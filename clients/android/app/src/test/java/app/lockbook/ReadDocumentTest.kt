package app.lockbook

import app.lockbook.core.readDocument
import app.lockbook.utils.*
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
            this::readDocumentOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::readDocumentOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::readDocumentOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<Unit>(
            this::readDocumentOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<DecryptedValue>(
            this::readDocumentOk.name,
            CoreModel.getDocumentContent(config, document.id).component1()
        )
    }

    @Test
    fun readFolder() {
        assertType<Unit>(
            this::readFolder.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::readFolder.name,
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::readFolder.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::readFolder.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<ReadDocumentError.TreatedFolderAsDocument>(
            this::readFolder.name,
            CoreModel.getDocumentContent(config, folder.id).component2()
        )
    }

    @Test
    fun readDocumentDoesNotExist() {
        assertType<Unit>(
            this::readDocumentDoesNotExist.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<ReadDocumentError.FileDoesNotExist>(
            this::readDocumentDoesNotExist.name,
            CoreModel.getDocumentContent(config, generateId()).component2()
        )
    }

    @Test
    fun readDocumentUnexpectedError() {
        val getDocumentResult: Result<DecryptedValue, ReadDocumentError>? =
            Klaxon().converter(readDocumentConverter).parse(readDocument("", ""))

        assertType<ReadDocumentError.UnexpectedError>(
            this::readDocumentUnexpectedError.name,
            getDocumentResult?.component2()
        )
    }
}
