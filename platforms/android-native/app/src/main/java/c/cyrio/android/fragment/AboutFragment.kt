package c.cyrio.android.fragment

import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.TextView
import androidx.fragment.app.Fragment
import c.cyrio.android.R

/**
 * 关于页 — 显示应用名称、版本号、描述和技术栈
 */
class AboutFragment : Fragment() {

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return inflater.inflate(R.layout.fragment_about, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        // 显示版本号
        val textVersion = view.findViewById<TextView>(R.id.text_version)
        try {
            val pkgInfo = requireContext().packageManager.getPackageInfo(
                requireContext().packageName, 0
            )
            textVersion.text = "版本 ${pkgInfo.versionName}"
        } catch (e: Exception) {
            textVersion.text = getString(R.string.about_version)
        }
    }
}
