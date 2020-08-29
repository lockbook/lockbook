package app.lockbook

import app.lockbook.core.setLastSynced
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.SetLastSyncedError
import app.lockbook.utils.setLastSyncedConverter
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class SetLastSyncedTest {

    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            System.loadLibrary("lockbook_core")
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun setLastSyncedOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setLastSynced(1)
    }

    @Test
    fun setLastSyncedUnexpectedError() {
        val lastSyncedResult: Result<Unit, SetLastSyncedError>? =
            Klaxon().converter(setLastSyncedConverter).parse(setLastSynced("", 0))
        val lastSyncedError = lastSyncedResult!!.component2()!!
        require(lastSyncedError is SetLastSyncedError.UnexpectedError)
    }
}
