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
        public void Init() {
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
        public void ListFileMetadata() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // list file metadata
            var listFileMetadataResult = CoreService.ListFileMetadata().WaitResult();
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

            var createFileResult = CoreService.CreateFile("TestFile", username, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
        }

        [TestMethod]
        public void CreateFileNoAccount() {
            //create file

            Assert.IsFalse(CoreService.AccountExists().WaitResult());
        }

        [TestMethod]
        public void WriteDoc() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var createFileResult = CoreService.CreateFile("TestFile", username, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var id = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var writeDocResult = CoreService.WriteDocument(id, "content").WaitResult();
            Assert.AreEqual(typeof(Core.WriteDocument.Success), writeDocResult.GetType());
        }

        [TestMethod]
        public void ReadDoc() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var createFileResult = CoreService.CreateFile("TestFile", username, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var id = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var readDocResult = CoreService.ReadDocument(id).WaitResult();
            Assert.AreEqual(typeof(Core.ReadDocument.Success), readDocResult.GetType());
        }

        [TestMethod]
        public void RenameFile() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            var createFileResult = CoreService.CreateFile("TestFile", username, FileType.Document).WaitResult();
            Assert.AreEqual(typeof(Core.CreateFile.Success), createFileResult.GetType());
            var id = ((Core.CreateFile.Success)createFileResult).newFile.Id;

            var renameFileResult = CoreService.RenameFile(id, "NewTestFile").WaitResult();
            Assert.AreEqual(typeof(Core.RenameFile.Success), renameFileResult.GetType());
        }


    }
}