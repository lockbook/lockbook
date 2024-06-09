import Foundation
import SwiftLockbookCore

#if os(macOS)
import AppKit
import Combine
#endif

class AccountService: ObservableObject {
    let core: LockbookApi
    
    @Published var account: Account? = nil
    var calculated = false
        
    init(_ core: LockbookApi) {
        self.core = core
        switch core.getAccount() {
        case .success(let account):
            self.account = account
        case .failure(let error):
            switch error.kind {
            case .UiError(let getAccountError):
                switch getAccountError {
                case .NoAccount:
                    account = nil
                }
            case .Unexpected(_):
                DI.errors.handleError(error)
            }
        }
        
        calculated = true
        #if os(macOS)
        registerBackgroundTasks()
        #endif
    }
    
    #if os(macOS)

    let backgroundSyncStartSecs = 60 * 5
    let backgroundSyncContSecs = 60 * 60

    private var cancellables: Set<AnyCancellable> = []
    var currentSyncTask: DispatchWorkItem? = nil

    func registerBackgroundTasks() {
        let willResignActivePublisher = NotificationCenter.default.publisher(for: NSApplication.willResignActiveNotification)
        let willBecomeActivePublisher = NotificationCenter.default.publisher(for: NSApplication.willBecomeActiveNotification)

        willResignActivePublisher
            .sink { [weak self] _ in
                if !DI.onboarding.initialSyncing {
                    self?.scheduleBackgroundTask(initialRun: true)
                }
            }
            .store(in: &cancellables)

        willBecomeActivePublisher
            .sink { [weak self] _ in
                self?.endBackgroundTasks()
            }
            .store(in: &cancellables)

    }

    func scheduleBackgroundTask(initialRun: Bool) {
        let newSyncTask = DispatchWorkItem { [weak self] in
            DI.sync.backgroundSync(onSuccess: {
                self?.scheduleBackgroundTask(initialRun: false)
            }, onFailure: {
                self?.scheduleBackgroundTask(initialRun: false)
            })
        }
        
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds((initialRun ? backgroundSyncStartSecs : backgroundSyncContSecs)), execute: newSyncTask)
        
        currentSyncTask = newSyncTask
    }

    func endBackgroundTasks() {
        currentSyncTask?.cancel()
        currentSyncTask = nil
    }

    #endif

    
    func getAccount() {
        if account == nil {
            switch core.getAccount() {
            case .success(let account):
                self.account = account
            case .failure(let error):
                switch error.kind {
                case .UiError(let getAccountError):
                    switch getAccountError {
                    case .NoAccount:
                        print("account get unsuccessful")
                        self.account = nil
                    }
                case .Unexpected(_):
                    DI.errors.handleError(error)
                }
            }
        }
    }
    
    func logout() {
        DI.freshState()
        core.logoutAndExit()
    }
    
    func deleteAccount() {
        switch core.deleteAccount() {
        case .success(_):
            DI.freshState()
        case .failure(let error):
            DI.errors.handleError(error)
        }
    }
}
