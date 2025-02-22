import SwiftUI
import SwiftWorkspace

struct TabsSheet: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceState: WorkspaceState
    
    let info: [(name: String, id: UUID)]
    
    var body: some View {
        VStack {
            Button(action: {
                homeState.tabsSheetInfo = nil
                workspaceState.requestCloseAllTabs()
            }, label: {
                HStack {
                    Image(systemName: "xmark.circle")
                        .foregroundColor(.primary)
                        .imageScale(.medium)
                        .padding(.trailing)
                    
                    Text("Close all tabs")
                        .foregroundColor(.primary)
                        .font(.body)
                    
                    Spacer()
                }
                .padding(.horizontal)
                .padding(.top, 5)
            })
            
            Divider()
                .padding(.horizontal)
                .padding(.vertical, 3)
            
            ForEach(info, id: \.id) { info in
                Button(action: {
                    workspaceState.requestOpenDoc(info.id)
                }, label: {
                    HStack {
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
}

#if os(iOS)
@available(iOS 17.0, *)
#Preview {
    @Previewable @State var sheetInfo: TabSheetInfo? = TabSheetInfo(info: [(name: "Cookie", id: UUID())])
    
    Color.accentColor
        .optimizedSheet(
            item: $sheetInfo,
            constrainedSheetHeight: .constant(200),
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
