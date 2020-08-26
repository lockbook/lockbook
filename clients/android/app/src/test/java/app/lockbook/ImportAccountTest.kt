package app.lockbook

import app.lockbook.core.importAccount
import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.ImportError
import app.lockbook.utils.importAccountConverter
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
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
            Runtime.getRuntime().exec("rm -rf $path")
        }
    }

    @Before
    fun createDirectory() {
        Runtime.getRuntime().exec("mkdir $path")
    }

    @After
    fun resetDirectory() {
        Runtime.getRuntime().exec("rm -rf $path")
    }

    @Test
    fun importAccountOk() {
        CoreModel.generateAccount(Config(path), generateAlphaString()).component1()!!
        val exportAccountString = CoreModel.exportAccount(Config(path)).component1()!!
        Runtime.getRuntime().exec("rm -rf $path")
        Runtime.getRuntime().exec("mkdir $path")

        CoreModel.importAccount(Config(path), exportAccountString).component1()!!
    }

    @Test
    fun importAccountStringCorrupted() {
        val firstImportAccountError = CoreModel.importAccount(
            Config(path),
            "!@#$%^&*()"
        ).component2()!!
        require(firstImportAccountError is ImportError.AccountStringCorrupted)
        val secondImportAccountError = CoreModel.importAccount(
            Config(path),
            "œ∑´´†¥¨ˆˆπåß∂ƒ"
        ).component2()!!
        require(secondImportAccountError is ImportError.AccountStringCorrupted)
        val thirdImportAccountError = CoreModel.importAccount(
            Config(path),
            "Ω≈ç√∫˜˜¬˚∆˙©"
        ).component2()!!
        require(thirdImportAccountError is ImportError.AccountStringCorrupted)
        val fourthImportAccountError = CoreModel.importAccount(
            Config(path),
            "☺️☠️✋☝️✊"
        ).component2()!!
        require(fourthImportAccountError is ImportError.AccountStringCorrupted)
    }

    @Test
    fun importAccountUnexpectedError() {
        val importResult: Result<Unit, ImportError>? =
            Klaxon().converter(importAccountConverter)
                .parse(importAccount("", ""))
        val importError = importResult!!.component2()!!
        require(importError is ImportError.UnexpectedError)
    }
}
