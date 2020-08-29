import CLockbookCore

struct SwiftLockbookCore {
    func getApiLoc() -> String {
        let result = get_api_loc()
        let resultString = String(cString: result!)
        release_pointer(UnsafeMutablePointer(mutating: result))
        return resultString
    }
}
