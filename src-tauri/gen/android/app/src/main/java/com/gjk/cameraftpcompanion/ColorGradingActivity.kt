/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.os.Bundle
import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import com.gjk.cameraftpcompanion.bridges.ColorGradingJniBridge
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.FileInputStream
import java.lang.ref.WeakReference
import java.net.URLDecoder

class ColorGradingActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ColorGradingActivity"
    }

    internal var webView: WebView? = null

    @Volatile
    internal var previewFilePath: String? = null

    @Volatile
    internal var isSessionActive = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        onBackPressedDispatcher.addCallback(this, object : androidx.activity.OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (isSessionActive) {
                    endPreviewSession()
                }
                finish()
            }
        })

        val filePath = intent.getStringExtra("filePath")
        if (filePath == null) {
            Log.e(TAG, "No filePath provided")
            finish()
            return
        }

        WindowCompat.setDecorFitsSystemWindows(window, false)

        webView = WebView(this).apply {
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = false
            settings.allowFileAccess = false

            webViewClient = object : WebViewClient() {
                override fun shouldInterceptRequest(
                    view: WebView, request: WebResourceRequest
                ): WebResourceResponse? {
                    if (request.url.scheme == "preview" && request.url.host == "latest") {
                        val path = previewFilePath
                        if (path != null) {
                            val file = File(path)
                            if (file.exists()) {
                                return WebResourceResponse(
                                    "image/jpeg", null, 200, "OK",
                                    mapOf("Content-Length" to file.length().toString()),
                                    FileInputStream(file)
                                )
                            }
                        }
                        return WebResourceResponse(
                            "image/jpeg", null, 404, "Not Found",
                            emptyMap(), null
                        )
                    }
                    return super.shouldInterceptRequest(view, request)
                }
            }

            addJavascriptInterface(
                NativeColorGradingPreviewBridge(this@ColorGradingActivity, filePath),
                "NativeBridge"
            )
            loadUrl("file:///android_asset/color_grading_preview.html")
        }

        val container = FrameLayout(this).apply {
            fitsSystemWindows = true
            addView(webView, FrameLayout.LayoutParams(
                FrameLayout.LayoutParams.MATCH_PARENT,
                FrameLayout.LayoutParams.MATCH_PARENT
            ))
        }
        setContentView(container)
    }

    override fun onDestroy() {
        // Unconditionally end the preview session — covers both the case where
        // a session is active AND the case where beginPreview is still decoding
        // (isSessionActive is set to true only after successful decode).
        if (isSessionActive) {
            endPreviewSession()
        } else {
            // No active session yet, but a beginPreview thread might be running.
            // end() is a no-op when there's no session, so this is safe.
            Thread { ColorGradingJniBridge.endPreview() }.start()
        }
        webView?.let {
            (it.parent as? android.view.ViewGroup)?.removeView(it)
            it.destroy()
        }
        webView = null
        super.onDestroy()
    }

    internal fun endPreviewSession() {
        isSessionActive = false
        previewFilePath = null
        Thread { ColorGradingJniBridge.endPreview() }.start()
    }

    internal fun extractFilePathFromUrl(url: String): String? {
        val prefix = "http://image-preview.localhost/"
        if (!url.startsWith(prefix)) {
            if (File(url).exists()) return url
            return null
        }
        val encoded = url.substring(prefix.length)
        return try {
            URLDecoder.decode(encoded, "UTF-8")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to decode preview URL: $url", e)
            null
        }
    }
}

internal class NativeColorGradingPreviewBridge(
    activity: ColorGradingActivity,
    private val filePath: String,
) {
    private val activityRef: WeakReference<ColorGradingActivity> = WeakReference(activity)

    @JavascriptInterface
    fun beginPreview(filePath: String) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "beginPreview: $filePath (JNI)")
        Thread {
            val result = ColorGradingJniBridge.beginPreview(filePath)
            activity.runOnUiThread {
                if (result.isSuccess) {
                    activity.isSessionActive = true
                    activity.webView?.evaluateJavascript("window.onPreviewReady?.();", null)
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "解码失败"
                    activity.webView?.evaluateJavascript(
                        "window.onPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            }
        }.start()
    }

    @JavascriptInterface
    fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "applyPreview: lut=$lutId metering=$meteringMode ev=$evOffset (JNI)")
        Thread {
            val result = ColorGradingJniBridge.applyPreview(lutId, true, meteringMode, evOffset)
            activity.runOnUiThread {
                if (result.isSuccess) {
                    val url = result.getOrDefault("")
                    val extractedPath = activity.extractFilePathFromUrl(url)
                    if (extractedPath != null) {
                        activity.previewFilePath = extractedPath
                        activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
                    } else {
                        activity.webView?.evaluateJavascript(
                            "window.notifyPreviewError?.(${JSONObject.quote("Invalid preview URL: $url")});", null
                        )
                    }
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "应用失败"
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                }
            }
        }.start()
    }

    @JavascriptInterface
    fun save(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset")

        activity.previewFilePath = null

        Thread {
            ColorGradingJniBridge.endPreview()
            val mainActivity = MainActivity.instance
            if (mainActivity != null) {
                val args = buildColorGradingArgsJson(filePath, lutId, meteringMode, evOffset, false)
                mainActivity.runOnUiThread {
                    mainActivity.getWebView()?.evaluateJavascript(
                        "(async function(){ try { await window.__tauriTriggerColorGrading?.(...${args}); } catch(e) {} " +
                        "try { window.__tauriSaveColorGradingLastUsed?.(${JSONObject.quote(lutId)},${JSONObject.quote(meteringMode)},${evOffset}); } catch(e) {} })();",
                        null
                    )
                }
            } else {
                Log.w(TAG, "save: MainActivity not available — color grading will not be applied")
            }
        }.start()

        activity.runOnUiThread {
            android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                activity.finish()
            }, 100)
        }
    }

    @JavascriptInterface
    fun cancelPreview() {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "cancelPreview")
        Thread { ColorGradingJniBridge.endPreview() }.start()
        activity.runOnUiThread { activity.finish() }
    }

    @JavascriptInterface
    fun getConfig(): String {
        val presetsJson = ColorGradingJniBridge.getPresets()

        // Read lastUsed directly from Rust config — no WebView IPC needed
        val lastUsedJson = ColorGradingJniBridge.getLastUsed()
        val lastUsed = if (lastUsedJson != null) {
            try { JSONObject(lastUsedJson) } catch (_: Exception) { null }
        } else null

        return JSONObject().apply {
            put("filePath", filePath)
            put("lastUsed", lastUsed ?: JSONObject.NULL)
            put("presets", JSONArray(presetsJson))
        }.toString()
    }

    companion object {
        private const val TAG = "NativeColorGradingPreviewBridge"

        internal fun buildColorGradingArgsJson(
            filePath: String, lutId: String, meteringMode: String, evOffset: Float, syncToAuto: Boolean
        ): String {
            return "[${JSONObject.quote(filePath)},${JSONObject.quote(lutId)},${JSONObject.quote(meteringMode)},$evOffset,$syncToAuto]"
        }
    }
}
