package c.cyrio.android.fragment

import android.content.Context
import android.os.Bundle
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import androidx.fragment.app.Fragment
import com.google.android.material.materialswitch.MaterialSwitch
import c.cyrio.android.MainActivity
import c.cyrio.android.R

/**
 * 设置页 — MD3 风格：圆角列表项 + MaterialSwitch + 关于入口
 *
 * 偏好使用 SharedPreferences 持久化
 */
class SettingsFragment : Fragment() {

    override fun onCreateView(
        inflater: LayoutInflater,
        container: ViewGroup?,
        savedInstanceState: Bundle?
    ): View {
        return inflater.inflate(R.layout.fragment_settings, container, false)
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        val prefs = requireContext().getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

        // 拼音转换开关 — MD3 MaterialSwitch
        val swSlug = view.findViewById<MaterialSwitch>(R.id.sw_default_slug)
        swSlug.isChecked = prefs.getBoolean(KEY_DEFAULT_SLUG, false)
        swSlug.setOnCheckedChangeListener { _, checked ->
            prefs.edit().putBoolean(KEY_DEFAULT_SLUG, checked).apply()
        }

        // 去词开关 — MD3 MaterialSwitch
        val swStrip = view.findViewById<MaterialSwitch>(R.id.sw_default_strip)
        swStrip.isChecked = prefs.getBoolean(KEY_DEFAULT_STRIP, false)
        swStrip.setOnCheckedChangeListener { _, checked ->
            prefs.edit().putBoolean(KEY_DEFAULT_STRIP, checked).apply()
        }

        // 关于 — 点击展示关于页（覆盖在设置页之上）
        val itemAbout = view.findViewById<LinearLayout>(R.id.item_about)
        itemAbout.setOnClickListener {
            (activity as? MainActivity)?.showAboutFragment()
        }
    }

    companion object {
        private const val PREFS_NAME = "cyrio_settings"
        private const val KEY_DEFAULT_SLUG = "default_slug"
        private const val KEY_DEFAULT_STRIP = "default_strip"

        /** 读取默认拼音转换设置（供 UploadFragment 使用） */
        fun getDefaultSlug(context: Context): Boolean {
            return context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                .getBoolean(KEY_DEFAULT_SLUG, false)
        }

        /** 读取默认去词设置（供 UploadFragment 使用） */
        fun getDefaultStrip(context: Context): Boolean {
            return context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
                .getBoolean(KEY_DEFAULT_STRIP, false)
        }
    }
}
