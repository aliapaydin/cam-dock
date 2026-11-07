package com.camdock.phone

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.graphics.Color
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.View
import android.view.WindowManager
import android.webkit.JavascriptInterface
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import org.json.JSONArray
import org.json.JSONObject
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.HttpURLConnection
import java.net.InetSocketAddress
import java.net.SocketTimeoutException
import java.net.URL
import kotlin.concurrent.thread

/**
 * Cam Dock Telefon: tam ekran bir WebView. Arayüzü bilgisayardaki Cam Dock
 * gönderir; bu yüzden yeni özellikler için uygulamayı güncellemek gerekmez.
 *
 * - Wi-Fi: aynı ağdaki Cam Dock'ları UDP yayınıyla bulur (bağlantı noktası 47801).
 * - USB: Cam Dock kablo takılınca ADB ile uygulamayı host=127.0.0.1 ve anahtarla başlatır.
 */
class MainActivity : Activity() {
    private lateinit var web: WebView
    private val prefs by lazy { getSharedPreferences("camdock", Context.MODE_PRIVATE) }
    private val ui = Handler(Looper.getMainLooper())
    /** Şu an bağlı (ya da bağlanmaya çalışılan) adres */
    private var current: String? = null
    private var onConnectPage = false

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        web = WebView(this)
        web.setBackgroundColor(Color.parseColor("#0B0E14"))
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.settings.mediaPlaybackRequiresUserGesture = false
        web.settings.allowFileAccess = true
        web.isVerticalScrollBarEnabled = false
        web.isHorizontalScrollBarEnabled = false
        web.overScrollMode = View.OVER_SCROLL_NEVER
        web.addJavascriptInterface(Bridge(), "Android")
        web.webViewClient = object : WebViewClient() {
            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (request.isForMainFrame) showConnect()
            }
        }
        setContentView(web)
        immersive()
        handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handle(intent)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) immersive()
    }

    @Suppress("DEPRECATION")
    private fun immersive() {
        window.decorView.systemUiVisibility = (View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
            or View.SYSTEM_UI_FLAG_FULLSCREEN
            or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
            or View.SYSTEM_UI_FLAG_LAYOUT_STABLE
            or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
            or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN)
    }

    private fun handle(i: Intent?) {
        val host = i?.getStringExtra("host")
        if (host != null) {
            val port = i.getIntExtra("port", 47800)
            open(host, port, i.getStringExtra("token"), remember = false)
            return
        }
        val last = prefs.getString("host", null)
        if (last != null) open(last, prefs.getInt("port", 47800), null, remember = false)
        else showConnect()
    }

    private fun open(host: String, port: Int, token: String?, remember: Boolean) {
        if (remember) prefs.edit().putString("host", host).putInt("port", port).apply()
        current = host
        onConnectPage = false
        val q = if (token != null) "?token=$token" else ""
        web.loadUrl("http://$host:$port/$q")
    }

    private fun showConnect() {
        if (onConnectPage) return
        onConnectPage = true
        web.loadUrl("file:///android_asset/connect.html")
    }

    private fun js(code: String) = ui.post { web.evaluateJavascript(code, null) }

    /** Aynı ağdaki Cam Dock'ları dinler (yaklaşık 3 sn). */
    private fun discover() = thread {
        val found = LinkedHashMap<String, JSONObject>()
        val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val lock = wifi.createMulticastLock("camdock").apply { setReferenceCounted(false) }
        try {
            lock.acquire()
            DatagramSocket(null).use { s ->
                s.reuseAddress = true
                s.broadcast = true
                s.bind(InetSocketAddress(47801))
                s.soTimeout = 700
                val buf = ByteArray(2048)
                val end = System.currentTimeMillis() + 3000
                while (System.currentTimeMillis() < end) {
                    try {
                        val p = DatagramPacket(buf, buf.size)
                        s.receive(p)
                        val o = JSONObject(String(p.data, 0, p.length))
                        if (o.optInt("camdock") != 1) continue
                        val ip = p.address.hostAddress ?: continue
                        o.put("ip", ip)
                        found[ip] = o
                    } catch (_: SocketTimeoutException) {
                    } catch (_: Exception) {
                    }
                }
            }
        } catch (_: Exception) {
        } finally {
            try { lock.release() } catch (_: Exception) {}
        }
        val arr = JSONArray()
        found.values.forEach { arr.put(it) }
        js("window.onFound && window.onFound($arr)")
    }

    /** Bilinen adres yanıt veriyor mu (1,5 sn). */
    private fun reachable(host: String, port: Int): Boolean = try {
        val c = URL("http://$host:$port/ping").openConnection() as HttpURLConnection
        c.connectTimeout = 1500
        c.readTimeout = 1500
        val ok = c.responseCode == 200
        c.disconnect()
        ok
    } catch (_: Exception) {
        false
    }

    inner class Bridge {
        @JavascriptInterface fun deviceName(): String {
            val m = Build.MODEL ?: "Android"
            val maker = Build.MANUFACTURER ?: ""
            return if (m.lowercase().startsWith(maker.lowercase())) m else "$maker $m".trim()
        }

        @JavascriptInterface fun version(): String = BuildConfigVersion.name(this@MainActivity)

        @JavascriptInterface fun discover() { this@MainActivity.discover() }

        @JavascriptInterface fun connect(host: String, port: Int) {
            ui.post { open(host.trim(), port, null, remember = true) }
        }

        @JavascriptInterface fun lastHost(): String = prefs.getString("host", "") ?: ""

        /** Bilinen bilgisayar yeniden açıldı mı? Açıksa ona geç. */
        @JavascriptInterface fun retryLast() {
            val host = prefs.getString("host", null) ?: return
            val port = prefs.getInt("port", 47800)
            thread { if (reachable(host, port)) ui.post { open(host, port, null, remember = false) } }
        }

        /** Sayfa yüklendi ve bilgisayara bağlandı. */
        @JavascriptInterface fun connected() {
            val host = current ?: return
            if (host != "127.0.0.1") prefs.edit().putString("host", host).apply()
        }

        /** Bağlantı koptu: USB'deysek Wi-Fi adresini dene. */
        @JavascriptInterface fun connectionLost() {
            val cur = current ?: return
            val lan = prefs.getString("host", null)
            if (cur == "127.0.0.1" && lan != null && lan != cur) {
                val port = prefs.getInt("port", 47800)
                thread { if (reachable(lan, port)) ui.post { open(lan, port, null, remember = false) } }
            }
        }

        @JavascriptInterface fun chooseOther() {
            ui.post {
                prefs.edit().remove("host").apply()
                current = null
                onConnectPage = false
                showConnect()
            }
        }
    }
}

object BuildConfigVersion {
    fun name(ctx: Context): String = try {
        ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName ?: ""
    } catch (_: Exception) {
        ""
    }
}
