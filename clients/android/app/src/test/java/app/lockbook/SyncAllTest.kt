package app.lockbook

import app.lockbook.core.syncAll
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class SyncAllTest {
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
    fun syncAllOk() {
        assertType<Unit>(
            this::syncAllOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::syncAllOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::syncAllOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::syncAllOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::syncAllOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::syncAllOk.name,
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            this::syncAllOk.name,
            CoreModel.syncAllFiles(config).component1()
        )
    }

    @Test
    fun syncAllNoAccount() {
        assertType<SyncAllError.NoAccount>(
            this::syncAllNoAccount.name,
            CoreModel.syncAllFiles(config).component2()
        )
    }

    @Test
    fun syncAllUnexpectedError() {
        val syncResult: Result<Unit, SyncAllError>? =
            Klaxon().converter(syncAllConverter).parse(syncAll(Klaxon().toJsonString("")))

        assertType<SyncAllError.UnexpectedError>(
            this::syncAllUnexpectedError.name,
            syncResult?.component2()
        )
    }
}
