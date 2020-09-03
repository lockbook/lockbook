package app.lockbook

import app.lockbook.core.executeSyncWork
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ExecuteWorkTest {
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
    fun executeWorkOk() {
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

        val folder = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )
        repeat(10) {
            val syncWork = assertTypeReturn<WorkCalculated>(
                CoreModel.calculateFileSyncWork(config).component1()
            )

            for (workUnit in syncWork.work_units) {
                assertType<Unit>(
                    CoreModel.executeFileSyncWork(
                        config,
                        assertTypeReturn(
                            CoreModel.getAccount(config).component1()
                        ),
                        workUnit
                    ).component1()
                )
            }
        }
    }

    @Test
    fun executeWorkImportAccountOk() {
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

        val folder = assertTypeReturn<FileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            CoreModel.insertFile(config, folder).component1()
        )

        assertType<Unit>(
            CoreModel.syncAllFiles(config).component1()
        )

        val exportAccountString = assertTypeReturn<String>(
            CoreModel.exportAccount(config).component1()
        )

        config = Config(createRandomPath())

        assertType<Unit>(
            CoreModel.importAccount(config, exportAccountString).component1()
        )

        repeat(10) {
            val syncWork = assertTypeReturn<WorkCalculated>(
                CoreModel.calculateFileSyncWork(config).component1()
            )

            for (workUnit in syncWork.work_units) {
                assertType<Unit>(
                    CoreModel.executeFileSyncWork(
                        config,
                        assertTypeReturn(
                            CoreModel.getAccount(config).component1()
                        ),
                        workUnit
                    ).component1()
                )
            }
        }
    }

    @Test
    fun executeWorkUnexpectedError() {
        val executeSyncWorkResult: Result<Unit, ExecuteWorkError>? =
            Klaxon().converter(executeSyncWorkConverter).parse(executeSyncWork("", "", ""))

        assertType<ExecuteWorkError.UnexpectedError>(
            executeSyncWorkResult?.component2()
        )
    }
}
