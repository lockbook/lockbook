package app.lockbook

import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.FileMetadata
import app.lockbook.utils.State
import org.junit.After
import org.junit.Before
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
    fun getDBStateOkEmpty() {
        assertEnumType(
            CoreModel.getDBState(config).component1(),
            State.Empty
        )
    }

    @Test
    fun getDBStateOkReadyToUse() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertEnumType(
            CoreModel.getDBState(config).component1(),
            State.ReadyToUse
        )
    }
}