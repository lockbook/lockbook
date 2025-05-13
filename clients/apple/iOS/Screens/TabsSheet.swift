import SwiftUI
import SwiftWorkspace

struct TabsSheet: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceState: WorkspaceState
    
    @Environment(\.dismiss) private var dismiss
    
    @State var info: [(name: String, id: UUID)]
    
    var body: some View {
        VStack {
            Button {
                self.closeAllTabs()
            } label: {
                Text("Close all")
                    .font(.body)
                    .fontWeight(.semibold)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .padding(.horizontal)
            
            Divider()
                .padding(.horizontal)
                .padding(.vertical, 3)
            
            ForEach(info, id: \.id) { info in
                Button(action: {
                    AppState.workspaceState.requestOpenDoc(info.id)
                }, label: {
                    HStack {
                        Button(action: {
                            self.closeTab(id: info.id)
                        }, label: {
                            Image(systemName: "xmark.circle")
                                .foregroundColor(.red)
                                .imageScale(.medium)
                                .padding(.trailing)
                        })
                        
                        Image(systemName: FileIconHelper.docNameToSystemImageName(name: info.name))
                            .foregroundColor(.primary)
                            .imageScale(.medium)
                            .padding(.trailing)
                        
                        Text(info.name)
                            .foregroundColor(.primary)
                            .font(.body)
                            .bold(false)
                            .lineLimit(1)
                            .truncationMode(.tail)
                        
                        Spacer()
                        
                        if info.id == workspaceState.openDoc {
                            Image(systemName: "checkmark.circle")
                                .foregroundColor(.primary)
                                .font(.headline)
                        }
                    }
                    .padding(.horizontal)
                    .padding(.vertical, 3)
                })
            }
        }
    }
    
    func closeTab(id: UUID) {
        AppState.workspaceState.requestCloseDoc(id: id)
        let i = self.info.firstIndex(where: {  $0.id == id })
        
        if let i {
            self.info.remove(at: i)
        }
        
        if info.isEmpty {
            dismiss()
        }
    }
    
    func closeAllTabs() {
        AppState.workspaceState.requestCloseAllTabs()
        dismiss()
    }
}

#if os(iOS)
@available(iOS 17.0, *)
#Preview {
    @Previewable @State var sheetInfo: TabSheetInfo? = TabSheetInfo(info: [(name: "Cookie", id: UUID())])
    
    Color.accentColor
        .optimizedSheet(
            item: $sheetInfo,
            constrainedSheetHeight: .constant(100),
            presentedContent: { item in
                TabsSheet(
                    info: item.info
                )
            }
        )
        .environmentObject(HomeState())
        .environmentObject(WorkspaceState())
}
#endif
