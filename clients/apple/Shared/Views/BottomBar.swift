import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace

struct BottomBar: View {
    
    var isiOS = false
    
    @EnvironmentObject var workspace: WorkspaceState
    @EnvironmentObject var settings: SettingsService
        
#if os(iOS)
    var menu: some View {
        HStack {
            if isiOS && !workspace.syncing {
                Button(action: {
                    DI.files.createDoc(isDrawing: false)
                }) {
                    Image(systemName: "doc.badge.plus")
                        .imageScale(.large)
                        .foregroundColor(.accentColor)
                        .frame(width: 40, height: 40, alignment: .center)
                }
                
                Button(action: {
                    DI.files.createDoc(isDrawing: true)
                }) {
                    Image(systemName: "pencil.tip.crop.circle.badge.plus")
                        .imageScale(.large)
                        .foregroundColor(.accentColor)
                        .frame(width: 40, height: 40, alignment: .center)
                }
                
                Button(action: {
                    DI.sheets.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent() ?? "ERROR", maybeParent: nil)
                }) {
                    Image(systemName: "folder.badge.plus")
                        .imageScale(.large)
                        .foregroundColor(.accentColor)
                        .frame(width: 40, height: 40, alignment: .center)
                }
            }
        }
    }
    
    @ViewBuilder var syncButton: some View {
        if workspace.syncing {
            ProgressView()
                .frame(width: 40, height: 40, alignment: .center)
                .padding(.trailing, 9)
        } else {
            Button(action: {
                workspace.requestSync()
            }) {
                Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                    .imageScale(.large)
                    .foregroundColor(.accentColor)
                    .frame(width: 40, height: 40, alignment: .center)
            }
        }
    }
#else
    @ViewBuilder var syncButton: some View {
        if workspace.syncing {
            Text("")
                .font(.callout)
                .foregroundColor(Color.gray)
        } else {
            Button(action: {
                workspace.requestSync()
            }) {
                Text(workspace.offline ? "Try again" : "Sync now")
                    .font(.callout)
                    .foregroundColor(Color.init(red: 0.3, green: 0.45, blue: 0.79))
            }
        }
    }
    
    func showUpgradeToPremium() {
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
    }
    
    @ViewBuilder
    var usageBar: some View {
        if let usage = settings.usages {
            VStack {
                ColorProgressBar(value: settings.usageProgress)
                
                HStack {
                    if settings.usageProgress > 0.8 {
                        Button(action: {
                            showUpgradeToPremium()
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
                    
                    if settings.usageProgress <= 0.8 && settings.tier != .Premium {
                        Button(action: {
                            showUpgradeToPremium()
                        }, label: {
                            Image(systemName: "dollarsign.circle")
                                .foregroundColor(.gray)
                        })
                        
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
    
    @ViewBuilder
    var statusText: some View {
        Text(workspace.statusMsg)
    }
    
    var body: some View {
        Group {
            #if os(iOS)
            HStack {
                syncButton
                Spacer()
                statusText
                Spacer()
                menu
            }
            .padding(.horizontal, 10)
            #else
            VStack {
                Divider()
                HStack {
                    statusText
                    Spacer()
                    syncButton
                }
                usageBar
            }
            .padding(.bottom)
            .padding(.horizontal)
            #endif
        }
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
