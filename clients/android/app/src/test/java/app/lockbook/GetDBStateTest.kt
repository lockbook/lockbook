package app.lockbook

import app.lockbook.core.getDBState
import app.lockbook.core.getRoot
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.createAccount(config, generateAlphaString()).unwrap()

        CoreModel.getDBState(config).unwrap()
    }

    @Test
    fun getDBStateUnexpectedError() {
        CoreModel.jsonParser.decodeFromString<IntermCoreResult<State, GetStateError>>(
            getDBState("")
        ).unwrapUnexpected()
    }
}
