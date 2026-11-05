import Foundation

public enum BitbucketError: Error, Equatable { case http(Int), transport }

public protocol BitbucketAPI: AnyObject {
    /// `path` begins with `/2.0/` and includes the query. Blocking: called only on the monitor's queue.
    func get(_ path: String) throws -> Data
    /// Blocking like `get`; `json` is sent as the request body.
    func post(_ path: String, json: [String: Any]) throws -> Data
}

/// Something that can drop its cached credentials and read the Keychain again.
public protocol CredentialReloading: AnyObject {
    func reload()
}

public final class BitbucketClient: BitbucketAPI, CredentialReloading {
    static let base = "https://api.bitbucket.org"

    private let http: BasicAuthHTTP

    /// Uses the given credentials until `reload()`; no Keychain read.
    public init(account: String, token: String) {
        http = BasicAuthHTTP(service: .bitbucket, account: account, token: token)
    }

    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    public func reload() { http.reload() }

    public func get(_ path: String) throws -> Data {
        guard let header = http.currentAuth(), let url = URL(string: Self.base + path) else { throw BitbucketError.transport }
        var req = URLRequest(url: url, timeoutInterval: 15)
        req.setValue(header, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        do {
            return try http.send(req)
        } catch BasicAuthHTTP.Failure.http(let code) {
            throw BitbucketError.http(code)
        } catch {
            throw BitbucketError.transport
        }
    }

    public func post(_ path: String, json: [String: Any]) throws -> Data {
        guard let header = http.currentAuth(), let url = URL(string: Self.base + path),
              let body = try? JSONSerialization.data(withJSONObject: json) else { throw BitbucketError.transport }
        var req = URLRequest(url: url, timeoutInterval: 15)
        req.httpMethod = "POST"
        req.httpBody = body
        req.setValue(header, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        do {
            return try http.send(req)
        } catch BasicAuthHTTP.Failure.http(let code) {
            throw BitbucketError.http(code)
        } catch {
            throw BitbucketError.transport
        }
    }
}
