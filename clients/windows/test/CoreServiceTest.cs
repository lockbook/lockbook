using lockbook;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;
using System.IO;
using System.Threading.Tasks;

namespace test {
    public static class Extensions {
        public static T WaitResult<T>(this Task<T> task) {
            task.Wait();
            return task.Result;
        }
    }

    [TestClass]
    public class CoreServiceTest {
        const string lockbookDir = "C:\\Temp\\.lockbook"; // todo: find a more suitable location
        public CoreService CoreService {
            get { return new CoreService(lockbookDir); }
        }

        public string RandomUsername() {
            return "testUsername" + Guid.NewGuid().ToString().Replace("-", "");
        }

        [TestInitialize]
        public void Init() {
            try {
                Directory.Delete(lockbookDir, true);
            }
            catch (System.IO.DirectoryNotFoundException e) { }
        }

        [TestMethod]
        public void AccountExistsFalse() {
            Assert.IsFalse(CoreService.AccountExists());
        }

        [TestMethod]
        public void AccountExistsTrue() {
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            //Console.WriteLine(((Core.CreateAccount.UnexpectedError)createAccountResult).errorMessage);
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());
            Assert.IsTrue(CoreService.AccountExists());
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
                ((Core.CreateAccount.ExpectedError)createAccountResult2).error);
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
                ((Core.CreateAccount.ExpectedError)createAccountResult2).error);
        }

        [TestMethod]
        public void CreateAccountInvalidUsername() {
            var username = "not! a! valid! username!";
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.ExpectedError), createAccountResult.GetType());
            Assert.AreEqual(Core.CreateAccount.PossibleErrors.InvalidUsername,
                ((Core.CreateAccount.ExpectedError)createAccountResult).error);
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
            Assert.AreEqual(typeof(Core.ImportAccount.Success), importAccountResult.GetType());
        }

        [TestMethod]
        public void ListFileMetadata() {
            // create account
            var username = RandomUsername();
            var createAccountResult = CoreService.CreateAccount(username).WaitResult();
            Assert.AreEqual(typeof(Core.CreateAccount.Success), createAccountResult.GetType());

            // list file metadata
            var listFileMetadataResult = CoreService.ListFileMetadata().WaitResult();
            Assert.AreEqual(typeof(Core.ListFileMetadata.Success), listFileMetadataResult.GetType());
        }

        [TestMethod]
        public void SyncAll() {
            // this one will be tricky let's tackle it later
        }

        [TestMethod]
        public void CreateFileNoAccount() {
            Assert.IsFalse(CoreService.AccountExists());
        }
    }
}