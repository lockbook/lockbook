import SwiftUI
import SwiftLockbookCore

struct BottomBar: View {
    
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var status: StatusService
    @EnvironmentObject var settings: SettingsService
    
#if os(iOS)
    var onCreating: () -> Void = {}
#endif
    
#if os(iOS)
    var menu: some View {
        Button(action: {
            onCreating()
        }) {
            Image(systemName: "plus.circle.fill")
                .imageScale(.large)
                .foregroundColor(.blue)
                .frame(width: 40, height: 40, alignment: .center)
        }
    }
#endif
    
#if os(iOS)
    @ViewBuilder var syncButton: some View {
        if sync.syncing {
            ProgressView()
        } else {
            Button(action: {
                sync.sync()
                status.work = 0
            }) {
                Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                    .imageScale(.large)
                    .foregroundColor(.blue)
                    .frame(width: 40, height: 40, alignment: .center)
            }
        }
    }
#else
    @ViewBuilder var syncButton: some View {
        if sync.syncing {
            Text("")
                .font(.callout)
                .foregroundColor(Color.gray)
            
        } else {
            
            Button(action: {
                sync.sync()
                status.work = 0
            }) {
                Text(sync.offline ? "Try again" : "Sync now")
                    .font(.callout)
                    .foregroundColor(Color.init(red: 0.3, green: 0.45, blue: 0.79))
            }
        }
    }
    
    @ViewBuilder
    var usageBar: some View {
        if let usage = settings.usages {
            VStack {
                ColorProgressBar(value: settings.usageProgress)
                
                HStack {
                    if settings.usageProgress > 0.8 {
                        Button(action: {
                            let previousWindow = NSApplication.shared.windows.last
                            
                            let overlayWindow = NSWindow(
                                contentRect: NSRect(x: 0, y: 0, width: 300, height: 200),
                                styleMask: [.titled, .closable, .miniaturizable, .resizable],
                                backing: .buffered,
                                defer: false
                            )
                            
                            if let previousFrame = previousWindow?.frame {
                                let windowSize = overlayWindow.frame.size
                                let x = previousFrame.origin.x + (previousFrame.size.width - windowSize.width) / 2
                                let y = previousFrame.origin.y + (previousFrame.size.height - windowSize.height) / 2
                                overlayWindow.setFrame(NSRect(x: x, y: y, width: windowSize.width, height: windowSize.height), display: true)
                            }

                            
                            overlayWindow.isReleasedWhenClosed = false
                            overlayWindow.contentView = NSHostingView(rootView: UpgradeToPremium().realDI())
                            overlayWindow.makeKeyAndOrderFront(nil)
                        }, label: {
                            Text("Upgrade")
                                .foregroundColor(.accentColor)
                                .font(.callout)
                        })
                        
                        Spacer()
                    }
                    
                    Text("\(usage.serverUsages.serverUsage.readable) out of \(usage.serverUsages.dataCap.readable) used")
                        .foregroundColor(.gray)
                        .font(.callout)
                    
                    if settings.usageProgress <= 0.8 {
                        Image(systemName: "plus.circle.fill")
                            .foregroundColor(.gray)
                        
                        Spacer()
                    }
                }
            }
        } else {
            VStack {
                HStack(alignment: .firstTextBaseline) {
                    RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                        .fill(.gray)
                        .opacity(0.1)
                        .cornerRadius(5)
                        .frame(width: 70, height: 16)
                    
                    RoundedRectangle(cornerSize: CGSize(width: 5, height: 5))
                        .fill(.gray)
                        .opacity(0.1)
                        .cornerRadius(5)
                        .frame(width: 40, height: 16)
                    
                    Spacer()
                }
            }
            .onAppear {
                settings.calculateUsage()
            }
        }
    }
#endif
    
    var localChangeText: String {
        if status.work == 0 { // not shown in this situation
            return ""
        } else if status.work == 1 {
            return "1 unsynced change"
        } else {
            return "\(status.work) unsynced changes"
        }
    }
    
    @ViewBuilder
    var statusText: some View {
        if sync.upgrade {
            Text("Update required")
                .foregroundColor(.secondary)
        } else if sync.syncing {
            Text("Syncing...")
                .foregroundColor(.secondary)
        } else if sync.outOfSpace {
            Text("Out of space")
                .foregroundColor(.secondary)
        } else {
            if sync.offline {
                Text("Offline")
                    .foregroundColor(.secondary)
            } else {
                Text(status.work == 0 ? "Last update: \(status.lastSynced)" : localChangeText)
                    .font(.callout)
                    .foregroundColor(.secondary)
                    .bold()
            }
        }
    }
    
    var body: some View {
#if os(iOS)
        syncButton
        Spacer()
        statusText
        Spacer()
        menu
#else
        Divider()
        HStack {
            statusText
            Spacer()
            syncButton
        }
        usageBar
#endif
    }
}

#if os(iOS)
struct SyncingPreview: PreviewProvider {
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar()
                }
            }
        }
        .mockDI()
        .onAppear {
            Mock.sync.syncing = true
        }
        
        
    }
}

struct NonSyncingPreview: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar()
                }
            }
        }
        .mockDI()
        .onAppear {
            Mock.sync.sync()
        }
        
        
    }
}

struct OfflinePreview: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar()
                }
            }
        }
        .mockDI()
        .onAppear {
            Mock.sync.offline = true
        }
        
        
    }
}

struct WorkItemsPreview: PreviewProvider {
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar()
                        .onAppear {
                            Mock.status.work = 5
                        }
                }
            }
            .mockDI()
        }
        
        
    }
}
#endif
