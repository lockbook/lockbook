package app.lockbook

import app.lockbook.core.exportAccount
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class ExportAccountTest {

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
    fun exportAccountOk() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        CoreModel.exportAccount(Config(path)).component1()!!
    }

    @Test
    fun exportAccountNoAccount() {
        val exportAccountError = CoreModel.exportAccount(Config(path)).component2()!!
        require(exportAccountError is AccountExportError.NoAccount) {
            "${Klaxon().toJsonString(exportAccountError)} != ${AccountExportError.NoAccount::class.qualifiedName}"
        }
    }

    @Test
    fun exportAccountUnexpectedError() {
        val exportAccountResult: Result<String, AccountExportError>? =
            Klaxon().converter(exportAccountConverter)
                .parse(exportAccount(""))
        val exportAccountError = exportAccountResult!!.component2()!!
        require(exportAccountError is AccountExportError.UnexpectedError) {
            "${Klaxon().toJsonString(exportAccountError)} != ${AccountExportError.UnexpectedError::class.qualifiedName}"
        }
    }
}
