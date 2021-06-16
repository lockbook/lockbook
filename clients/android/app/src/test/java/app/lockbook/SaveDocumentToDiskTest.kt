package app.lockbook

import app.lockbook.core.saveDocumentToDisk
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class SaveDocumentToDiskTest {
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
    fun saveDocumentToDiskOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
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
            CoreModel.saveDocumentToDisk(config, document.id, generateFakeRandomPath()).component1()
        )
    }

    @Test
    fun saveDocumentToDiskFolder() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
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

        assertType<SaveDocumentToDiskError.TreatedFolderAsDocument>(
            CoreModel.saveDocumentToDisk(config, folder.id, generateFakeRandomPath()).component2()
        )
    }

    @Test
    fun saveDocumentToDiskDoesNotExist() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<SaveDocumentToDiskError.FileDoesNotExist>(
            CoreModel.saveDocumentToDisk(config, generateId(), generateFakeRandomPath()).component2()
        )
    }

    @Test
    fun saveDocumentToDiskBadPath() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
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

        assertType<SaveDocumentToDiskError.BadPath>(
            CoreModel.saveDocumentToDisk(config, document.id, "").component2()
        )
    }

    @Test
    fun exportDrawingToDiskFileAlreadyExistsInDisk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
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

        val path = generateFakeRandomPath()

        assertType<Unit>(
            CoreModel.saveDocumentToDisk(config, document.id, path).component1()
        )

        assertType<SaveDocumentToDiskError.FileAlreadyExistsInDisk>(
            CoreModel.saveDocumentToDisk(config, document.id, path).component2()
        )
    }

    @Test
    fun saveDocumentToDiskUnexpectedError() {
        assertType<SaveDocumentToDiskError.Unexpected>(
            Klaxon().converter(saveDocumentToDiskConverter).parse<Result<Unit, SaveDocumentToDiskError>>(
                saveDocumentToDisk("", "", "")
            )?.component2()
        )
    }
}
