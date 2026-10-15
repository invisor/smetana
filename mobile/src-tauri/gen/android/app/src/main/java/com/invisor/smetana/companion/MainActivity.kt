package com.invisor.smetana.companion

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import com.google.firebase.messaging.FirebaseMessaging

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    askForNotifications()
    registerForPush()
    openedFromPush(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    openedFromPush(intent)
  }

  private fun askForNotifications() {
    if (Build.VERSION.SDK_INT >= 33 &&
      ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
    ) {
      ActivityCompat.requestPermissions(this, arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
    }
  }

  /** Asks FCM for this install's token. Without a Firebase project (no google-services.json) there is none. */
  private fun registerForPush() {
    try {
      FirebaseMessaging.getInstance().token.addOnSuccessListener { PushCrypto.onToken(applicationContext, it) }
    } catch (e: IllegalStateException) {
      Log.i("Smetana", "push is off: this build has no Firebase project")
    }
  }

  private fun openedFromPush(intent: Intent?) {
    val target = intent?.getStringExtra(PushService.EXTRA_TARGET) ?: return
    try {
      PushCrypto.nativeOpenedFromPush(target)
    } catch (e: UnsatisfiedLinkError) {
      // The core is not up yet; the extra stays on the intent.
    }
  }
}
