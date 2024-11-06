import Bridge

public struct LbError: Error {
    public let code: EC
    public let msg: String
    public let trace: String
    
    init(_ err: LbFfiErr) {
        self.code = EC(err.code)
        self.msg = String(cString: err.msg)
        self.trace = String(cString: err.trace)
    }
}

public enum EC: Int {
    case success = 0
    case unexpected
    case accountExists
    case accountNonexistent
    case accountStringCorrupted
    case alreadyCanceled
    case alreadyPremium
    case appStoreAccountAlreadyLinked
    case alreadySyncing
    case cannotCancelSubscriptionForAppStore
    case cardDecline
    case cardExpired
    case cardInsufficientFunds
    case cardInvalidCvc
    case cardInvalidExpMonth
    case cardInvalidExpYear
    case cardInvalidNumber
    case cardNotSupported
    case clientUpdateRequired
    case currentUsageIsMoreThanNewTier
    case diskPathInvalid
    case diskPathTaken
    case drawingInvalid
    case existingRequestPending
    case fileNameContainsSlash
    case fileNameTooLong
    case fileNameEmpty
    case fileNonexistent
    case fileNotDocument
    case fileNotFolder
    case fileParentNonexistent
    case folderMovedIntoSelf
    case insufficientPermission
    case invalidPurchaseToken
    case invalidAuthDetails
    case keyPhraseInvalid
    case linkInSharedFolder
    case linkTargetIsOwned
    case linkTargetNonexistent
    case multipleLinksToSameFile
    case notPremium
    case usageIsOverDataCap
    case usageIsOverFreeTierDataCap
    case oldCardDoesNotExist
    case pathContainsEmptyFileName
    case pathTaken
    case rootModificationInvalid
    case rootNonexistent
    case reReadRequired
    case serverDisabled
    case serverUnreachable
    case shareAlreadyExists
    case shareNonexistent
    case tryAgain
    case usernameInvalid
    case usernameNotFound
    case usernamePublicKeyMismatch
    case usernameTaken

    init(_ lbEC: LbEC) {
        self = EC(rawValue: Int(lbEC.rawValue))!
    }
}

