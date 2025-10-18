import SwiftUI
import SwiftWorkspace

struct PendingSharesView: View {
    @StateObject var model = PendingSharesViewModel()

    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel

    var body: some View {
        VStack {
            if let error = model.error {
                Spacer()

                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
                    .lineLimit(2, reservesSpace: false)
                    .padding(.top, 5)

                Spacer()
            } else if let pendingShares = model.pendingShares {
                if pendingShares.isEmpty {
                    Spacer()

                    Image(systemName: "person.2.slash")
                        .padding(.vertical, 5)
                        .imageScale(.large)

                    Text("You have no pending shares.")

                    Spacer()
                } else {
                    ScrollView {
                        VStack {
                            ForEach(pendingShares.sorted { $0 > $1 }, id: \.id)
                            { file in
                                PendingShareFileCell(
                                    pendingSharesModel: model,
                                    file: file
                                )
                            }
                        }
                        .padding(.horizontal)
                    }
                }
            } else {
                ProgressView()
            }
        }
        .navigationTitle("Pending Shares")
        .modifier(LargeNavigationTitleBar())
    }
}

struct PendingShareFileCell: View {
    @EnvironmentObject var homeState: HomeState
    @Environment(\.dismiss) private var dismiss

    @ObservedObject var pendingSharesModel: PendingSharesViewModel
    @State var confirmRejection = false

    let file: File

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: FileIconHelper.fileToSystemImageName(file: file))
                .foregroundColor(
                    file.type == .folder ? Color.accentColor : .secondary
                )
                .imageScale(.large)

            Text(file.name)
                .font(.title3)

            Spacer()

            Button {
                homeState.selectSheetInfo = .acceptShare(
                    name: file.name,
                    id: file.id
                )

                dismiss()
            } label: {
                Image(systemName: "plus.circle")
                    .imageScale(.large)
                    .foregroundColor(Color.accentColor)
            }
            .buttonStyle(.plain)

            Button {
                confirmRejection = true
            } label: {
                Image(systemName: "minus.circle")
                    .imageScale(.large)
                    .foregroundColor(.red)
            }
            .buttonStyle(.plain)
        }
        .padding(.vertical, 7)
        .contentShape(Rectangle())
        .confirmationDialog(
            "Are you sure?",
            isPresented: $confirmRejection,
            titleVisibility: .visible
        ) {
            Button("Reject \"\(file.name)\"", role: .destructive) {
                pendingSharesModel.rejectShare(id: file.id)
                dismiss()
            }
        }
    }
}

#Preview("Pending Shares") {
    NavigationStack {
        PendingSharesView()
            .withMacPreviewSize()
            .withCommonPreviewEnvironment()
    }
}
