import SwiftUI

func updateInMainThread(f: @escaping () -> Void) {
    DispatchQueue.main.async {
        f()
    }
}
