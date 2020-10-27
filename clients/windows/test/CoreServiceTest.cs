using Core;
using lockbook;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;
using System.IO;

namespace test {
    [TestClass]
    public class CoreServiceTest {
        const string lockbookDir = "C:\\Temp\\.lockbook"; // todo: find a more suitable location
        public CoreService CoreService {
            get { return new CoreService(lockbookDir); }
        }

        // todo: test helper to assert status and print things otherwise


        public string RandomUsername() {
            return "testUsername" + Guid.NewGuid().ToString().Replace("-", "");
        }

        [TestInitialize]
        public void DeleteAccount() {
            try {
                Directory.Delete(lockbookDir, true);
            } catch (DirectoryNotFoundException) { }
        }

        [TestMethod]
        public void AccountExistsFalse() {
            Assert.IsFalse(CoreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void AccountExistsTrue() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());
            var getAccountResult = CoreService.GetAccount().WaitResult();
            Assert.AreEqual(typeof(Core.GetAccount.Success), getAccountResult.GetType());
            Assert.IsTrue(CoreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void CreateAccountSuccess() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());
        }

        [TestMethod]
        public void CreateAccountAccountExistsAlready() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // create another account
            var username2 = RandomUsername();
            var createAccountResult2 = CoreService.CreateAccount(username2).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.ExpectedError), createAccountResult2.GetType());
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.AccountExistsAlready,
                ((Core.CreateAccount.ExpectedError)createAccountResult2).Error);
        }

        [TestMethod]
        public void CreateAccountUsernameTaken() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // sync account to the server
            var syncResult = CoreService.SyncAll().WaitResult();
            Assert.AreEqual(typeof(Core.SyncAll.Success), syncResult.GetType());

            // delete directory to avoid AccountExistsAlready
            Directory.Delete(lockbookDir, true);

            // create account with the same name
            var createAccountResult2 = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.ExpectedError), createAccountResult2.GetType());
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.UsernameTaken,
                ((Core.CreateAccount.ExpectedError)createAccountResult2).Error);
        }

        [TestMethod]
        public void CreateAccountInvalidUsername() {
            var username = "not! a! valid! username!";
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.ExpectedError), createAccountResult.GetType());
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.InvalidUsername,
                ((Core.CreateAccount.ExpectedError)createAccountResult).Error);
        }

        [TestMethod]
        public void GetAccount() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // get account
            var getAccountResult = CoreService.GetAccount().WaitResult();
            Assert.AreEqual(typeof(Core.GetAccount.Success), getAccountResult.GetType());
        }

        [TestMethod]
        public void GetAccountNoAccount() {
            // get account
            var getAccountResult = CoreService.GetAccount().WaitResult();
            Assert.AreEqual(typeof(Core.GetAccount.ExpectedError), getAccountResult.GetType());
            Assert.AreEqual(Core.GetAccount.PossibleErrors.NoAccount,
               ((Core.GetAccount.ExpectedError)getAccountResult).Error);
        }
        [TestMethod]
        public void ImportAccount() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // export account string
            var accountString = "TODO";

            // delete directory to avoid AccountExistsAlready
            Directory.Delete(lockbookDir, true);

            // import account via string
            var importAccountResult = CoreService.ImportAccount(accountString).WaitResult();
            Assert.AreEqual(typeof(Core.ImportAccount.Success), importAccountResult.GetType());
        }

        [TestMethod]
        public void ImportAccountAccountStringCorrupted() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // export account string
            var accountString = "#######!!@$@%";

            // delete directory to avoid AccountExistsAlready
            Directory.Delete(lockbookDir, true);

            // import account via string
            var importAccountResult = CoreService.ImportAccount(accountString).WaitResult();
            Assert.AreEqual(typeof(Core.ImportAccount.ExpectedError), importAccountResult.GetType());
            Assert.AreEqual(Core.ImportAccount.PossibleErrors.AccountStringCorrupted,
                ((Core.ImportAccount.ExpectedError)importAccountResult).Error);

        }

        [TestMethod]
        public void ListMetadatas() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // list file metadata
            var listFileMetadataResult = CoreService.ListMetadatas().WaitResult();
            Assert.AreEqual(typeof(Core.ListMetadatas.Success), listFileMetadataResult.GetType());
        }

        [TestMethod]
        public void SyncAll() {
            // this one will be tricky let's tackle it later
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            CoreService.SyncAll().WaitResult();
            var syncAllResult = CoreService.SyncAll().WaitResult();
            Assert.AreEqual(typeof(Core.SyncAll.Success), syncAllResult.GetType());
        }

        [TestMethod]
        public void SyncAllNoAccount() {

            CoreService.SyncAll().WaitResult();
            var syncAllResult = CoreService.SyncAll().WaitResult();
            Assert.AreEqual(typeof(Core.SyncAll.ExpectedError), syncAllResult.GetType());
            Assert.AreEqual(Core.SyncAll.PossibleErrors.NoAccount,
                ((Core.SyncAll.ExpectedError)syncAllResult).Error);
        }

        [TestMethod]
        public void CreateFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
        }

        [TestMethod]
        public void CreateFileNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;
            DeleteAccount();

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.ExpectedError), createFileResult.GetType());
            Assert.AreEqual(Core.CreateFile.PossibleErrors.NoAccount,
                ((Core.CreateFile.ExpectedError)createFileResult).Error);
        }

        [TestMethod]
        public void CreateFileDocTreatedAsFolder() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var createFileResult2 = CoreService.CreateFile("TestFile", fileId, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.ExpectedError), createFileResult2.GetType());
            Assert.AreEqual(Core.CreateFile.PossibleErrors.DocumentTreatedAsFolder,
                ((Core.CreateFile.ExpectedError)createFileResult2).Error);
        }

        [TestMethod]
        public void WriteDoc() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult();  // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var writeDocResult = CoreService.WriteDocument(fileId, "content").WaitResult();
            Assert.AreEqual(typeof(Core.WriteDocument.Success), writeDocResult.GetType());
        }

        [TestMethod]
        public void WriteDocNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult();  // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;
            DeleteAccount();

            var writeDocResult = CoreService.WriteDocument(fileId, "content").WaitResult();
            Assert.AreEqual(typeof(Core.WriteDocument.ExpectedError), writeDocResult.GetType());
            Assert.AreEqual(Core.WriteDocument.PossibleErrors.NoAccount,
                ((Core.WriteDocument.ExpectedError)writeDocResult).Error);
        }

        [TestMethod]
        public void ReadDoc() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult(); // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var readDocResult = CoreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual(typeof(Core.ReadDocument.Success), readDocResult.GetType());
        }

        [TestMethod]
        public void ReadDocNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult();  // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;
            DeleteAccount();

            var readDocResult = CoreService.ReadDocument(fileId).WaitResult();
            Assert.AreEqual(typeof(Core.ReadDocument.ExpectedError), readDocResult.GetType());
            Assert.AreEqual(Core.ReadDocument.PossibleErrors.NoAccount,
                ((Core.ReadDocument.ExpectedError)readDocResult).Error);
        }

        [TestMethod]
        public void RenameFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult(); // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var renameFileResult = CoreService.RenameFile(fileId, "NewTestFile").WaitResult();
            Assert.AreEqual(typeof(Core.RenameFile.Success), renameFileResult.GetType());
        }

        [TestMethod]
        public void MoveFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult(); // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var createFolderResult = CoreService.CreateFile("TestFile2", id, FileType.Folder).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFolderResult.GetType());
            var folderId = ((Core.CreateFile.Success)createFolderResult).newFile.Id;

            var moveFileResult = CoreService.MoveFile(fileId, folderId).WaitResult();
            Assert.AreEqual(typeof(Core.MoveFile.Success), moveFileResult.GetType());
        }

        [TestMethod]
        public void MoveFileNoAccount() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var getRootResult = CoreService.GetRoot().WaitResult();
            Assert.AreEqual(typeof(Core.GetRoot.Success), getRootResult.GetType());
            var id = ((Core.GetRoot.Success)getRootResult).root.Id;

            var createFileResult = CoreService.CreateFile("TestFile", id, FileType.Document).WaitResult(); // TODO: get root and use id instead of username
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var fileId = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var createFolderResult = CoreService.CreateFile("TestFile2", id, FileType.Folder).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFolderResult.GetType());
            var folderId = ((Core.CreateFile.Success)createFolderResult).newFile.Id;
            DeleteAccount();

            var moveFileResult = CoreService.MoveFile(fileId, folderId).WaitResult();
            Assert.AreEqual(typeof(Core.MoveFile.ExpectedError), moveFileResult.GetType());
            Assert.AreEqual(Core.MoveFile.PossibleErrors.NoAccount,
                ((Core.MoveFile.ExpectedError)moveFileResult).Error);
        }
    }
}