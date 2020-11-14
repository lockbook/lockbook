package app.lockbook

import app.lockbook.util.Config
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
    fun test() {
        getErrorVariants()
    }
}