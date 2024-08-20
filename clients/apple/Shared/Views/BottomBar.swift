import SwiftUI
import SwiftLockbookCore
import SwiftWorkspace

#if os(iOS)

struct BottomBar: View {
    var isiOS = false
    
    @EnvironmentObject var selected: SelectedFilesState
    @EnvironmentObject var workspace: WorkspaceState

    var body: some View {
        if selected.selectedFiles != nil {
            selectionView
        } else {
            mainView
        }
    }
    
    var selectionView: some View {
        HStack(alignment: .center) {
            Spacer()
            
            Button(role: .destructive, action: {
                if let selectedFiles = selected.selectedFiles {
                    DI.sheets.deleteConfirmationInfo = Array(selectedFiles)
                }
            }) {
                Image(systemName: "trash")
                    .imageScale(.large)
            }
            .disabled(selected.selectedFiles?.count == 0)
            
            Spacer()
            
            Button(action: {
                if let selectedIds = selected.selectedFiles?.map({ $0.id }) {
                    DI.sheets.movingInfo = .Move(selectedIds)
                }
            }, label: {
                Image(systemName: "folder")
                    .imageScale(.large)
            })
            .disabled(selected.selectedFiles?.count == 0)
            
            Spacer()
            
            Button(action: {
                if let selectedFiles = selected.selectedFiles {
                    exportFilesAndShowShareSheet(metas: Array(selectedFiles))
                    selected.selectedFiles = nil
                }
            }, label: {
                Image(systemName: "square.and.arrow.up")
                    .imageScale(.large)
            })
            .disabled(selected.selectedFiles?.count == 0)
            
            Spacer()
        }
        .foregroundStyle(selected.selectedFiles?.count == 0 ? .gray : .blue)
        .padding(.horizontal)
    }
    
    var mainView: some View {
        HStack(alignment: .center) {
            statusText
            Spacer()
            if isiOS && !workspace.syncing {
                menu
            }
            if !isiOS {
                if workspace.syncing {
                    ProgressView()
                        .frame(width: 40, height: 40, alignment: .center)
                        .padding(.trailing, 5)
                } else {
                    Button(action: {
                        workspace.requestSync()
                    }) {
                        Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                            .imageScale(.large)
                            .foregroundColor(.accentColor)
                    }
                    .padding(.trailing, 5)
                }
            }
        }
        .padding(.horizontal, 15)
        .frame(height: 50)
    }
    
    var statusText: some View {
        Text(workspace.statusMsg)
            .font(.callout)
            .lineLimit(1)
            .padding(.leading, 5)
    }
    
    var menu: some View {
        HStack {
            Button(action: {
                DI.files.createDoc(isDrawing: false)
            }) {
                Image(systemName: "doc.badge.plus")
                    .font(.title2)
                    .foregroundColor(.accentColor)
            }
            .padding(.trailing, 5)
            
            Button(action: {
                DI.files.createDoc(isDrawing: true)
            }) {
                Image(systemName: "pencil.tip.crop.circle.badge.plus")
                    .font(.title2)
                    .foregroundColor(.accentColor)
            }
            .padding(.trailing, 5)
            
            Button(action: {
                DI.sheets.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent() ?? "ERROR", maybeParent: nil)
            }) {
                Image(systemName: "folder.badge.plus")
                    .font(.title2)
                    .foregroundColor(.accentColor)
            }
        }
    }

}

#else

struct BottomBar: View {
    @EnvironmentObject var settings: SettingsService
    @EnvironmentObject var workspace: WorkspaceState
    
    var body: some View {
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
    }
    
    @ViewBuilder
    var statusText: some View {
        Text(workspace.statusMsg)
    }
    
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
}

#endif
