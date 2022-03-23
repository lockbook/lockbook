package app.lockbook

import app.lockbook.core.calculateWork
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import kotlinx.serialization.decodeFromString
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
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        val rootFileMetadata = CoreModel.getRoot(config).unwrapOk()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Document
        ).unwrapOk()

        CoreModel.createFile(
            config,
            rootFileMetadata.id,
            generateAlphaString(),
            FileType.Folder
        ).unwrapOk()

        CoreModel.calculateWork(config).unwrapOk()
    }

    @Test
    fun calculateWorkUnexpectedError() {
        CoreModel.calculateWorkParser.decodeFromString<IntermCoreResult<WorkCalculated, CalculateWorkError>>(
            calculateWork("")
        ).unwrapUnexpected()
    }
}
