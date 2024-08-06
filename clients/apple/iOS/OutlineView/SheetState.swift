import Combine
import Foundation
import SwiftLockbookCore

struct CreatingFolderInfo: Identifiable {
    var id = UUID()
    
    let parentPath: String
    let maybeParent: UUID?
}

struct RenamingFileInfo: Identifiable {
    let id: UUID
    let name: String
    let parentPath: String
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
    
    @Published var renamingFile: Bool = false {
        didSet {
            if !renamingFile && renamingFileInfo != nil {
                renamingFileInfo = nil
            }
        }
    }
    @Published var renamingFileInfo: RenamingFileInfo? {
        didSet {
            renamingFile = renamingFileInfo != nil
        }
    }
    
    @Published var moving: Bool = false {
        didSet {
            if !moving && movingInfo != nil {
                movingInfo = nil
            }
        }
    }
    @Published var movingInfo: SelectFolderAction? {
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
    
    @Published var deleteConfirmation: Bool = false {
        didSet {
            if !deleteConfirmation && deleteConfirmationInfo != nil {
                deleteConfirmationInfo = nil
            }
        }
    }
    @Published var deleteConfirmationInfo: File? {
        didSet {
            deleteConfirmation = deleteConfirmationInfo != nil
        }
    }
    
    @Published var tabsList: Bool = false
    
    private var cancellables: Set<AnyCancellable> = []
    
    init() {
        DI.workspace.$newFolderButtonPressed.sink { pressed in
            if pressed {
                DI.workspace.newFolderButtonPressed = false
                self.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent() ?? "Error", maybeParent: nil)
            }
        }
        .store(in: &cancellables)
        
        DI.workspace.$renameOpenDoc.sink { isRenaming in
            if isRenaming {
                DI.workspace.renameOpenDoc = false
                let file = DI.files.idsAndFiles[DI.workspace.openDoc!]!
                
                DI.sheets.renamingFileInfo = RenamingFileInfo(id: file.id, name: file.name, parentPath: DI.files.getPathByIdOrParent(maybeId: file.parent) ?? "Error")
            }
        }
        .store(in: &cancellables)

    }
}
