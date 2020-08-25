package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.AccountExportError
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test

class ExportAccountTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
            Runtime.getRuntime().exec("mkdir $path")
        }
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path/*")
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
        require(exportAccountError is AccountExportError.NoAccount)
    }
}