// A command-line check of PushBlob.swift against the Rust-sealed vector.
// Run by ios-push/check.sh; not part of any app target.
import Foundation

func hex(_ s: String) -> Data {
    var d = Data(); var i = s.startIndex
    while i < s.endIndex { let j = s.index(i, offsetBy: 2); d.append(UInt8(s[i..<j], radix: 16)!); i = j }
    return d
}

let path = CommandLine.arguments[1]
let vector = try JSONSerialization.jsonObject(with: Data(contentsOf: URL(fileURLWithPath: path))) as! [String: String]
let secret = hex(vector["secret"]!)
let wire = vector["wire"]!
let expected = vector["plaintext"]!

var failures = 0
func check(_ ok: Bool, _ what: String) { print(ok ? "ok   " : "FAIL ", what); if !ok { failures += 1 } }

let opened = PushBlob.open(secret: secret, wire: wire).flatMap { String(data: $0, encoding: .utf8) }
check(opened == expected, "the blob sealed by remote_protocol::push::seal opens to its plaintext")
check(PushBlob.open(secret: Data(repeating: 8, count: 32), wire: wire) == nil, "another key does not open it")
var damaged = Data(base64Encoded: wire)!; damaged[damaged.count - 1] ^= 1
check(PushBlob.open(secret: secret, wire: damaged.base64EncodedString()) == nil, "a damaged byte does not open")
check(PushBlob.open(secret: secret, wire: "not base64!") == nil, "garbage is nil")
exit(failures == 0 ? 0 : 1)
