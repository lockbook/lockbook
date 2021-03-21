import Foundation
import SwiftLockbookCore
import SwiftUI
import Combine

class GlobalState: ObservableObject {
    let documenstDirectory: String
    let api: LockbookApi
    @Published var state: DbState               // Handles post update logic
    @Published var account: Account?            // Determines whether to show onboarding or the main view
    @Published var globalError: AnyFfiError?    // Shows modals for unhandled errors
    @Published var files: [FileMetadata] = []   // What the file tree displays
    @Published var root: FileMetadata?          // What the file tree displays
    @Published var syncing: Bool = false {      // Setting this to true kicks off a sync
        didSet {
            if oldValue == false && syncing == true {
                serialQueue.async {
                    self.syncChannel.send(self.api.syncAll())
                }
            }
        }
    }
    var syncTimer: Timer? = nil
    var lastSyncedTimer: Timer? = nil
    @Published var work: Int = 0
    @Published var lastSynced: String = ""
    let serialQueue = DispatchQueue(label: "syncQueue")
    #if os(iOS)
    @Published var openDrawing: DrawingModel
    #endif
    @Published var openDocument: Content

    private var syncChannel = PassthroughSubject<FfiResult<SwiftLockbookCore.Empty, SyncAllError>, Never>()
    private var cancellableSet: Set<AnyCancellable> = []

    func startLastSyncedTimer() {
        lastSyncedTimer = Timer.scheduledTimer(timeInterval: 60, target: self, selector: #selector(setLastSynced), userInfo: nil, repeats: true)
    }
    
    func startOrRestartSyncTimer() {
        syncTimer?.invalidate()
        syncTimer = Timer.scheduledTimer(timeInterval: 30*60, target: self, selector: #selector(syncTimerTick), userInfo: nil, repeats: true)
    }

    @objc func syncTimerTick() {
        syncing = true
    }

    func stopTimer() {
        syncTimer?.invalidate()
    }

    func loadAccount() {
        switch api.getAccount() {
        case .success(let acc):
            account = acc
        case .failure(let err):
            handleError(err)
        }
    }

    func migrate() {
        let res = api.migrateState()
                .eraseError()
                .flatMap(transform: { _ in api.getState().eraseError() })
        switch res {
        case .success(let newState):
            state = newState
            loadAccount()
            switch newState {
            case .ReadyToUse:
                break
            default:
                print("Weird state after migration: \(newState)")
            }
        case .failure(let err):
            handleError(err)
        }
    }

    func purge() {
        let lockbookDir = URL(fileURLWithPath: documenstDirectory).appendingPathComponent("lockbook.sled")
        if let _ = try? FileManager.default.removeItem(at: lockbookDir) {
            DispatchQueue.main.async {
                self.account = nil
                switch self.api.getState() {
                case .success(let db):
                    self.state = db
                case .failure(let err):
                    self.handleError(err)
                }
            }
        }
    }

    func handleError(_ error: AnyFfiError) {
        DispatchQueue.main.async {
            self.globalError = error
        }
    }

    func checkForLocalWork() {
        DispatchQueue.main.async {
            switch self.api.getLocalChanges() {
            case .success(let local):
                self.work = local.count
            case .failure(let err):
                switch err.kind {
                case .UiError(let error):
                    // TODO handle error
                    break
                case .Unexpected(_):
                    self.handleError(err)
                }
            }
        }
    }

    func documentChangeHappened() {
        startOrRestartSyncTimer()
        checkForLocalWork()
    }

    func updateFiles() {
        if (account != nil) {
            switch api.getRoot() {
            case .success(let root):
                self.root = root
                switch api.listFiles() {
                case .success(let metas):
                    self.files = metas
                    metas.forEach { meta in
                        #if os(iOS)
                        if let openDrawingMeta = openDrawing.meta, meta.id == openDrawingMeta.id, meta.contentVersion != openDrawingMeta.contentVersion {
                            DispatchQueue.main.async {
                                self.openDrawing.closeDrawing(meta: openDrawingMeta)
                                self.openDrawing.loadDrawing(meta: openDrawingMeta)
                            }
                        }
                        #endif
                        if let openDocumentMeta = openDocument.meta, meta.id == openDocumentMeta.id, meta.contentVersion != openDocumentMeta.contentVersion {
                            DispatchQueue.main.async {
                                self.openDocument.closeDocument(meta: openDocumentMeta)
                                self.openDocument.openDocument(meta: openDocumentMeta)
                            }
                        }
                    }

                case .failure(let err):
                    handleError(err)
                }
            case .failure(let err):
                handleError(err)
            }
        }
    }
    
    @objc func setLastSynced() {
        self.lastSynced = (try? self.api.getLastSyncedHumanString().get())!
    }

    init(documenstDirectory: String) {
        print("Initializing core...")

        self.documenstDirectory = documenstDirectory
        self.api = CoreApi(documentsDirectory: documenstDirectory)
        self.state = (try? self.api.getState().get())!
        self.account = (try? self.api.getAccount().get())
        self.lastSynced = (try? self.api.getLastSyncedHumanString().get())!
        self.openDocument = Content(write: api.updateFile, read: api.getFile)
        #if os(iOS)
        self.openDrawing = DrawingModel(write: api.writeDrawing, read: api.readDrawing)
        openDrawing.writeListener = documentChangeHappened
        #endif
        openDocument.writeListener = documentChangeHappened
        updateFiles()

        print("Starting")
        startOrRestartSyncTimer()
        startLastSyncedTimer()
        syncChannel
                .debounce(for: .milliseconds(500), scheduler: RunLoop.main)
                .removeDuplicates(by: {
                    switch ($0, $1) {
                    case (.failure(let e1), .failure(let e2)):
                        return e1 == e2
                    default:
                        return false
                    }
                })
                .receive(on: RunLoop.main)
                .sink(receiveValue: { res in
                    self.syncing = false
                    switch res {
                    case .success(_):
                        self.updateFiles()
                        self.setLastSynced()
                        self.checkForLocalWork()
                    case .failure(let err):
                        self.handleError(err)
                    }
                })
                .store(in: &cancellableSet)
    }

    init() {
        self.documenstDirectory = "<USING-FAKE-API>"
        self.api = FakeApi()
        self.state = .ReadyToUse
        self.account = Account(username: "testy", apiUrl: "ftp://lockbook.gov", keys: .empty)
        #if os(iOS)
        self.openDrawing = DrawingModel(write: { _, _ in .failure(.init(unexpected: "LAZY")) }, read: { _ in .failure(.init(unexpected: "LAZY")) })
        #endif
        self.openDocument = Content(write: { _, _ in .failure(.init(unexpected: "LAZY")) }, read: { _ in .failure(.init(unexpected: "LAZY")) })
        if case .success(let root) = api.getRoot(), case .success(let metas) = api.listFiles() {
            self.files = metas
            self.root = root
        }
    }
}
