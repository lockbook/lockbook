import LocalAuthentication

class AuthHelper {
    static func authenticateWithBiometricsOrPasscode(completion: @escaping (Bool) -> Void) {
        let context = LAContext()
        var error: NSError?

        if context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &error) {
            let reason = "Authenticate to reveal your account keys"

            context.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: reason) { success, _ in
                DispatchQueue.main.async {
                    completion(success || isPreviewEnvironmentKey.defaultValue)
                }
            }
        } else {
            completion(true)
        }
    }
}
