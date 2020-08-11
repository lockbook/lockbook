using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Threading.Tasks;
using Windows.Data.Json;
using Windows.UI.Popups;

namespace lockbook {
    class Core {
        static String path = Windows.Storage.ApplicationData.Current.LocalFolder.Path;

        [DllImport("lockbook_core.dll")]
        private static extern System.IntPtr create_account(string path, string username);

        [DllImport("lockbook_core.dll")]
        private unsafe static extern void release_pointer(System.IntPtr str_pointer);

        private static String getStringAndRelease(System.IntPtr pointer) {
            String temp_string = Marshal.PtrToStringAnsi(pointer);
            String result = (String)temp_string.Clone();
            release_pointer(pointer);
            return result;
        }

        public enum CreateAccountResult {
            Success,
            UsernameTaken,
            InvalidUsername,
            CouldNotReachServer,
            AccountExistsAlready,
            ContractError,
            UnexpectedError,
        }

        public static async Task<(CreateAccountResult, String)> CreateAccount(String username) {
            return await Task.Run(() => {
                string result = getStringAndRelease(create_account(path, username));

                JObject obj = JObject.Parse(result);
                
                JToken unexpectedError = obj.SelectToken("Err.UnexpectedError", errorWhenNoMatch: false);
                JToken expectedError = obj.SelectToken("Err", errorWhenNoMatch: false);
                JToken ok = obj.SelectToken("Ok", errorWhenNoMatch: false);

                if (unexpectedError != null) {
                    return (CreateAccountResult.UnexpectedError, result);
                }

                if (expectedError != null) {
                    switch (expectedError.ToString()) {
                        case "InvalidUsername":
                            return (CreateAccountResult.InvalidUsername, null);
                        case "UsernameTaken":
                            return (CreateAccountResult.UsernameTaken, null);
                        case "CouldNotReachServer":
                            return (CreateAccountResult.CouldNotReachServer, null);
                        case "AccountExistsAlready":
                            return (CreateAccountResult.AccountExistsAlready, null);
                    }
                }

                if (ok != null) {
                    return (CreateAccountResult.Success, null);
                }

                return (CreateAccountResult.ContractError, null);
            });
        }
    }
}

// Unexpected Error:
// {"Err":{"UnexpectedError":"Could not connect to db, config: Config {\n    writeable_path: \"\",\n}, error: SledError(\n    Io(\n        Os {\n            code: 5,\n            kind: PermissionDenied,\n            message: \"Access is denied.\",\n        },\n    ),\n)"}}

// Expected Error:
// {"Err":"InvalidUsername"}

// OK
// {"Ok":null}
