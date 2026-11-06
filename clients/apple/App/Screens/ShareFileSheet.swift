import SwiftUI
import SwiftWorkspace

struct ShareFileSheet: View {
    @Environment(FilesModel.self) private var filesModel
    @Environment(WorkspaceInputState.self) private var workspaceInput

    let id: UUID

    @State private var query = ""
    @State private var lookup: UsernameLookup = .idle
    @State private var lookupFor = ""
    @State private var staged: [StagedPerson] = []
    @State private var mode: ShareMode = .write
    @State private var knownUsernames: [String] = []
    @State private var sharing = false
    @State private var error = ""
    @State private var rosterHeight: CGFloat = 0
    @State private var linkCopied = false
    @State private var copyResetTask: Task<Void, Never>? = nil
    @FocusState private var queryFocused: Bool

    private var file: File? {
        filesModel.idsToFiles[id]
    }

    private var trimmedQuery: String {
        query.trimmingCharacters(in: .whitespaces)
    }

    var body: some View {
        #if os(iOS)
            ScrollView {
                content
                    .padding()
            }
            .scrollDismissesKeyboard(.interactively)
            .presentationDetents([.medium, .large])
        #else
            content
                .padding()
                .frame(width: 440)
                .onAppear {
                    queryFocused = true
                }
        #endif
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 18) {
            header

            roster

            addPeople

            Divider()

            HStack(spacing: 10) {
                Button(action: copyLink) {
                    Label(linkCopied ? "Copied" : "Copy link", systemImage: linkCopied ? "checkmark" : "link")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.bordered)

                shareExternally
            }
        }
        .task {
            await loadKnownUsernames()
        }
        .task(id: trimmedQuery) {
            await verifyField(trimmedQuery)
        }
        .onChange(of: query) {
            queryChanged()
        }
    }

    private var header: some View {
        HStack {
            Text("Share")
                .font(.title2)
                .bold()

            Spacer()

            Text(file?.name ?? "")
                .font(.callout)
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(.middle)

            #if os(macOS)
                SheetCloseButton()
            #endif
        }
    }

    @ViewBuilder
    private var roster: some View {
        let rows = file.map { ShareAccess.rows(for: $0, in: filesModel, me: AppState.shared.account?.username ?? "") } ?? []

        VStack(alignment: .leading, spacing: 6) {
            sectionLabel("People with access (\(rows.count))")

            #if os(macOS)
                ScrollView {
                    rosterRows(rows)
                        .onGeometryChange(for: CGFloat.self) { proxy in
                            proxy.size.height
                        } action: { height in
                            rosterHeight = height
                        }
                }
                .scrollBounceBehavior(.basedOnSize)
                .frame(height: min(rosterHeight, 220))
            #else
                rosterRows(rows)
            #endif
        }
    }

    private func rosterRows(_ rows: [ShareAccess.Row]) -> some View {
        VStack(spacing: 0) {
            ForEach(rows) { row in
                accessRow(row)
            }
        }
    }

    private func accessRow(_ row: ShareAccess.Row) -> some View {
        HStack(spacing: 10) {
            HStack(spacing: 10) {
                Image(systemName: row.icon)
                    .font(.title3)
                    .foregroundStyle(row.isOwner ? Color.accentColor : Color.secondary)
                    .frame(width: 26)

                VStack(alignment: .leading, spacing: 1) {
                    Text(row.name)
                        .lineLimit(1)

                    Text(row.status)
                        .font(.footnote)
                        .foregroundStyle(row.isOwner ? Color.accentColor : Color.secondary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }
            .accessibilityElement(children: .combine)

            Spacer(minLength: 8)

            if row.removable {
                Button(role: .destructive) {
                    removePerson(row.username)
                } label: {
                    Image(systemName: "trash")
                        .padding(6)
                }
                .buttonStyle(.borderless)
                .accessibilityLabel("Stop sharing with \(row.username)")
                .help("Stop sharing with \(row.username)")
            }
        }
        .padding(.vertical, 5)
    }

    private var addPeople: some View {
        VStack(alignment: .leading, spacing: 6) {
            sectionLabel("Add people")

            usernameField

            if !staged.isEmpty {
                ChipFlow(spacing: 6) {
                    ForEach(staged) { person in
                        stagedChip(person)
                    }
                }
                .padding(.top, 2)
            }

            if !error.isEmpty {
                Text(error)
                    .foregroundStyle(.red)
                    .fontWeight(.bold)
            }

            sectionLabel("Access")
                .padding(.top, 8)

            Picker("Access", selection: $mode) {
                Text("Can edit").tag(ShareMode.write)
                Text("Can view").tag(ShareMode.read)
            }
            .pickerStyle(.segmented)
            .labelsHidden()

            if let summary {
                Text(summary)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 10)
            }

            Button(action: share) {
                Text("Share")
                    .frame(maxWidth: .infinity)
                    .opacity(sharing ? 0 : 1)
                    .overlay {
                        if sharing {
                            ProgressView()
                                .controlSize(.small)
                        }
                    }
            }
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.return, modifiers: .command)
            .disabled(!canShare)
            .padding(.top, 10)
        }
    }

    private func sectionLabel(_ title: String) -> some View {
        Text(title)
            .font(.callout)
            .foregroundStyle(.secondary)
    }

    private var usernameField: some View {
        let icon = lookupIcon

        return HStack(spacing: 8) {
            Image(systemName: icon.name)
                .foregroundStyle(icon.color)
                .help(icon.help)
                .accessibilityLabel(icon.help)

            TextField("Username", text: $query)
                .textFieldStyle(.plain)
                .autocapitalizationDisabled()
                .autocorrectionDisabled()
                .focused($queryFocused)
                .onSubmit(stageField)

            if !query.isEmpty {
                Button {
                    query = ""
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(.tertiary)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Clear")
            }
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 8).fill(Color.gray.opacity(0.15)))
    }

    private var lookupIcon: (name: String, color: Color, help: String) {
        let settling = !trimmedQuery.isEmpty && trimmedQuery != lookupFor

        switch lookup {
        case .found:
            return ("person.crop.circle.badge.checkmark", settling ? Color.secondary : Color.accentColor, "Lockbook user found")
        case .notFound:
            return ("person.crop.circle.badge.xmark", settling ? Color.secondary : Color.red, "No Lockbook user with that name")
        case let .failed(msg):
            return ("person.crop.circle.badge.xmark", settling ? Color.secondary : Color.red, msg)
        case .idle, .checking:
            return ("person.crop.circle", Color.secondary, "Username")
        }
    }

    private func stagedChip(_ person: StagedPerson) -> some View {
        let bad = person.lookup.isBad

        return HStack(spacing: 5) {
            if person.lookup == .checking {
                ProgressView()
                    .controlSize(.mini)
            }

            Text(person.username)
                .lineLimit(1)

            Button {
                staged.removeAll { $0.id == person.id }
                error = ""
            } label: {
                Image(systemName: "xmark")
                    .font(.caption2.weight(.bold))
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Remove \(person.username)")
        }
        .font(.callout)
        .foregroundStyle(bad ? Color.red : Color.primary)
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(Capsule().fill(bad ? Color.red.opacity(0.15) : Color.gray.opacity(0.15)))
        .help(person.lookup.message ?? person.username)
    }

    @ViewBuilder
    private var shareExternally: some View {
        if let file {
            ShareLink(
                item: DraggedFile(file: file, filesModel: filesModel),
                preview: SharePreview(file.name)
            ) {
                Label("Share externally", systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
        } else {
            Button {} label: {
                Label("Share externally", systemImage: "square.and.arrow.up")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
            .disabled(true)
        }
    }

    private var fieldReady: Bool {
        !trimmedQuery.isEmpty && trimmedQuery == lookupFor && lookup == .found
    }

    private var canShare: Bool {
        !sharing
            && (fieldReady || staged.contains { $0.lookup == .found })
            && !staged.contains { $0.lookup == .checking }
    }

    private var summary: AttributedString? {
        var names = staged.filter { $0.lookup == .found }.map(\.username)
        if fieldReady, !names.contains(where: { $0.caseInsensitiveCompare(trimmedQuery) == .orderedSame }) {
            names.append(trimmedQuery)
        }

        guard let file, !names.isEmpty else {
            return nil
        }

        return ShareSummary.sentence(subject: file.name, people: names, edit: mode == .write)
    }

    private func isKnown(_ name: String) -> Bool {
        knownUsernames.contains { $0.caseInsensitiveCompare(name) == .orderedSame }
    }

    private func remember(_ name: String) {
        if !isKnown(name) {
            knownUsernames.append(name)
        }
    }

    private func loadKnownUsernames() async {
        let names = await Task.detached(priority: .userInitiated) {
            (try? AppState.lb.knownUsernames().get()) ?? []
        }.value

        for name in names where name != "<unknown>" {
            remember(name)
        }
    }

    private func queryChanged() {
        if query.first?.isWhitespace == true {
            query = String(query.drop(while: \.isWhitespace))
            return
        }

        let (complete, remainder) = ShareSummary.splitUsernames(query)

        for name in complete {
            stage(name)
        }

        if !complete.isEmpty {
            query = remainder
        }

        error = ""

        let next = remainder.trimmingCharacters(in: .whitespaces)
        if next.isEmpty {
            lookup = .idle
            lookupFor = ""
        } else if isKnown(next) {
            lookup = .found
            lookupFor = next
        }
    }

    private func verifyField(_ name: String) async {
        guard !name.isEmpty, !(name == lookupFor && lookup.isSettled) else {
            return
        }

        if isKnown(name) {
            lookup = .found
            lookupFor = name
            return
        }

        try? await Task.sleep(for: .milliseconds(100))
        guard !Task.isCancelled else {
            return
        }

        let result = await Task.detached(priority: .userInitiated) {
            AppState.lb.usernameExists(username: name)
        }.value

        guard !Task.isCancelled, trimmedQuery == name else {
            return
        }

        lookup = UsernameLookup(result)
        lookupFor = name

        if lookup == .found {
            remember(name)
        }
    }

    private func settledLookup(for raw: String) -> UsernameLookup? {
        let name = raw.trimmingCharacters(in: .whitespaces)
        guard name == lookupFor, lookup == .found || lookup == .notFound else {
            return nil
        }

        return lookup
    }

    private func stage(_ raw: String, lookup settled: UsernameLookup? = nil) {
        let name = raw.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty, !staged.contains(where: { $0.id == name.lowercased() }) else {
            return
        }

        if let settled {
            staged.append(StagedPerson(username: name, lookup: settled))
            return
        }

        if isKnown(name) {
            staged.append(StagedPerson(username: name, lookup: .found))
            return
        }

        staged.append(StagedPerson(username: name, lookup: .checking))

        Task {
            let result = await Task.detached(priority: .userInitiated) {
                AppState.lb.usernameExists(username: name)
            }.value

            guard let idx = staged.firstIndex(where: { $0.id == name.lowercased() }) else {
                return
            }

            staged[idx].lookup = UsernameLookup(result)

            if staged[idx].lookup == .found {
                remember(name)
            }
        }
    }

    private func stageField() {
        let (complete, remainder) = ShareSummary.splitUsernames(query)

        for name in complete {
            stage(name)
        }

        stage(remainder, lookup: settledLookup(for: remainder))

        query = ""
        lookup = .idle
        lookupFor = ""
        error = ""
        queryFocused = true
    }

    private func share() {
        if !trimmedQuery.isEmpty {
            stage(trimmedQuery, lookup: settledLookup(for: trimmedQuery))
            query = ""
            lookup = .idle
            lookupFor = ""
        }

        guard !staged.isEmpty else {
            error = "Enter a username"
            return
        }

        guard !staged.contains(where: { $0.lookup == .checking }) else {
            error = "Still checking usernames…"
            return
        }

        let targets = staged.filter { $0.lookup == .found }.map(\.username)
        guard !targets.isEmpty else {
            error = staged.contains { $0.lookup == .notFound }
                ? "No Lockbook user with that name"
                : "Could not verify those usernames"
            return
        }

        sharing = true
        error = ""

        let id = id
        let mode = mode

        Task {
            let failures = await Task.detached(priority: .userInitiated) {
                targets.compactMap { name -> (String, LbError)? in
                    guard case let .failure(err) = AppState.lb.shareFile(id: id, username: name, mode: mode) else {
                        return nil
                    }

                    return (name, err)
                }
            }.value

            sharing = false
            finishSharing(targets: targets, failures: failures)
        }
    }

    private func finishSharing(targets: [String], failures: [(String, LbError)]) {
        let shared = targets.filter { name in !failures.contains { $0.0 == name } }

        staged.removeAll { person in shared.contains { $0.lowercased() == person.id } }

        for (name, err) in failures {
            if let idx = staged.firstIndex(where: { $0.id == name.lowercased() }) {
                staged[idx].lookup = err.code == .usernameNotFound ? .notFound : .failed(err.msg)
            }
        }

        if failures.isEmpty {
            error = ""
        } else if shared.isEmpty {
            error = failures[0].1.msg
        } else {
            error = "Shared with \(shared.count); failed for \(failures.map { $0.0 }.joined(separator: ", "))"
        }

        if !shared.isEmpty {
            workspaceInput.requestSync()
            filesModel.loadFiles()
        }

        #if os(macOS)
            queryFocused = true
        #endif
    }

    private func removePerson(_ name: String) {
        switch AppState.lb.unshareFile(id: id, username: name) {
        case .success:
            error = ""
            workspaceInput.requestSync()
            filesModel.loadFiles()
        case let .failure(err):
            error = err.msg
        }
    }

    private func copyLink() {
        ClipboardHelper.copyFileLink(id)
        linkCopied = true

        copyResetTask?.cancel()
        copyResetTask = Task {
            try? await Task.sleep(for: .seconds(1.5))

            if !Task.isCancelled {
                linkCopied = false
            }
        }
    }
}

enum UsernameLookup: Equatable {
    case idle
    case checking
    case found
    case notFound
    case failed(String)

    init(_ result: Result<Bool, LbError>) {
        switch result {
        case .success(true): self = .found
        case .success(false): self = .notFound
        case let .failure(err): self = .failed(err.msg)
        }
    }

    var isSettled: Bool {
        switch self {
        case .found, .notFound, .failed: true
        case .idle, .checking: false
        }
    }

    var isBad: Bool {
        switch self {
        case .notFound, .failed: true
        case .idle, .checking, .found: false
        }
    }

    var message: String? {
        switch self {
        case .notFound: "No Lockbook user with that name"
        case let .failed(msg): msg
        case .idle, .checking, .found: nil
        }
    }
}

struct StagedPerson: Identifiable, Equatable {
    var username: String
    var lookup: UsernameLookup

    var id: String {
        username.lowercased()
    }
}

enum ShareAccess {
    struct Row: Identifiable, Equatable {
        let username: String
        let mode: ShareMode?
        let via: String?
        let isMe: Bool
        let removable: Bool

        var id: String {
            username.lowercased()
        }

        var isOwner: Bool {
            mode == nil
        }

        var name: String {
            isMe ? "you" : username
        }

        var status: String {
            let role = switch mode {
            case .none: "Owner"
            case .write: "Can edit"
            case .read: "Can view"
            }

            guard let via else {
                return role
            }

            return "\(role) · Via \(via)"
        }

        var icon: String {
            switch mode {
            case .none: "person.circle"
            case .write: "pencil.circle"
            case .read: "eye.circle"
            }
        }

        fileprivate var band: Int {
            switch mode {
            case .none: 0
            case .write: 1
            case .read: 2
            }
        }
    }

    static func rows(for file: File, in filesModel: FilesModel, me: String) -> [Row] {
        rows(for: file, files: filesModel.idsToFiles, me: me) { folder in
            "/" + (filesModel.ancestors(of: folder) + [folder]).map { $0.name + "/" }.joined()
        }
    }

    static func rows(
        for file: File, files: [UUID: File], me: String, path: (File) -> String
    ) -> [Row] {
        var grants: [String: (share: Share, via: File?)] = [:]
        var order: [String] = []

        func grant(_ share: Share, via: File?) {
            let key = share.with.lowercased()

            if let existing = grants[key], rank(existing.share.mode) >= rank(share.mode) {
                return
            }

            if grants[key] == nil {
                order.append(key)
            }

            grants[key] = (share, via)
        }

        for share in file.shares {
            grant(share, via: nil)
        }

        var current = file
        var visited: Set<UUID> = [file.id]
        while !current.isRoot,
              let parent = files[current.parent],
              visited.insert(parent.id).inserted
        {
            for share in parent.shares {
                grant(share, via: parent)
            }

            current = parent
        }

        let isMe = { (name: String) in !me.isEmpty && name.caseInsensitiveCompare(me) == .orderedSame }
        let ownerKey = file.owner.lowercased()
        let canEdit = isMe(file.owner) || grants[me.lowercased()]?.share.mode == .write

        var rows = [Row(username: file.owner, mode: nil, via: nil, isMe: isMe(file.owner), removable: false)]

        for key in order where key != ownerKey {
            guard let grant = grants[key] else {
                continue
            }

            rows.append(Row(
                username: grant.share.with,
                mode: grant.share.mode,
                via: grant.via.map(path),
                isMe: isMe(grant.share.with),
                removable: grant.via == nil && !isMe(grant.share.with) && canEdit
            ))
        }

        return rows.sorted {
            ($0.band, $0.name.lowercased()) < ($1.band, $1.name.lowercased())
        }
    }

    private static func rank(_ mode: ShareMode) -> Int {
        switch mode {
        case .write: 2
        case .read: 1
        }
    }
}

enum ShareSummary {
    static func sentence(subject: String, people: [String], edit: Bool) -> AttributedString {
        var text = bold(subject)
        text += AttributedString(" will be shared with ")
        text += names(people)
        text += AttributedString(" with ")
        text += bold(edit ? "edit access" : "view access")
        text += AttributedString(".")
        return text
    }

    static func names(_ people: [String]) -> AttributedString {
        guard people.count <= 4 else {
            return bold("\(people.count) people")
        }

        var text = AttributedString()
        for (idx, name) in people.enumerated() {
            if idx > 0 {
                let separator = people.count == 2 ? " and " : idx == people.count - 1 ? ", and " : ", "
                text += AttributedString(separator)
            }

            text += bold(name)
        }

        return text
    }

    static func splitUsernames(_ query: String) -> (complete: [String], remainder: String) {
        guard query.contains(",") else {
            return ([], query)
        }

        let parts = query.split(separator: ",", omittingEmptySubsequences: false).map(String.init)
        let complete = { (parts: ArraySlice<String>) in
            parts.map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        }

        if query.trimmingCharacters(in: .whitespaces).hasSuffix(",") {
            return (complete(parts[...]), "")
        }

        return (complete(parts.dropLast()), String(parts.last?.drop(while: \.isWhitespace) ?? ""))
    }

    private static func bold(_ string: String) -> AttributedString {
        var text = AttributedString(string)
        text.inlinePresentationIntent = .stronglyEmphasized
        return text
    }
}

private struct ChipFlow: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let frames = arrange(subviews, width: proposal.width ?? .infinity)
        let width = frames.map(\.maxX).max() ?? 0
        let height = frames.map(\.maxY).max() ?? 0

        return CGSize(width: width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let frames = arrange(subviews, width: bounds.width)

        for (subview, frame) in zip(subviews, frames) {
            subview.place(
                at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                proposal: ProposedViewSize(frame.size)
            )
        }
    }

    private func arrange(_ subviews: Subviews, width: CGFloat) -> [CGRect] {
        var frames: [CGRect] = []
        var x: CGFloat = 0
        var y: CGFloat = 0
        var lineHeight: CGFloat = 0

        for subview in subviews {
            var size = subview.sizeThatFits(.unspecified)
            size.width = min(size.width, width)

            if x > 0, x + size.width > width {
                x = 0
                y += lineHeight + spacing
                lineHeight = 0
            }

            frames.append(CGRect(origin: CGPoint(x: x, y: y), size: size))
            x += size.width + spacing
            lineHeight = max(lineHeight, size.height)
        }

        return frames
    }
}

#Preview {
    Color.clear
        .sheet(isPresented: .constant(true)) {
            ShareFileSheet(id: (AppState.lb as! MockLb).file1.id)
        }
        .environment(FilesModel.preview)
        .environment(WorkspaceInputState.preview)
}
