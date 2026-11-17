import SwiftUI
import SwiftWorkspace

#if os(iOS)
    struct StorageView: View {
        let settingsModel: SettingsModel

        @State private var model = StorageModel()

        var body: some View {
            Form {
                if let usage = model.usage {
                    Section {
                        StorageSummaryView(usage: usage)
                    }

                    if let message = storageMessage(usage: usage, outOfSpace: AppState.lb.events.status.outOfSpace) {
                        Section {
                            VStack(alignment: .leading, spacing: 4) {
                                Text(message.title)
                                    .font(.headline)
                                Text(message.detail)
                                    .foregroundStyle(.secondary)
                            }

                            if settingsModel.isPremium != true {
                                NavigationLink("Upgrade") {
                                    UpgradeAccountView(settingsModel: settingsModel)
                                }
                            }
                        }
                    }
                }

                Section {
                    Picker("Breakdown", selection: $model.tab) {
                        ForEach(StorageTab.allCases) { tab in
                            Text(tab.rawValue).tag(tab)
                        }
                    }
                    .pickerStyle(.segmented)
                    .listRowBackground(Color.clear)
                    .listRowInsets(EdgeInsets())
                }

                Section {
                    if let report = model.report {
                        StorageBreakdownRows(model: model, report: report)
                    } else {
                        ProgressView()
                            .frame(maxWidth: .infinity)
                    }
                }
            }
            .navigationTitle("Storage")
            .navigationBarTitleDisplayMode(.inline)
            .onAppear {
                model.load()
            }
            .onChange(of: AppState.lb.events.status.usage) {
                model.load(refreshCap: false)
            }
        }
    }
#endif
