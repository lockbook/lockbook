package app.lockbook.model

import net.lockbook.Account
import net.lockbook.Lb

internal object AccountStore {
    @Volatile
    private var account: Account? = null

    val username: String? get() = account?.username

    @Synchronized
    fun load(): Account = account ?: Lb.getAccount().also { account = it }

    @Synchronized
    fun set(account: Account) {
        this.account = account
    }

    @Synchronized
    fun clear() {
        account = null
    }
}
