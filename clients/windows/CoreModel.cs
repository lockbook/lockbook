using Newtonsoft.Json;
using System;
using System.Collections.Generic;

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

    namespace GetAccount {
        abstract class Result { }

        class Success : Result {
            public String accountJson;
        }

        public enum PossibleErrors {
            NoAccount
        }

        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace ImportAccount {
        abstract class Result { }

        class Success : Result { }

        public enum PossibleErrors {
            AccountStringCorrupted,
            AccountExistsAlready,
            AccountDoesNotExist,
            UsernamePKMismatch,
            CouldNotReachServer
        }

        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    class FileMetadata {
        [JsonProperty("id")]
        public String Id { get; set; }

        [JsonProperty("name")]
        public String Name { get; set; }

        [JsonProperty("parent")]
        public String Parent { get; set; }

        [JsonProperty("file_type")]
        public String Type { get; set; }
    }

    class DecryptedValue {
        [JsonProperty("secret")]
        public String secret { get; set; }
    }

    public enum FileType {
        Folder,
        Document
    }

    namespace ListFileMetadata {
        abstract class Result { }

        class Success : Result {
            public List<FileMetadata> files;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace CreateFile {
        abstract class Result { }

        class Success : Result {
            public FileMetadata NewFile { get; set; }
        }

        public enum PossibleErrors {
            NoAccount,
            DocumentTreatedAsFolder,
            CouldNotFindAParent,
            FileNameNotAvailable,
            FileNameContainsSlash,
            FileNameEmpty,
        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }


        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace SyncAll {
        abstract class Result { }

        class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ExecuteWorkError
        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace ReadDocument {
        abstract class Result { }

        class Success : Result {
            public DecryptedValue content;
        }

        public enum PossibleErrors {
            NoAccount,
            TreatedFolderAsDocument,
            FileDoesNotExist
        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace WriteDocument {
        abstract class Result { }

        class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            TreatedFolderAsDocument,
            FileDoesNotExist
        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace RenameFile {
        abstract class Result { }

        class Success : Result { }

        public enum PossibleErrors {
            FileDoesNotExist,
            NewNameContainsSlash,
            FileNameNotAvailable,
            NewNameEmpty,
            CannotRenameRoot
        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace MoveFile {
        abstract class Result { }

        class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            FileDoesNotExist,
            DocumentTreatedAsFolder,
            TargetParentHasChildNamedThat,
            TargetParentDoesNotExist,
            CannotMoveRoot,

        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace CalculateWork {
        class WorkCalculated {
            [JsonProperty("work_units")]
            public List<dynamic> WorkUnits { get; set; }

            [JsonProperty("most_recent_update_from_server")]
            public UInt64 MostRecentUpdateFromServer { get; set; }
        }

        abstract class Result { }

        class Success : Result {
            public WorkCalculated workCalculated;
        }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer

        }
        class ExpectedError : Result {
            public PossibleErrors error;
        }

        class UnexpectedError : Result {
            public String errorMessage;
        }
    }
}

