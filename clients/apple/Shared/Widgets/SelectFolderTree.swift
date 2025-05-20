import SwiftUI

struct SelectFolderNestedList<T: Identifiable & Comparable, V: View>: View {
    let node: WithChild<T>
    let row: (T) -> V
    @State var expanded: Bool
    
    init(node: WithChild<T>, row: @escaping (T) -> V) {
        self.node = node
        self.row = row
        self._expanded = .init(initialValue: node.level < 3)
    }
    
    var body: some View {
        VStack(spacing: 10) {
            HStack {
                row(node.value)
                Spacer()
                if (!node.children.isEmpty) {
                    Image(systemName: "chevron.right")
                        .rotationEffect(expanded ? .degrees(90) : .zero)
                        .onTapGesture {
                            withAnimation {
                                expanded.toggle()
                            }
                        }
                        .padding(.trailing)
                }
            }
            if (expanded) {
                ForEach(node.children) { c in
                    SelectFolderNestedList(node: c, row: row).padding(.leading, 30)
                }
            }
        }
    }
}

struct WithChild<T>: Identifiable & Comparable where T: Identifiable & Comparable {
    static func < (lhs: WithChild<T>, rhs: WithChild<T>) -> Bool {
        lhs.value < rhs.value
    }
    
    var id: T.ID {
        value.id
    }
    
    let value: T
    let children: [WithChild<T>]
    let level: Int
    
    init(_ value: T, _ children: [WithChild<T>], level: Int = 0) {
        self.value = value
        self.children = children
        self.level = level
    }
    
    init(_ value: T, _ ts: [T], _ desc: (T, T) -> Bool, level: Int = 0) {
        self.value = value
        self.level = level
        self.children = ts.compactMap {
            if (desc(value, $0)) {
                return Self($0, ts, desc, level: level+1)
            } else {
                return nil
            }
        }.sorted(by: { $0 < $1 })
    }
}
