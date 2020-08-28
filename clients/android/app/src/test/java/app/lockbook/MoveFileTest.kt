package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.core.moveFile
import app.lockbook.utils.*
import com.beust.klaxon.Klaxon
import com.github.michaelbull.result.Result
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test
import java.util.*

class MoveFileTest {
    var path = createRandomPath()

    companion object {
        @BeforeClass
        @JvmStatic
        fun loadLib() {
            loadLockbookCore()
        }
    }

    @After
    fun createDirectory() {
        path = createRandomPath()
    }

    @Test
    fun moveFileOk() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        coreModel.moveFile(document.id, folder.id).component1()!!
    }

    @Test
    fun moveFileDoesNotExist() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val moveFileError = coreModel.moveFile(generateId(), folder.id).component2()!!
        require(moveFileError is MoveFileError.FileDoesNotExist)
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val moveFileError = coreModel.moveFile(folder.id, document.id).component2()!!
        require(moveFileError is MoveFileError.DocumentTreatedAsFolder)
    }

    @Test
    fun moveFileTargetParentDoesNotExist() {
        val coreModel = CoreModel(Config(path))
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val moveFileError = coreModel.moveFile(document.id, generateId()).component2()!!
        require(moveFileError is MoveFileError.TargetParentDoesNotExist)
    }

    @Test
    fun moveFIleTargetParentHasChildNamedThat() {
        val coreModel = CoreModel(Config(path))
        val documentName = generateAlphaString()
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        coreModel.parentFileMetadata = folder
        val firstDocument = coreModel.createFile(documentName, Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(firstDocument).component1()!!
        coreModel.setParentToRoot()
        val secondDocument = coreModel.createFile(documentName, Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(secondDocument).component1()!!
        val moveFileError = coreModel.moveFile(secondDocument.id, folder.id).component2()!!
        require(moveFileError is MoveFileError.TargetParentHasChildNamedThat)
    }

    @Test
    fun moveFileUnexpectedError() {
        val moveResult: Result<Unit, MoveFileError>? =
            Klaxon().converter(moveFileConverter).parse(moveFile("", "", ""))
        val moveError = moveResult!!.component2()!!
        require(moveError is MoveFileError.UnexpectedError)
    }
}
