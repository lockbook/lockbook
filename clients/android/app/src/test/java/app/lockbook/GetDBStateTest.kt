package app.lockbook

import app.lockbook.core.getDBState
import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.GetStateError
import app.lockbook.util.IntermCoreResult
import app.lockbook.util.State
import kotlinx.serialization.decodeFromString
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class GetDBStateTest {
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
    fun getDBStateOk() {
        CoreModel.createAccount(config, generateAlphaString()).unwrapOk()

        CoreModel.getDBState(config).unwrapOk()
    }

    @Test
    fun getDBStateUnexpectedError() {
        CoreModel.getDBStateParser.decodeFromString<IntermCoreResult<State, GetStateError>>(
            getDBState("")
        ).unwrapUnexpected()
    }
}
