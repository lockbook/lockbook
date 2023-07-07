import Combine
import Foundation
import SwiftLockbookCore

struct CreatingFolderInfo: Identifiable {
    var id = UUID()
    
    let parentPath: String
    let maybeParent: UUID?
}


class SheetState: ObservableObject {
    // These can't just be a part of OutlineContextMenu because the view goes away before
    // the sheet is presented
    
    @Published var creatingFolder: Bool = false {
        didSet {
            if !creatingFolder && creatingFolderInfo != nil {
                creatingFolderInfo = nil
            }
        }
    }
    @Published var creatingFolderInfo: CreatingFolderInfo? {
        didSet {
            creatingFolder = creatingFolderInfo != nil
        }
    }
    
    @Published var moving: Bool = false {
        didSet {
            if !moving && movingInfo != nil {
                movingInfo = nil
            }
        }
    }
    @Published var movingInfo: File? {
        didSet {
            moving = movingInfo != nil
        }
    }
    
    @Published var acceptingShare: Bool = false {
        didSet {
            if !acceptingShare && acceptingShareInfo != nil {
                acceptingShareInfo = nil
            }
        }
    }
    @Published var acceptingShareInfo: File? {
        didSet {
            acceptingShare = acceptingShareInfo != nil
        }
    }
    
    @Published var sharingFile: Bool = false {
        didSet {
            if !sharingFile && sharingFileInfo != nil {
                sharingFileInfo = nil
            }
        }
    }
    @Published var sharingFileInfo: File? {
        didSet {
            sharingFile = sharingFileInfo != nil
        }
    }
}
