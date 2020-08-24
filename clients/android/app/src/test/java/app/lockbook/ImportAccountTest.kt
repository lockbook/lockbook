package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.ImportError
import org.junit.Before
import org.junit.Test


class ImportAccountTest {

    @Before
    fun loadLib() {
        loadLockbookCore()
    }

    @Test
    fun importAccount() {
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