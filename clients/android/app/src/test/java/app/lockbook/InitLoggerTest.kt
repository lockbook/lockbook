package app.lockbook

import app.lockbook.model.CoreModel
import app.lockbook.util.Config
import app.lockbook.util.InitLoggerError
import com.github.michaelbull.result.unwrap
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
        CoreModel.setUpInitLogger(config.writeable_path).unwrap()
    }

    @Test
    fun initLoggerUnexpected() {
        CoreModel.setUpInitLogger("${config.writeable_path}/${generateAlphaString()}.txt")
            .unwrapErrorType<InitLoggerError.Unexpected>()
    }
}
