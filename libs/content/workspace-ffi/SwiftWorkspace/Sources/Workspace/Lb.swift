import Bridge

public class Lb {
    public var lb: OpaquePointer? = nil
    
    func toLbErr(_ err: UnsafeMutablePointer<LbFfiErr>) -> LbError {
        return LbError(code: err.pointee.code, msg: String(cString: err.pointee.msg), trace: String(cString: err.pointee.trace))
    }
    
    public func start(writablePath: String, logs: Bool) -> Result<Void, LbError> {
        let res = lb_init(writablePath, logs)
                
        if let err = res.err {
            let err = toLbErr(err)
            lb_free_init(res)
            return .failure(err)
        }

        lb = res.lb
        lb_free_init(res)
        return .success(())
    }
    
    public func createAccount(username: String, apiUrl: String, welcomeDoc: Bool) -> Result<Account, LbError> {
        
    }
}


public struct LbError: Error {
    let code: LbEC
    let msg: String
    let trace: String
}
