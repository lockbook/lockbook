package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ExportAccountTest {
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
    fun exportAccountOk() {
        assertType<Unit>(
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        assertType<String>(
            CoreModel.exportAccount(config).component1()
        )
    }

    @Test
    fun exportAccountNoAccount() {
        assertType<CoreError.NoAccount>(
            CoreModel.exportAccount(config).component2()
        )
    }

    @Test
    fun exportAccountUnexpectedError() {
        val exportAccountResult: Result<String, CoreError>? =
            Klaxon().converter(exportAccountConverter)
                .parse(exportAccount(""))

        assertType<CoreError.Unexpected>(
            exportAccountResult?.component2()
        )
    }
}
