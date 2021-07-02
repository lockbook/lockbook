package app.lockbook

import app.lockbook.core.calculateWork
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class CalculateWorkTest {
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
    fun calculateWorkOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<ClientFileMetadata>(
            CoreModel.getRoot(config).component1()
        )

        assertType<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        assertType<ClientFileMetadata>(
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<WorkCalculated>(
            CoreModel.calculateWork(config).component1()
        )
    }

    @Test
    fun calculateWorkUnexpectedError() {
        assertType<CalculateWorkError.Unexpected>(
            Klaxon().converter(calculateWorkConverter).parse<Result<WorkCalculated, CalculateWorkError>>(
                calculateWork("")
            )?.component2()
        )
    }
}
