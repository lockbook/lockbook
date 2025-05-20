import LocalAuthentication

class AuthHelper {
    static func authenticateWithBiometricsOrPasscode(completion: @escaping (Bool) -> Void) {
        let context = LAContext()
        var error: NSError?

        if context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) {
            let reason = "Authenticate to access the app"

            context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason) { success, authenticationError in
                DispatchQueue.main.async {
                    completion(success || isPreviewEnvironmentKey.defaultValue)
                }
            }
        } else {
            completion(true)
        }
    }
}

