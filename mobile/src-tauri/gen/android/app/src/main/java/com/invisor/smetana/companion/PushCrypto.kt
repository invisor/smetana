package com.invisor.smetana.companion

import android.content.Context
import org.json.JSONObject

/**
 * The Kotlin face of the Rust core's push functions (mobile/src-tauri/src/android.rs).
 * `nativeOpen` is the same `push::open_wire` the Rust test opens the committed vector
 * with, so a blob the desktop's `seal` produced opens here byte for byte.
 */
object PushCrypto {
    init {
        System.loadLibrary("smetana_companion_lib")
    }

    @JvmStatic external fun nativeOpen(secret: ByteArray, blob: String): ByteArray?
    @JvmStatic external fun nativeTokenChanged(token: String)
    @JvmStatic external fun nativeOpenedFromPush(target: String)

    class Message(val title: String, val body: String, val target: String?)

    /** Opens a blob with the push secret in the vault; null when anything is wrong. */
    fun open(context: Context, blob: String): Message? {
        val hex = SecretVault.get(context, "push_secret") ?: return null
        if (hex.length != 64) return null
        val secret = ByteArray(32) { hex.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
        val plain = nativeOpen(secret, blob) ?: return null
        return try {
            val json = JSONObject(String(plain, Charsets.UTF_8))
            Message(json.getString("title"), json.getString("body"), json.optString("target").ifEmpty { null })
        } catch (e: Exception) {
            null
        }
    }

    /** A token from FCM: kept in the vault (the core reads it at its next connect) and handed to a running core. */
    fun onToken(context: Context, token: String) {
        SecretVault.put(context, "push_token", token)
        try {
            nativeTokenChanged(token)
        } catch (e: UnsatisfiedLinkError) {
            // The core is not loaded in this process; the vault has the token.
        }
    }
}
