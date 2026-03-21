import AppKit
import SwiftUI
import SwiftWorkspace

class CreateDocumentMenuItem: NSMenuItem {
    @ObservedObject var workspaceInput: WorkspaceInputState

    let file: File

    init(workspaceInput: WorkspaceInputState, file: File) {
        self.workspaceInput = workspaceInput
        self.file = file

        super.init(title: "Create document", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_: AnyObject) {
        workspaceInput.createDocAt(parent: file.id, drawing: false)
    }
}

class CreateDrawingMenuItem: NSMenuItem {
    @ObservedObject var workspaceInput: WorkspaceInputState

    let file: File

    init(workspaceInput: WorkspaceInputState, file: File) {
        self.workspaceInput = workspaceInput
        self.file = file

        super.init(title: "Create drawing", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_: AnyObject) {
        workspaceInput.createDocAt(parent: file.id, drawing: true)
    }
}

class CreateFolderMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let file: File

    init(homeState: HomeState, file: File) {
        self.homeState = homeState
        self.file = file

        super.init(title: "Create folder", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_: AnyObject) {
        homeState.sheetInfo = .createFolder(parent: file)
    }
}

class RenameFileMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let file: File

    init(homeState: HomeState, file: File) {
        self.homeState = homeState
        self.file = file

        super.init(title: "Rename", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_: AnyObject) {
        homeState.sheetInfo = .rename(file: file)
    }
}

class DeleteMenuItem: NSMenuItem {
    @ObservedObject var filesModel: FilesViewModel

    let files: [File]

    init(filesModel: FilesViewModel, files: [File]) {
        self.filesModel = filesModel
        self.files = files

        super.init(title: "Delete", action: #selector(delete(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func delete(_: AnyObject) {
        filesModel.deleteFileConfirmation = files
    }
}

class ShareMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let file: File

    init(homeState: HomeState, file: File) {
        self.homeState = homeState
        self.file = file

        super.init(title: "Share", action: #selector(share(_:)), keyEquivalent: "")

        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func share(_: AnyObject) {
        homeState.sheetInfo = .share(file: file)
    }
}

class CopyLinkMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let file: File

    init(homeState: HomeState, file: File) {
        self.homeState = homeState
        self.file = file

        super.init(title: "Copy file link", action: #selector(share(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func share(_: AnyObject) {
        ClipboardHelper.copyFileLink(file.id)
    }
}

class ShareExternallyMenu: NSMenuItem {
    let file: File

    let fileTree: NSOutlineView

    init(homeState: HomeState, file: File, fileTree: NSOutlineView) {
        self.file = file
        self.fileTree = fileTree

        super.init(title: "Share externally", action: nil, keyEquivalent: "")

        submenu = NSMenu(title: "Share externally")
        submenu!.addItem(ShareToMenuItem(homeState: homeState, file: file, fileTree: fileTree))

        if file.type == .document {
            submenu!.addItem(CopyLinkMenuItem(homeState: homeState, file: file))
        }
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }
}

class ShareToMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let file: File

    let fileTree: NSOutlineView

    init(homeState: HomeState, file: File, fileTree: NSOutlineView) {
        self.homeState = homeState
        self.file = file
        self.fileTree = fileTree
        super.init(title: "Share to...", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_: AnyObject) {
        if let dest = ImportExportHelper.exportFilesToTempDir(homeState: homeState, file: file) {
            let maybeFileRow = fileTree.rowView(atRow: fileTree.row(forItem: file), makeIfNecessary: false)

            if let fileRow = maybeFileRow {
                NSSharingServicePicker(items: [dest]).show(relativeTo: .zero, of: fileRow, preferredEdge: .maxY)
            }
        }
    }
}

class ShareMultipleToMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let files: [File]
    let fileTree: NSOutlineView

    init(homeState: HomeState, files: [File], fileTree: NSOutlineView) {
        self.homeState = homeState
        self.files = files
        self.fileTree = fileTree
        super.init(title: "Share to...", action: #selector(create(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func create(_: AnyObject) {
        var urls: [URL] = []

        for file in files {
            if let url = ImportExportHelper.exportFilesToTempDir(homeState: homeState, file: file) {
                urls.append(url)
            }
        }

        NSSharingServicePicker(items: urls).show(relativeTo: .zero, of: fileTree, preferredEdge: .maxY)
    }
}

class MoveToMenuItem: NSMenuItem {
    @ObservedObject var homeState: HomeState

    let files: [File]

    init(homeState: HomeState, files: [File]) {
        self.homeState = homeState
        self.files = files
        super.init(title: "Move to...", action: #selector(moveTo(_:)), keyEquivalent: "")
        target = self
    }

    @available(*, unavailable)
    required init(coder _: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    @objc func moveTo(_: AnyObject) {
        homeState.selectSheetInfo = .move(files: files)
    }
}
