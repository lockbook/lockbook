package app.lockbook

import app.lockbook.core.setLastSynced
import app.lockbook.model.CoreModel
import app.lockbook.util.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import com.github.michaelbull.result.unwrap
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
        CoreModel.generateAccount(config, generateAlphaString()).unwrap()

        CoreModel.setLastSynced(config, 1).unwrap()
    }

    @Test
    fun setLastSyncedUnexpectedError() {
        Klaxon().converter(setLastSyncedConverter).parse<Result<Unit, SetLastSyncedError>>(setLastSynced("", 0)).unwrapErrorType<SetLastSyncedError.Unexpected>()
    }
}
