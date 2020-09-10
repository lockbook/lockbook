package app.lockbook

import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.InitLoggerError
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class InitLoggerTest {
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
    fun initLoggerOk() {
        assertType<Unit>(
            CoreModel.setUpInitLogger(config.writeable_path).component1()
        )
    }

    @Test
    fun initLoggerUnexpected() {
        assertType<InitLoggerError.Unexpected>(
            CoreModel.setUpInitLogger("${config.writeable_path}/${generateAlphaString()}.txt").component2()
        )
    }
}
