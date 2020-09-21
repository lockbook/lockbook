using lockbook;
using Microsoft.VisualStudio.TestTools.UnitTesting;
using System;

namespace test {
    [TestClass]
    public class CoreServiceTest {
        private TestContext testContextInstance;
        /// <summary>
        ///  Gets or sets the test context which provides
        ///  information about and functionality for the current test run.
        ///</summary>
        public TestContext TestContext {
            get { return testContextInstance; }
            set { testContextInstance = value; }
        }

        public CoreService CoreService {
            get { return new CoreService("C:\\Temp"); } // todo: find a more suitable location
        }

        public string RandomUsernameHelper() {
            var username = "testUsername" + Guid.NewGuid().ToString().Replace("-", "");
            return username;
        }

        [TestMethod]
        public void AccountExists() {
            Assert.IsTrue(CoreService.AccountExists());
        }

        [TestMethod]
        public void CreateAccountOk() {
            var username = RandomUsernameHelper();
            var task = CoreService.CreateAccount(username);
            task.Wait();
            Assert.AreEqual(System.Threading.Tasks.TaskStatus.RanToCompletion, task.Status);
            Assert.AreEqual(typeof(Core.CreateAccount.Success), task.Result.GetType()); //figure out error with guid and replace "ExpectedError" with "Success"
            var createAccountResult = task.Result;
            switch (createAccountResult) {
                case Core.CreateAccount.Success:
                    break;
                case Core.CreateAccount.UnexpectedError:
                    break;
                case Core.CreateAccount.ExpectedError expectedError:
                    switch (expectedError.error) {
                        case Core.CreateAccount.PossibleErrors.InvalidUsername:
                            Console.WriteLine("invalid");
                            break;
                        case Core.CreateAccount.PossibleErrors.UsernameTaken:
                            Console.WriteLine("taken");
                            break;
                        case Core.CreateAccount.PossibleErrors.CouldNotReachServer:
                            Console.WriteLine("noServer");
                            break;
                        case Core.CreateAccount.PossibleErrors.AccountExistsAlready:
                            Console.WriteLine("accountExists");
                            break;
                    }
                    break;
            }
        }

        [TestMethod]
        public void CreateAccountUsernameTaken() {
            var username = RandomUsernameHelper();
            var task = CoreService.CreateAccount(username);
            task.Wait();
            Assert.AreEqual(System.Threading.Tasks.TaskStatus.RanToCompletion, task.Status);
            Assert.AreEqual(typeof(Core.CreateAccount.ExpectedError), task.Result.GetType());
        }

        [TestMethod]
        public void CreateAccountInvalidUsername() {
            CoreService.CreateAccount("@#$%^&*()");
        }

        [TestMethod]
        public void GetAccount() {
            var task = CoreService.GetAccount();
            task.Wait();
            Assert.AreEqual(System.Threading.Tasks.TaskStatus.RanToCompletion, task.Status);
            Assert.AreEqual(typeof(Core.GetAccount.Success), task.Result.GetType());
        }

        [TestMethod]
        public void ImportAccount() {
            var task = CoreService.ImportAccount(Guid.NewGuid().ToString().Replace("-", ""));
            task.Wait();
            Assert.AreEqual(System.Threading.Tasks.TaskStatus.RanToCompletion, task.Status);
            Assert.AreEqual(typeof(Core.ImportAccount.ExpectedError), task.Result.GetType()); //figure out error with guid and replace "ExpectedError" with "Success"
        }

        [TestMethod]
        public void ListFileMetaData() {
            Assert.IsNotNull(CoreService.ListFileMetadata());//this aint right ill need some help
        }

        [TestMethod]
        public void SyncAll() {// same here idk what to do
        }
    }
}