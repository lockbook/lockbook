package app.lockbook.screen

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Bundle
import android.text.method.ScrollingMovementMethod
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.activityViewModels
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.RecyclerView
import app.lockbook.R
import app.lockbook.databinding.ActivityDebugInfoBinding
import app.lockbook.databinding.ActivityMainScreenBinding
import app.lockbook.databinding.FragmentShareFileBinding
import app.lockbook.model.AlertModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import net.lockbook.Lb
import net.lockbook.LbError
import java.lang.ref.WeakReference

class DebugInfoActivity: AppCompatActivity() {
    private lateinit var binding: ActivityDebugInfoBinding
    private val alertModel by lazy {
        AlertModel(WeakReference(this))
    }

    var debugInfo: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityDebugInfoBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.debugInfoToolbar.setNavigationOnClickListener {
            finish()
        }

        binding.debugInfoToolbar.setOnMenuItemClickListener {
            if(it.itemId == R.id.menu_debug_info_copy) {
                val clipBoard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                clipBoard.setPrimaryClip(ClipData.newPlainText("lockbook debug info", debugInfo ?: ""))
            }

            true
        }

        binding.debugInfoText.apply {
            movementMethod = ScrollingMovementMethod()
            setTextIsSelectable(true)
        }


        lifecycleScope.launch(Dispatchers.IO) {
            try {
                val osInfo = "${Build.VERSION.RELEASE}.${Build.VERSION.SDK_INT}"
                debugInfo = Lb.getDebugInfo(osInfo)
                withContext(Dispatchers.Main) {
                    binding.debugInfoText.text = debugInfo
                    binding.debugInfoProgressBar.visibility = View.GONE
                    binding.debugInfoText.visibility = View.VISIBLE
                }
            } catch (err: LbError) {
                alertModel.notifyError(err)
            }
        }
    }
}
