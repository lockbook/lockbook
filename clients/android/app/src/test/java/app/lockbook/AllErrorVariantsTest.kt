package app.lockbook

import app.lockbook.core.getAllErrorVariants
import app.lockbook.util.Config
import com.beust.klaxon.Klaxon
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class AllErrorVariantsTest {
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
    fun checkIfAllErrorsPresent() {
        Klaxon().converter(checkIfAllErrorsPresentConverter).parse<Unit>(getAllErrorVariants())
    }
}
