// Opening a push blob on iOS: the same algorithm as `remote_protocol::push::open`
// (crates/remote-protocol/src/push.rs), written against CryptoKit.
//
// The notification service extension has a memory budget far below the app's, and
// the app's Rust core carries an entire QUIC stack, so the extension does not link
// it. What it needs is small: an X25519 agreement, a BLAKE3 key derivation over
// 96 bytes, and XChaCha20-Poly1305. CryptoKit has the first and ChaCha20-Poly1305;
// the key derivation and the extension of the nonce (HChaCha20) are written out
// here. `ios-push/check.sh` opens the vector in `test-vectors/push.json`, sealed by
// the Rust code, with this file.
//
// The format, for whoever changes either side:
//   wire        = base64(postcard(PushBlob))
//   PushBlob    = ephemeral[32] || nonce[24] || varint(len) || ciphertext
//   key         = BLAKE3.derive_key("smetana 2026-10-15 push blob v1",
//                                   shared || ephemeral || recipient_public)
//   plaintext   = XChaCha20-Poly1305(key, nonce, ciphertext)   (tag = last 16 bytes)

import CryptoKit
import Foundation

public enum PushBlob {
    static let kdfContext = "smetana 2026-10-15 push blob v1"

    /// The plaintext of a blob, or nil when it does not open for any reason.
    public static func open(secret: Data, wire: String) -> Data? {
        guard secret.count == 32,
              let bytes = Data(base64Encoded: wire.trimmingCharacters(in: .whitespacesAndNewlines)),
              bytes.count >= 32 + 24 + 1 else { return nil }
        let ephemeral = bytes.subdata(in: 0..<32)
        let nonce = bytes.subdata(in: 32..<56)
        // postcard writes a Vec's length as a LEB128 varint.
        var length = 0, shift = 0, index = 56
        while index < bytes.count {
            let byte = Int(bytes[index]); index += 1
            length |= (byte & 0x7f) << shift
            if byte & 0x80 == 0 { break }
            shift += 7
            if shift > 28 { return nil }
        }
        guard bytes.count - index == length, length >= 16 else { return nil }
        let ciphertext = bytes.subdata(in: index..<bytes.count)

        guard let privateKey = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: secret),
              let peer = try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: ephemeral),
              let agreed = try? privateKey.sharedSecretFromKeyAgreement(with: peer) else { return nil }
        let shared = agreed.withUnsafeBytes { Data($0) }
        let recipient = privateKey.publicKey.rawRepresentation

        var material = Data()
        material.append(shared); material.append(ephemeral); material.append(recipient)
        let key = Blake3.deriveKey(context: Data(kdfContext.utf8), material: material)

        let subkey = hChaCha20(key: key, nonce: nonce.prefix(16))
        var nonce12 = Data(count: 4)
        nonce12.append(nonce.suffix(8))
        guard let chachaNonce = try? ChaChaPoly.Nonce(data: nonce12),
              let box = try? ChaChaPoly.SealedBox(nonce: chachaNonce,
                                                   ciphertext: ciphertext.dropLast(16),
                                                   tag: ciphertext.suffix(16)),
              let plain = try? ChaChaPoly.open(box, using: SymmetricKey(data: subkey)) else { return nil }
        return plain
    }

    static func hChaCha20(key: Data, nonce: Data) -> Data {
        func word(_ d: Data, _ i: Int) -> UInt32 {
            let b = d.startIndex + i * 4
            return UInt32(d[b]) | UInt32(d[b + 1]) << 8 | UInt32(d[b + 2]) << 16 | UInt32(d[b + 3]) << 24
        }
        var s: [UInt32] = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]
        for i in 0..<8 { s.append(word(key, i)) }
        for i in 0..<4 { s.append(word(nonce, i)) }
        func rotl(_ x: UInt32, _ n: UInt32) -> UInt32 { (x << n) | (x >> (32 - n)) }
        func quarter(_ a: Int, _ b: Int, _ c: Int, _ d: Int) {
            s[a] = s[a] &+ s[b]; s[d] = rotl(s[d] ^ s[a], 16)
            s[c] = s[c] &+ s[d]; s[b] = rotl(s[b] ^ s[c], 12)
            s[a] = s[a] &+ s[b]; s[d] = rotl(s[d] ^ s[a], 8)
            s[c] = s[c] &+ s[d]; s[b] = rotl(s[b] ^ s[c], 7)
        }
        for _ in 0..<10 {
            quarter(0, 4, 8, 12); quarter(1, 5, 9, 13); quarter(2, 6, 10, 14); quarter(3, 7, 11, 15)
            quarter(0, 5, 10, 15); quarter(1, 6, 11, 12); quarter(2, 7, 8, 13); quarter(3, 4, 9, 14)
        }
        var out = Data()
        for w in [s[0], s[1], s[2], s[3], s[12], s[13], s[14], s[15]] {
            out.append(contentsOf: [UInt8(w & 0xff), UInt8((w >> 8) & 0xff), UInt8((w >> 16) & 0xff), UInt8(w >> 24)])
        }
        return out
    }
}

/// BLAKE3, only as far as the key derivation needs: inputs of at most one chunk
/// (1024 bytes), 32 bytes of output, `derive_key` mode.
enum Blake3 {
    static let iv: [UInt32] = [0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
                               0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19]
    static let permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
    static let chunkStart: UInt32 = 1, chunkEnd: UInt32 = 2, root: UInt32 = 8
    static let deriveKeyContext: UInt32 = 32, deriveKeyMaterial: UInt32 = 64

    static func deriveKey(context: Data, material: Data) -> Data {
        let contextKey = hash(context, key: iv, flags: deriveKeyContext)
        return hash(material, key: words(contextKey), flags: deriveKeyMaterial)
    }

    static func words(_ d: Data) -> [UInt32] {
        (0..<(d.count / 4)).map { i in
            let b = d.startIndex + i * 4
            return UInt32(d[b]) | UInt32(d[b + 1]) << 8 | UInt32(d[b + 2]) << 16 | UInt32(d[b + 3]) << 24
        }
    }

    static func hash(_ input: Data, key: [UInt32], flags: UInt32) -> Data {
        precondition(input.count <= 1024, "one chunk only")
        var chainingValue = key
        let blocks = max(1, (input.count + 63) / 64)
        var out = Data()
        for block in 0..<blocks {
            let start = block * 64
            let slice = input.subdata(in: (input.startIndex + start)..<(input.startIndex + min(input.count, start + 64)))
            var padded = slice
            padded.append(Data(count: 64 - slice.count))
            var f = flags
            if block == 0 { f |= chunkStart }
            let last = block == blocks - 1
            if last { f |= chunkEnd | root }
            let state = compress(chainingValue, words(padded), counter: 0, blockLength: UInt32(slice.count), flags: f)
            if last {
                for w in state[0..<8] {
                    out.append(contentsOf: [UInt8(w & 0xff), UInt8((w >> 8) & 0xff), UInt8((w >> 16) & 0xff), UInt8(w >> 24)])
                }
            } else {
                chainingValue = Array(state[0..<8])
            }
        }
        return out
    }

    static func compress(_ cv: [UInt32], _ block: [UInt32], counter: UInt64, blockLength: UInt32, flags: UInt32) -> [UInt32] {
        var v: [UInt32] = cv + Array(iv[0..<4]) + [UInt32(truncatingIfNeeded: counter), UInt32(truncatingIfNeeded: counter >> 32), blockLength, flags]
        var m = block
        func rotr(_ x: UInt32, _ n: UInt32) -> UInt32 { (x >> n) | (x << (32 - n)) }
        func g(_ a: Int, _ b: Int, _ c: Int, _ d: Int, _ x: UInt32, _ y: UInt32) {
            v[a] = v[a] &+ v[b] &+ x; v[d] = rotr(v[d] ^ v[a], 16)
            v[c] = v[c] &+ v[d]; v[b] = rotr(v[b] ^ v[c], 12)
            v[a] = v[a] &+ v[b] &+ y; v[d] = rotr(v[d] ^ v[a], 8)
            v[c] = v[c] &+ v[d]; v[b] = rotr(v[b] ^ v[c], 7)
        }
        for round in 0..<7 {
            g(0, 4, 8, 12, m[0], m[1]); g(1, 5, 9, 13, m[2], m[3])
            g(2, 6, 10, 14, m[4], m[5]); g(3, 7, 11, 15, m[6], m[7])
            g(0, 5, 10, 15, m[8], m[9]); g(1, 6, 11, 12, m[10], m[11])
            g(2, 7, 8, 13, m[12], m[13]); g(3, 4, 9, 14, m[14], m[15])
            if round < 6 { m = permutation.map { m[$0] } }
        }
        for i in 0..<8 { v[i] ^= v[i + 8]; v[i + 8] ^= cv[i] }
        return v
    }
}
