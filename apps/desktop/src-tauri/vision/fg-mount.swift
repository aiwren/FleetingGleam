// fg-mount: connects a network share (SMB, NFS, WebDAV, AFP) for the app, so the person never has to go through the Finder.
// One command per run, one JSON line out:
//   mount <url> <dir> [<user>] [--stdin-password] [--save]   mounts the share at <dir>; the password comes on stdin or from the
//                                                            Keychain (an "Internet password" for that server and user); --save
//                                                            stores it there afterwards
//   unmount <dir>                                            disconnects
//   forget <host> <user>                                     removes the stored password
// It uses the system's own file-sharing client (NetFS), the same one the Finder uses, so the mounted share behaves like any folder.
import Foundation
import NetFS
import Security

func emit(_ d: [String: Any]) {
    if let data = try? JSONSerialization.data(withJSONObject: d), let s = String(data: data, encoding: .utf8) { print(s) }
}
func fail(_ msg: String) -> Never {
    emit(["ok": false, "error": msg])
    exit(1)
}

/// The Keychain protocol a share address means, so a password saved for the NAS's web page (http) is never taken for its SMB share.
func protocolOf(_ url: URL) -> CFString {
    switch url.scheme?.lowercased() {
    case "smb", "cifs": return kSecAttrProtocolSMB
    case "afp": return kSecAttrProtocolAFP
    case "nfs": return kSecAttrProtocolSMB   // NFS carries no password; the key only has to be consistent, and Security has no NFS constant
    case "https": return kSecAttrProtocolHTTPS
    default: return kSecAttrProtocolHTTP
    }
}
func keychainQuery(server: String, account: String, proto: CFString) -> [String: Any] {
    [kSecClass as String: kSecClassInternetPassword, kSecAttrServer as String: server, kSecAttrAccount as String: account, kSecAttrProtocol as String: proto]
}
func keychainGet(server: String, account: String, proto: CFString) -> String? {
    var q = keychainQuery(server: server, account: account, proto: proto)
    q[kSecReturnData as String] = true
    q[kSecMatchLimit as String] = kSecMatchLimitOne
    var item: CFTypeRef?
    guard SecItemCopyMatching(q as CFDictionary, &item) == errSecSuccess, let data = item as? Data else { return nil }
    return String(data: data, encoding: .utf8)
}
func keychainSet(server: String, account: String, proto: CFString, password: String) {
    SecItemDelete(keychainQuery(server: server, account: account, proto: proto) as CFDictionary)
    var q = keychainQuery(server: server, account: account, proto: proto)
    q[kSecValueData as String] = password.data(using: .utf8)
    q[kSecAttrLabel as String] = "FleetingGleam · \(server)"
    q[kSecAttrDescription as String] = "Network share password"
    SecItemAdd(q as CFDictionary, nil)
}
func keychainDelete(server: String, account: String, proto: CFString) {
    SecItemDelete(keychainQuery(server: server, account: account, proto: proto) as CFDictionary)
}

let args = CommandLine.arguments
guard args.count >= 2 else { fail("usage: fg-mount mount|unmount|forget …") }

switch args[1] {
case "mount":
    guard args.count >= 4, let url = URL(string: args[2]), let host = url.host else { fail("usage: fg-mount mount <url> <dir> [<user>] [--stdin-password] [--save]") }
    let dir = URL(fileURLWithPath: args[3])
    let user: String? = args.count > 4 && !args[4].hasPrefix("--") ? args[4] : nil
    var password: String? = nil
    if args.contains("--stdin-password") { password = readLine() }
    if password == nil, let u = user { password = keychainGet(server: host, account: u, proto: protocolOf(url)) }
    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    // the keys are the CFSTR values of NetFS.h (kNetFSAllowLoopbackKey, kNetFSUseGuestKey, kNetFSMountAtMountDirKey, kNetFSSoftMountKey)
    let open = NSMutableDictionary()
    open["AllowLoopback"] = true
    if user == nil { open["Guest"] = true }
    let mount = NSMutableDictionary()
    mount["MountAtMountDir"] = true   // exactly at <dir>, not in a subfolder of it
    mount["SoftMount"] = true         // a share that stops answering fails the operation instead of hanging the app
    var mountpoints: Unmanaged<CFArray>? = nil
    let rc = NetFSMountURLSync(url as CFURL, dir as CFURL, user as CFString?, password as CFString?, open, mount, &mountpoints)
    if rc != 0 {
        // errno values from the local side; NTSTATUS values (negative) from the server
        let why: String
        switch rc {
        case 13, -1073741715, -1073741790: why = "wrong user name or password"            // EACCES, STATUS_LOGON_FAILURE, STATUS_ACCESS_DENIED
        case 2, -1073741620, -1073741275: why = "no such share on that server"           // ENOENT, STATUS_BAD_NETWORK_NAME, STATUS_NOT_FOUND
        case 64, 65, 60, 61: why = "the server cannot be reached"                          // EHOSTDOWN, EHOSTUNREACH, ETIMEDOUT, ECONNREFUSED
        default: why = rc > 0 ? String(cString: strerror(rc)) : "the server refused the connection"
        }
        fail("\(why) (\(rc))")
    }
    if args.contains("--save"), let u = user, let p = password { keychainSet(server: host, account: u, proto: protocolOf(url), password: p) }
    emit(["ok": true, "mountpoint": dir.path])

case "unmount":
    guard args.count >= 3 else { fail("usage: fg-mount unmount <dir>") }
    // only a directory that is itself a mount point: anything else would ask the system to eject the volume it sits on
    var fs = statfs()
    guard statfs(args[2], &fs) == 0 else { fail("no such folder") }
    let mountedAt = withUnsafePointer(to: &fs.f_mntonname) { $0.withMemoryRebound(to: CChar.self, capacity: Int(MAXPATHLEN)) { String(cString: $0) } }
    guard mountedAt == URL(fileURLWithPath: args[2]).standardizedFileURL.path else { fail("not connected") }
    let sem = DispatchSemaphore(value: 0)
    var failure: Error? = nil
    FileManager.default.unmountVolume(at: URL(fileURLWithPath: args[2]), options: [.withoutUI]) { e in failure = e; sem.signal() }
    sem.wait()
    if let e = failure { fail(e.localizedDescription) }
    emit(["ok": true])

case "forget":
    // forget <url-or-host> <user>: the protocol comes from the address when one is given
    guard args.count >= 4 else { fail("usage: fg-mount forget <url> <user>") }
    let url = URL(string: args[2])
    keychainDelete(server: url?.host ?? args[2], account: args[3], proto: url.map(protocolOf) ?? kSecAttrProtocolSMB)
    emit(["ok": true])

default:
    fail("unknown command \(args[1])")
}
