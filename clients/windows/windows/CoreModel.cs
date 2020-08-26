using Newtonsoft.Json;
using System;
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
            public String errorMessage;
        }
    }

    namespace GetAccount {
        public abstract class Result { }

        public class Success : Result {
            public String accountJson;
        }

        public enum PossibleErrors {
            NoAccount
        }

        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public String errorMessage;
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
            public String errorMessage;
        }
    }

    public class FileMetadata {
        [JsonProperty("id")]
        public String Id { get; set; }

        [JsonProperty("name")]
        public String Name { get; set; }

        [JsonProperty("parent")]
        public String Parent { get; set; }

        [JsonProperty("file_type")]
        public String Type { get; set; }
    }

    public class DecryptedValue {
        [JsonProperty("secret")]
        public String secret { get; set; }
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
            public String errorMessage;
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
            FileNameContainsSlash
        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }


        public class UnexpectedError : Result {
            public String errorMessage;
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
            public String errorMessage;
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
            public String errorMessage;
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
            public String errorMessage;
        }
    }

    namespace RenameFile {
        public abstract class Result { }

        public class Success : Result { }

        public enum PossibleErrors {
            FileDoesNotExist,
            NewNameContainsSlash,
            FileNameNotAvailable,
        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public String errorMessage;
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

        }
        public class ExpectedError : Result {
            public PossibleErrors error;
        }

        public class UnexpectedError : Result {
            public String errorMessage;
        }
    }

    namespace CalculateWork {
        public class WorkCalculated {
            [JsonProperty("work_units")]
            public List<dynamic> WorkUnits { get; set; }

            [JsonProperty("most_recent_update_from_server")]
            public UInt64 MostRecentUpdateFromServer { get; set; }
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
            public String errorMessage;
        }
    }
}

