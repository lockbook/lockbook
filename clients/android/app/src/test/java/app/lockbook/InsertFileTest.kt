package app.lockbook

import app.lockbook.core.insertFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class InsertFileTest {
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
    fun insertFileOk() {
        assertType<Unit>(
            this::insertFileOk.name,
            CoreModel.generateAccount(config, generateAlphaString()).component1()
        )

        val rootFileMetadata = assertTypeReturn<FileMetadata>(
            this::insertFileOk.name,
            CoreModel.getRoot(config).component1()
        )

        val document = assertTypeReturn<FileMetadata>(
            this::insertFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Document)
            ).component1()
        )

        val folder = assertTypeReturn<FileMetadata>(
            this::insertFileOk.name,
            CoreModel.createFile(
                config,
                rootFileMetadata.id,
                generateAlphaString(),
                Klaxon().toJsonString(FileType.Folder)
            ).component1()
        )

        assertType<Unit>(
            this::insertFileOk.name,
            CoreModel.insertFile(config, document).component1()
        )

        assertType<Unit>(
            this::insertFileOk.name,
            CoreModel.insertFile(config, folder).component1()
        )
    }

    @Test
    fun insertFileError() {
        val insertResult: Result<Unit, InsertFileError>? =
            Klaxon().converter(insertFileConverter)
                .parse(insertFile("", ""))

        assertType<InsertFileError.UnexpectedError>(
            this::insertFileError.name,
            insertResult?.component2()
        )
    }
}
