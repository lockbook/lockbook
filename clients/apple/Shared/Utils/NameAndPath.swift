
extension String {
    func nameAndPath() -> (String, String) {
        let components = self.split(separator: "/")
        
        let name = String(components.last ?? "ERROR")
        let path = components.dropLast().joined(separator: "/")
        
        return (name, path.isEmpty ? "/" : path)
    }
}
