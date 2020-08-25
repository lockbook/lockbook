package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.ImportError
import org.junit.After
import org.junit.Before
import org.junit.BeforeClass
import org.junit.Test


class ImportAccountTest {

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
    fun importAccountOk() {
        CoreModel.generateAccount(Config(path), generateAlphaString()).component1()!!
        val exportAccountString = CoreModel.exportAccount(Config(path)).component1()!!
        CoreModel.importAccount(Config(path), exportAccountString).component1()!!
    }

    @Test
    fun importAccountStringCorrupted() {
        val importAccountError =
            CoreModel.importAccount(Config(path), "!@#$%^&*()").component2()!!
        require(importAccountError is ImportError.AccountStringCorrupted)
    }
}