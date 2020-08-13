using Newtonsoft.Json.Linq;
using System;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace lockbook {
    class CoreService {
        static String path = Windows.Storage.ApplicationData.Current.LocalFolder.Path;

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr create_account(string path, string username);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr get_account(string path);

        [DllImport("lockbook_core.dll")]
        private static extern IntPtr import_account(string path, string account_string);

        [DllImport("lockbook_core.dll")]
        private unsafe static extern void release_pointer(IntPtr str_pointer);

        private static String getStringAndRelease(IntPtr pointer) {
            String temp_string = Marshal.PtrToStringAnsi(pointer);
            String result = (String)temp_string.Clone();
            release_pointer(pointer);
            return result;
        }

        public static async Task<Core.CreateAccount.Result> CreateAccount(String username) {
            String result = await Task.Run(() => getStringAndRelease(create_account(path, username)));

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.CreateAccount.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "InvalidUsername":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.InvalidUsername
                        };
                    case "UsernameTaken":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.UsernameTaken
                        };
                    case "CouldNotReachServer":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.CouldNotReachServer
                        };
                    case "AccountExistsAlready":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.AccountExistsAlready
                        };
                }
            }

            if (ok != null) {
                return new Core.CreateAccount.Success { };
            }

            return new Core.CreateAccount.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }

        public static bool AccountExists() {
            String result = getStringAndRelease(get_account(path));
            JObject obj = JObject.Parse(result);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);
            return ok != null;
        }

        public static async Task<Core.GetAccount.Result> GetAccount() {
            String result = await Task.Run(() => getStringAndRelease(get_account(path)));


            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.GetAccount.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "NoAccount":
                        return new Core.GetAccount.ExpectedError {
                            error = Core.GetAccount.PossibleErrors.NoAccount
                        };
                };
            }

            if (ok != null) {
                return new Core.GetAccount.Success {
                    accountJson = ok.ToString()
                };
            }

            return new Core.GetAccount.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }
        public static async Task<Core.CreateAccount.Result> ImportAccount(String account_string) {
            String result = await Task.Run(() => getStringAndRelease(import_account(path, account_string)));

            JObject obj = JObject.Parse(result);

            JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
            JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
            JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

            if (unexpectedError != null) {
                return new Core.CreateAccount.UnexpectedError {
                    errorMessage = result
                };
            }

            if (expectedError != null) {
                switch (expectedError.ToString()) {
                    case "InvalidUsername":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.InvalidUsername
                        };
                    case "UsernameTaken":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.UsernameTaken
                        };
                    case "CouldNotReachServer":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.CouldNotReachServer
                        };
                    case "AccountExistsAlready":
                        return new Core.CreateAccount.ExpectedError {
                            error = Core.CreateAccount.PossibleErrors.AccountExistsAlready
                        };
                }
            }

            if (ok != null) {
                return new Core.CreateAccount.Success { };
            }

            return new Core.CreateAccount.UnexpectedError {
                errorMessage = "Contract error!"
            };
        }
    }
}
