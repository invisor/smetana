package com.invisor.smetana.companion

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The phone's secrets: the iroh key, the push key, the desktop's key and the relay
 * URL, as the Rust core stores them (mobile/src-tauri/src/secrets.rs).
 *
 * An AES-256-GCM key is made inside the Android Keystore and never leaves it; each
 * value is stored encrypted under it, in a preferences file that holds nothing
 * readable. The Rust core reaches this over JNI (android.rs), and the messaging
 * service reads the push secret from here directly. Entry names are spelled in
 * secrets.rs (`names`); "push_secret" and "push_token" are used here too.
 */
object SecretVault {
    private const val ALIAS = "smetana_vault_key"
    private const val PREFS = "smetana_vault"
    private const val PROVIDER = "AndroidKeyStore"

    private fun key(): SecretKey {
        val store = KeyStore.getInstance(PROVIDER).apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, PROVIDER)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return generator.generateKey()
    }

    @JvmStatic
    fun get(context: Context, name: String): String? {
        val stored = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(name, null) ?: return null
        return try {
            val raw = Base64.decode(stored, Base64.NO_WRAP)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, raw, 0, 12))
            String(cipher.doFinal(raw, 12, raw.size - 12), Charsets.UTF_8)
        } catch (e: Exception) {
            null
        }
    }

    @JvmStatic
    fun put(context: Context, name: String, value: String): Boolean {
        return try {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key())
            val sealed = cipher.iv + cipher.doFinal(value.toByteArray(Charsets.UTF_8))
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .edit().putString(name, Base64.encodeToString(sealed, Base64.NO_WRAP)).commit()
        } catch (e: Exception) {
            false
        }
    }

    @JvmStatic
    fun delete(context: Context, name: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().remove(name).commit()
    }
}
