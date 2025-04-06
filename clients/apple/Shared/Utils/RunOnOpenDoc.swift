import SwiftUI
import SwiftWorkspace

func runOnOpenDoc(f: @escaping (File) -> Void) {
    guard let id = AppState.workspaceState.openDoc else {
        return
    }
    
    if let file =  try? AppState.lb.getFile(id: id).get() {
        f(file)
    }
}
