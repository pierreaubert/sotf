import Foundation
import Security

private let sotfRemoteTokenService = "org.spinorama.sotf.remote"

private func keychainQuery(account: String) -> [String: Any] {
    [
        kSecClass as String: kSecClassGenericPassword,
        kSecAttrService as String: sotfRemoteTokenService,
        kSecAttrAccount as String: account,
        kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
    ]
}

private func stringFromCString(_ pointer: UnsafePointer<CChar>?) -> String? {
    guard let pointer else {
        return nil
    }
    let value = String(cString: pointer)
    return value.isEmpty ? nil : value
}

@_cdecl("sotf_ios_keychain_save")
func sotfIosKeychainSave(
    keyPtr: UnsafePointer<CChar>?,
    tokenPtr: UnsafePointer<CChar>?
) -> Bool {
    guard let key = stringFromCString(keyPtr),
          let token = stringFromCString(tokenPtr),
          let data = token.data(using: .utf8) else {
        return false
    }

    let query = keychainQuery(account: key)
    let attributes: [String: Any] = [
        kSecValueData as String: data
    ]

    let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
    if updateStatus == errSecSuccess {
        return true
    }
    if updateStatus != errSecItemNotFound {
        NSLog("[Keychain] Failed to update remote token: \(updateStatus)")
        return false
    }

    var addQuery = query
    addQuery[kSecValueData as String] = data
    let addStatus = SecItemAdd(addQuery as CFDictionary, nil)
    if addStatus != errSecSuccess {
        NSLog("[Keychain] Failed to save remote token: \(addStatus)")
        return false
    }
    return true
}

@_cdecl("sotf_ios_keychain_load")
func sotfIosKeychainLoad(keyPtr: UnsafePointer<CChar>?) -> UnsafePointer<CChar>? {
    guard let key = stringFromCString(keyPtr) else {
        return nil
    }

    var query = keychainQuery(account: key)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne

    var item: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &item)
    guard status == errSecSuccess else {
        if status != errSecItemNotFound {
            NSLog("[Keychain] Failed to load remote token: \(status)")
        }
        return nil
    }
    guard let data = item as? Data,
          let token = String(data: data, encoding: .utf8) else {
        return nil
    }

    struct Static {
        static var buffer: [CChar] = []
    }
    Static.buffer = Array(token.utf8CString)
    return Static.buffer.withUnsafeBufferPointer { $0.baseAddress }
}

@_cdecl("sotf_ios_keychain_delete")
func sotfIosKeychainDelete(keyPtr: UnsafePointer<CChar>?) -> Bool {
    guard let key = stringFromCString(keyPtr) else {
        return false
    }

    let status = SecItemDelete(keychainQuery(account: key) as CFDictionary)
    if status == errSecSuccess || status == errSecItemNotFound {
        return true
    }
    NSLog("[Keychain] Failed to delete remote token: \(status)")
    return false
}
