package c.cyrio.android

import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.widget.FrameLayout
import androidx.appcompat.app.AppCompatActivity
import androidx.fragment.app.Fragment
import com.google.android.material.bottomnavigation.BottomNavigationView
import c.cyrio.android.fragment.AboutFragment
import c.cyrio.android.fragment.*
import c.cyrio.android.usb.CyrioUsbHelper
import c.cyrio.android.util.CyrioDeviceManager

/**
 * 主 Activity — 管理 Fragment 切换和底部导航
 *
 * 底部导航 5 个 Tab：歌曲 / 歌单 / 上传传输 / 设备 / 设置
 * 「关于」入口在设置页内，点击后以独立 Fragment 覆盖展示
 *
 * 传输进行时：除"上传传输"外的所有 Tab 置灰禁用
 */
class MainActivity : AppCompatActivity() {

    private val TAG = "MainActivity"

    private lateinit var bottomNav: BottomNavigationView
    private lateinit var fragmentContainer: FrameLayout

    /** 缓存的 Fragment 实例（避免重复创建） */
    private val fragments = mutableMapOf<Int, Fragment>()

    /** 当前显示的 Tab ID */
    private var currentTabId = R.id.nav_songs

    /** 是否处于传输中状态（传输时禁用其他 Tab） */
    private var transferring = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        bottomNav = findViewById(R.id.bottom_nav)
        fragmentContainer = findViewById(R.id.fragment_container)

        // 初始化 USB Helper（保存 Context + JavaVM）
        CyrioUsbHelper.init(this)

        // 设置底部导航监听
        bottomNav.setOnItemSelectedListener { item ->
            if (transferring && item.itemId != R.id.nav_upload) {
                // 传输中只允许切换到"上传传输"Tab
                false
            } else {
                switchFragment(item.itemId)
                true
            }
        }

        // 默认选中歌曲 Tab
        if (savedInstanceState == null) {
            bottomNav.selectedItemId = R.id.nav_songs
        }

        // 启动时自动尝试连接设备（UI 先显示"连接中..."转圈）
        if (!CyrioDeviceManager.isConnected && !CyrioDeviceManager.isConnecting) {
            Log.i(TAG, "Auto-connecting device on startup...")
            CyrioDeviceManager.openDevice { success ->
                Log.i(TAG, "Auto-connect result: $success")
            }
        }

        // 处理 USB 设备插入 Intent
        handleUsbIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleUsbIntent(intent)
    }

    /**
     * 处理 USB 设备插入 Intent
     * 当 Rio 设备插入时自动切换到"设备"Tab 开始扫描
     */
    private fun handleUsbIntent(intent: Intent?) {
        if (intent?.action == android.hardware.usb.UsbManager.ACTION_USB_DEVICE_ATTACHED) {
            bottomNav.selectedItemId = R.id.nav_device
        }
    }

    /**
     * 切换 Fragment
     * 使用 show/hide 而非 replace，保持各 Fragment 状态
     */
    private fun switchFragment(tabId: Int) {
        currentTabId = tabId

        val fragment = getOrCreateFragment(tabId)
        val transaction = supportFragmentManager.beginTransaction()

        // 隐藏所有 Fragment
        for ((id, frag) in fragments) {
            if (id != tabId && frag.isAdded) {
                transaction.hide(frag)
            }
        }

        // 显示目标 Fragment
        if (fragment.isAdded) {
            transaction.show(fragment)
        } else {
            transaction.add(R.id.fragment_container, fragment)
        }

        transaction.commitAllowingStateLoss()
    }

    /** 获取或创建指定 Tab 的 Fragment */
    private fun getOrCreateFragment(tabId: Int): Fragment {
        return fragments.getOrPut(tabId) {
            when (tabId) {
                R.id.nav_songs -> SongsFragment()
                R.id.nav_playlists -> PlaylistsFragment()
                R.id.nav_upload -> UploadFragment()
                R.id.nav_device -> DeviceFragment()
                R.id.nav_settings -> SettingsFragment()
                else -> SongsFragment()
            }
        }
    }

    /**
     * 设置传输状态
     * 传输中时禁用除"上传传输"外的所有 Tab
     *
     * 由 UploadFragment 调用
     */
    fun setTransferring(isTransferring: Boolean) {
        transferring = isTransferring

        val menu = bottomNav.menu
        for (i in 0 until menu.size()) {
            val item = menu.getItem(i)
            if (item.itemId != R.id.nav_upload) {
                item.isEnabled = !isTransferring
            }
        }

        // 传输中自动切换到上传 Tab
        if (isTransferring && currentTabId != R.id.nav_upload) {
            bottomNav.selectedItemId = R.id.nav_upload
        }
    }

    /** 切换到指定 Tab（供 Fragment 调用，如设置页跳转关于页） */
    fun switchToTab(tabId: Int) {
        bottomNav.selectedItemId = tabId
    }

    /**
     * 展示「关于」页（覆盖在设置页之上，按返回键回到设置页）
     * 由 SettingsFragment 的「关于」入口调用
     */
    fun showAboutFragment() {
        val aboutFragment = AboutFragment()
        supportFragmentManager.beginTransaction()
            .add(R.id.fragment_container, aboutFragment, "about")
            .addToBackStack("about")
            .commitAllowingStateLoss()
    }
}
