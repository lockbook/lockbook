using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Windows.Data.Json;
using Windows.UI.Popups;
using Windows.Web.AtomPub;

namespace lockbook {
    class CoreService {
        static String path = Windows.Storage.ApplicationData.Current.LocalFolder.Path;

        [DllImport("lockbook_core.dll")]
        private static extern System.IntPtr create_account(string path, string username);

        [DllImport("lockbook_core.dll")]
        private static extern System.IntPtr get_account(string path);


        [DllImport("lockbook_core.dll")]
        private unsafe static extern void release_pointer(System.IntPtr str_pointer);

        private static String getStringAndRelease(System.IntPtr pointer) {
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
    }
}

// Unexpected Error:
// {"Err":{"UnexpectedError":"Could not connect to db, config: Config {\n    writeable_path: \"\",\n}, error: SledError(\n    Io(\n        Os {\n            code: 5,\n            kind: PermissionDenied,\n            message: \"Access is denied.\",\n        },\n    ),\n)"}}

// Expected Error:
// {"Err":"InvalidUsername"}

// OK
// {"Ok":null}
