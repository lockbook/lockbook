import SwiftUI
import SwiftWorkspace

struct TabsSheet: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState

    @Environment(\.dismiss) private var dismiss

    @State var info: [(name: String, id: UUID)]

    var body: some View {
        VStack(spacing: 0) {
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
                .padding(.vertical)

            ScrollView {
                ForEach(info, id: \.id) { info in
                    Button(
                        action: {
                            workspaceInput.openFile(id: info.id)
                            dismiss()
                        },
                        label: {
                            HStack {
                                Image(
                                    systemName:
                                        FileIconHelper.docNameToSystemImageName(
                                            name: info.name
                                        )
                                )
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

                                Button(
                                    action: {
                                        self.closeTab(id: info.id)
                                    },
                                    label: {
                                        Image(systemName: "xmark.circle.fill")
                                            .foregroundColor(.red)
                                            .imageScale(.medium)
                                            .padding(.leading)
                                    }
                                )

                            }
                            .padding(.horizontal)
                            .padding(.vertical, 6)
                            .background(
                                info.id == workspaceOutput.openDoc
                                    ? RoundedRectangle(cornerRadius: 10).fill(
                                        .gray.opacity(0.2)
                                    ) : nil
                            )
                            .padding(.horizontal)
                            .padding(.vertical, 2)
                        }
                    )
                }
            }
            .frame(maxHeight: 400)
            .fixedSize(horizontal: false, vertical: true)
        }
        .padding(.top)
    }

    func closeTab(id: UUID) {
        workspaceInput.closeDoc(id: id)
        let i = self.info.firstIndex(where: { $0.id == id })

        if let i {
            self.info.remove(at: i)
        }

        if info.isEmpty {
            dismiss()
        }
    }

    func closeAllTabs() {
        workspaceInput.closeAllTabs()
        dismiss()
    }
}

#if os(iOS)
    @available(iOS 17.0, *)
    #Preview {
        @Previewable @State var sheetInfo: TabSheetInfo? = TabSheetInfo(info: [
            (name: "Cookie", id: UUID())
        ])

        Color.accentColor
            .optimizedSheet(
                item: $sheetInfo,
                compactSheetHeight: .constant(100),
                presentedContent: { item in
                    TabsSheet(
                        info: item.info
                    )
                }
            )
            .withCommonPreviewEnvironment()
    }
#endif
