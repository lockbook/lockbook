import XCTest
@testable import SwiftLockbookCore

final class DbMigrationTests: SLCTest {
    func testSimple() throws {
        assertSuccess(core.api.getState()) { $0 == .Empty }

        assertSuccess(try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()))

        assertSuccess(core.api.getState()) { $0 == .ReadyToUse }

        assertSuccess(core.api.synchronize())

        assertSuccess(core.api.getState()) { $0 == .ReadyToUse }
    }

    func testMustGetStateFirst() throws {
        assertSuccess(try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()))

        assertSuccess(core.api.getState()) { $0 == .StateRequiresClearing }

        assertSuccess(core.api.synchronize())

        assertSuccess(core.api.getState()) { $0 == .StateRequiresClearing }
    }

    func testTryMigrateWhenReadyToUse() throws {
        assertSuccess(core.api.getState()) { $0 == .Empty }

        assertSuccess(try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()))

        assertSuccess(core.api.getState()) { $0 == .ReadyToUse }

        assertSuccess(core.api.migrateState())
    }

    func testTryMigrateWhenRequiresCleaning() throws {
        assertSuccess(try core.api.createAccount(username: randomUsername(), apiLocation: systemApiLocation()))

        assertSuccess(core.api.getState()) { $0 == .StateRequiresClearing }

        assertFailure(core.api.migrateState()) { $0 == .init(.StateRequiresCleaning) }
    }
}
