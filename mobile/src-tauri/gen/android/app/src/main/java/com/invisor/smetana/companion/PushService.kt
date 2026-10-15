package com.invisor.smetana.companion

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

/**
 * Receives the data-only message the push gateway sends through FCM
 * (server/push-gateway/src/providers/fcm.rs): `data.blob` is the encrypted message.
 * It is opened with the push secret from the vault and shown as a notification;
 * tapping it opens the app with the blob's `target` as an intent extra.
 */
class PushService : FirebaseMessagingService() {
    override fun onNewToken(token: String) {
        PushCrypto.onToken(applicationContext, token)
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val blob = message.data["blob"] ?: return
        val opened = PushCrypto.open(applicationContext, blob)
        show(applicationContext, opened?.title ?: "Smetana", opened?.body ?: "Open the app to see what happened.", opened?.target)
    }

    private fun show(context: Context, title: String, body: String, target: String?) {
        val manager = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            manager.createNotificationChannel(NotificationChannel(CHANNEL, "Runs and agents", NotificationManager.IMPORTANCE_DEFAULT))
        }
        val open = Intent(context, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP
            if (target != null) putExtra(EXTRA_TARGET, target)
        }
        val pending = PendingIntent.getActivity(context, 0, open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle(title)
            .setContentText(body)
            .setAutoCancel(true)
            .setContentIntent(pending)
            .build()
        manager.notify(System.currentTimeMillis().toInt(), notification)
    }

    companion object {
        const val CHANNEL = "runs"
        const val EXTRA_TARGET = "target"
    }
}
