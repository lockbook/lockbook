package app.lockbook

import app.lockbook.core.loadLockbookCore
import app.lockbook.utils.Config
import app.lockbook.utils.CoreModel
import app.lockbook.utils.FileType
import app.lockbook.utils.MoveFileError
import com.beust.klaxon.Klaxon
import org.junit.After
import org.junit.BeforeClass
import org.junit.Test

class MoveFileTest {
    private val coreModel = CoreModel(Config(path))

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
    fun moveFileOk() {
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
    fun moveFileNoAccount() {
        val moveFileError = coreModel.moveFile(generateAlphaString(), generateAlphaString()).component2()!!
        require(moveFileError is MoveFileError.NoAccount)
    }

    @Test
    fun moveFileDoesNotExist() {
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val folder = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Folder)).component1()!!
        coreModel.insertFile(folder).component1()!!
        val moveFileError = coreModel.moveFile(generateAlphaString(), folder.id).component2()!!
        require(moveFileError is MoveFileError.FileDoesNotExist)
    }

    @Test
    fun moveFileDocumentTreatedAsFolder() {
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
        CoreModel.generateAccount(
            Config(path),
            generateAlphaString()
        ).component1()!!
        coreModel.setParentToRoot().component1()!!
        val document = coreModel.createFile(generateAlphaString(), Klaxon().toJsonString(FileType.Document)).component1()!!
        coreModel.insertFile(document).component1()!!
        val moveFileError = coreModel.moveFile(document.id, generateAlphaString()).component2()!!
        require(moveFileError is MoveFileError.TargetParentDoesNotExist)
    }

    @Test
    fun moveFIleTargetParentHasChildNamedThat() {
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
        coreModel.moveFile(secondDocument.id, folder.id)
        coreModel.moveFile(firstDocument.id, folder.id).component1()!!
    }
}