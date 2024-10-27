import Bridge

public struct LbError: Error {
    let code: LbEC
    let msg: String
    let trace: String
    
    init(_ err: LbFfiErr) {
        self.code = err.code
        self.msg = String(cString: err.msg)
        self.trace = String(cString: err.trace)
    }
}
