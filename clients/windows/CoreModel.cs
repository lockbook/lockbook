﻿using Newtonsoft.Json;
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
}

