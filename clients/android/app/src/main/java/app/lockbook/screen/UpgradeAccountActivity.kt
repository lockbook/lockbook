package app.lockbook.screen

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.Fragment
import androidx.viewpager2.adapter.FragmentStateAdapter
import app.lockbook.R
import app.lockbook.databinding.ActivityUpgradeAccountBinding
import app.lockbook.databinding.FragmentMonthlyPremiumInfoBinding
import app.lockbook.databinding.FragmentYearlyPremiumInfoBinding
import com.google.android.material.tabs.TabLayoutMediator
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job

class UpgradeAccountActivity: AppCompatActivity() {

    private var _binding: ActivityUpgradeAccountBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    val binding get() = _binding!!


    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        _binding = ActivityUpgradeAccountBinding.inflate(layoutInflater)
        setContentView(binding.root)

        binding.upgradeAccountViewpager.adapter = BillingFragmentAdapter(this)

        TabLayoutMediator(
            binding.switchAccountTierTabLayout,
            binding.upgradeAccountViewpager
        ) { tabLayout, position ->
            tabLayout.text = if (position == 0) {
                resources.getText(R.string.yearly_plan)
            } else {
                resources.getText(R.string.monthly_plan)
            }
        }.attach()
    }

    inner class BillingFragmentAdapter(activity: AppCompatActivity) :
        FragmentStateAdapter(activity) {
        override fun getItemCount(): Int = 2

        override fun createFragment(position: Int): Fragment {
            return if (position == 0) {
                YearlyPremiumFragment()
            } else {
                MonthlyPremiumFragment()
            }
        }
    }
}

class YearlyPremiumFragment : Fragment() {
    private var _createBinding: FragmentYearlyPremiumInfoBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val createBinding get() = _createBinding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _createBinding = FragmentYearlyPremiumInfoBinding.inflate(inflater, container, false)


        return createBinding.root
    }
}

class MonthlyPremiumFragment : Fragment() {
    private var _createBinding: FragmentMonthlyPremiumInfoBinding? = null

    // This property is only valid between onCreateView and
    // onDestroyView.
    private val createBinding get() = _createBinding!!

    private var job = Job()
    private val uiScope = CoroutineScope(Dispatchers.Main + job)

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        _createBinding = FragmentMonthlyPremiumInfoBinding.inflate(inflater, container, false)


        return createBinding.root
    }
}
