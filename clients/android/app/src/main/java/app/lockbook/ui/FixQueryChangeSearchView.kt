package app.lockbook.ui

import android.content.Context
import android.util.AttributeSet
import androidx.annotation.AttrRes
import androidx.appcompat.widget.SearchView

class FixQueryChangeSearchView : SearchView {
    var shouldIgnoreQueryChange = false
        private set

    constructor(context: Context) : super(context)

    constructor(context: Context, attrs: AttributeSet?) : super(context, attrs)

    constructor(context: Context, attrs: AttributeSet?, @AttrRes defStyleAttr: Int) : super(
        context, attrs, defStyleAttr
    )

    override fun setIconified(iconify: Boolean) {
        shouldIgnoreQueryChange = true
        super.setIconified(iconify)
        shouldIgnoreQueryChange = false
    }

    override fun onActionViewCollapsed() {
        shouldIgnoreQueryChange = true
        super.onActionViewCollapsed()
        shouldIgnoreQueryChange = false
    }

    override fun onActionViewExpanded() {
        shouldIgnoreQueryChange = true
        super.onActionViewExpanded()
        shouldIgnoreQueryChange = false
    }
}
