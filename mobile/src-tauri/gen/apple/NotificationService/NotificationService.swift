// The notification service extension: the half of push that happens before the
// notification is shown.
//
// The gateway sends APNs a payload with a fixed title ("Smetana"), `mutable-content`
// and the encrypted blob at the top level (server/push-gateway/src/providers/apns.rs).
// This replaces the title and body with the decrypted message. The key it needs is
// the push secret the app keeps in the Keychain, readable here because both targets
// carry the same keychain access group and the item is `AfterFirstUnlock` (see
// mobile/src-tauri/src/keychain.rs). The decryption is `PushBlob.swift`, the same
// algorithm as `remote_protocol::push::open`, checked against a vector the Rust code
// sealed (mobile/ios-push/check.sh).
//
// Nothing here may be slow or large: the system kills an extension that exceeds its
// memory limit or its ~30 seconds. If anything fails the notification is delivered
// as the gateway sent it, with a neutral body.

import Foundation
import Security
import UserNotifications

private struct Message: Decodable {
    let title: String
    let body: String
    let target: String?
}

private enum Vault {
    // Also spelled in mobile/src-tauri/src/keychain.rs and secrets.rs (names::PUSH_SECRET).
    static let service = "com.invisor.smetana.companion.vault"
    static let account = "push_secret"

    static func pushSecret() -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var found: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &found) == errSecSuccess,
              let data = found as? Data,
              let hex = String(data: data, encoding: .utf8) else { return nil }
        var bytes = Data()
        var index = hex.startIndex
        while index < hex.endIndex {
            guard let next = hex.index(index, offsetBy: 2, limitedBy: hex.endIndex),
                  let byte = UInt8(hex[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        return bytes.count == 32 ? bytes : nil
    }
}

class NotificationService: UNNotificationServiceExtension {
    private var handler: ((UNNotificationContent) -> Void)?
    private var content: UNMutableNotificationContent?

    override func didReceive(
        _ request: UNNotificationRequest,
        withContentHandler contentHandler: @escaping (UNNotificationContent) -> Void
    ) {
        handler = contentHandler
        content = request.content.mutableCopy() as? UNMutableNotificationContent
        guard let content = content else {
            contentHandler(request.content)
            return
        }
        content.body = "Open the app to see what happened."
        if let wire = request.content.userInfo["blob"] as? String,
           let secret = Vault.pushSecret(),
           let plain = PushBlob.open(secret: secret, wire: wire),
           let message = try? JSONDecoder().decode(Message.self, from: plain) {
            content.title = message.title
            content.body = message.body
            if let target = message.target {
                var info = content.userInfo
                info["target"] = target
                content.userInfo = info
            }
        }
        contentHandler(content)
    }

    override func serviceExtensionTimeWillExpire() {
        if let handler = handler, let content = content {
            handler(content)
        }
    }
}
