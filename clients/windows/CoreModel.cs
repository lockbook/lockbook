using System;

namespace Core {
    namespace CreateAccount {
        abstract class Result { }

        class Success : Result { }

        public enum PossibleErrors {
            UsernameTaken,
            InvalidUsername,
            CouldNotReachServer,
            AccountExistsAlready,
        }

        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }
}

// Unexpected Error:
// {"Err":{"UnexpectedError":"Could not connect to db, config: Config {\n    writeable_path: \"\",\n}, error: SledError(\n    Io(\n        Os {\n            code: 5,\n            kind: PermissionDenied,\n            message: \"Access is denied.\",\n        },\n    ),\n)"}}

// Expected Error:
// {"Err":"InvalidUsername"}

// OK
// {"Ok":null}
