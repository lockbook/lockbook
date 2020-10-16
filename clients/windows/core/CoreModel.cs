using Newtonsoft.Json;
using System.Collections.Generic;

namespace Core {
    namespace CreateAccount {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            UsernameTaken,
            InvalidUsername,
            CouldNotReachServer,
            AccountExistsAlready,
        }

        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace GetAccount {
        public abstract class Result { }

        public class Success : Result {
            public string accountJson;
        }

        public enum PossibleErrors {
            NoAccount
        }

        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace ImportAccount {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            AccountStringCorrupted,
            AccountExistsAlready,
            AccountDoesNotExist,
            UsernamePKMismatch,
            CouldNotReachServer
        }

        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    public class FileMetadata {
        [JsonProperty("id")]
        public string Id { get; set; }

        [JsonProperty("name")]
        public string Name { get; set; }

        [JsonProperty("parent")]
        public string Parent { get; set; }

        [JsonProperty("file_type")]
        public string Type { get; set; }
    }

    public class DecryptedValue {
        [JsonProperty("secret")]
        public string secret { get; set; }
    }

    public enum FileType {
        Folder,
        Document
    }

    namespace ListFileMetadata {
        public abstract class Result { }

        public class Success : Result {
            public List<FileMetadata> files;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace CreateFile {
        public abstract class Result { }

        public class Success : Result {
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
        public class ExpectedError : Result {
            public PossibleErrors error;
        }


        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace SyncAll {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer,
            ExecuteWorkError
        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace ReadDocument {
        public abstract class Result { }

        public class Success : Result {
            public DecryptedValue content;
        }

        public enum PossibleErrors {
            NoAccount,
            TreatedFolderAsDocument,
            FileDoesNotExist
        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace WriteDocument {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            TreatedFolderAsDocument,
            FileDoesNotExist
        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace RenameFile {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            FileDoesNotExist,
            NewNameContainsSlash,
            FileNameNotAvailable,
            NewNameEmpty,
            CannotRenameRoot
        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace MoveFile {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            NoAccount,
            FileDoesNotExist,
            DocumentTreatedAsFolder,
            TargetParentHasChildNamedThat,
            TargetParentDoesNotExist,
            CannotMoveRoot,

        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }

    namespace CalculateWork {
        public class WorkCalculated {
            [JsonProperty("work_units")]
            public List<dynamic> WorkUnits { get; set; }

            [JsonProperty("most_recent_update_from_server")]
            public ulong MostRecentUpdateFromServer { get; set; }
        }

        public abstract class Result { }

        public class Success : Result {
            public WorkCalculated workCalculated;
        }

        public enum PossibleErrors {
            NoAccount,
            CouldNotReachServer

        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public string errorMessage;
        }
    }
}

