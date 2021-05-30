package app.lockbook

import app.lockbook.core.getDBState
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<State>(
            CoreModel.getDBState(config).component1()
        )
    }

    @Test
    fun getDBStateUnexpectedError() {
        assertType<GetStateError.Unexpected>(
            Klaxon().converter(getStateConverter).parse<Result<State, GetStateError>>(getDBState(""))?.component2()
        )
    }
}
