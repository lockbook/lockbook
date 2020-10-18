package app.lockbook

import app.lockbook.core.setLastSynced
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class SetLastSyncedTest {
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
    fun setLastSyncedOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<Unit>(
            CoreModel.setLastSynced(config, 1).component1()
        )
    }

    @Test
    fun setLastSyncedUnexpectedError() {
        val lastSyncedResult: Result<Unit, CoreError>? =
            Klaxon().converter(setLastSyncedConverter).parse(setLastSynced("", 0))

        assertType<CoreError.Unexpected>(
            lastSyncedResult?.component2()
        )
    }
}
