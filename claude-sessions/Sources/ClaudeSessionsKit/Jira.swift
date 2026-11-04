import Foundation

public enum JiraError: Error, Equatable { case http(Int), transport }

public protocol JiraAPI: AnyObject {
    /// Blocking: called only on the monitor's queue or a background queue.
    func bulkFetch(host: String, keys: [String]) throws -> Data
    /// Blocking: called only on a background queue.
    func myself(host: String) throws -> Data
}

public struct JiraIssue: Equatable {
    public let title: String?
    public let status: String?
    public init(title: String?, status: String?) { self.title = title; self.status = status }
}

enum Jira {
    /// The only hosts that may receive the Jira token.
    static func isAtlassianHost(_ host: String) -> Bool { host.lowercased().hasSuffix(".atlassian.net") }

    /// Every requested key is in the result; one `bulkfetch` did not return gets no title and no status. Nil when `data` is not JSON.
    static func issues(from data: Data, requested: [String]) -> [String: JiraIssue]? {
        guard let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        var out = Dictionary(uniqueKeysWithValues: Set(requested).map { ($0, JiraIssue(title: nil, status: nil)) })
        for issue in o["issues"] as? [[String: Any]] ?? [] {
            guard let key = issue["key"] as? String, out[key] != nil else { continue }
            let fields = issue["fields"] as? [String: Any]
            out[key] = JiraIssue(title: fields?["summary"] as? String,
                                 status: (fields?["status"] as? [String: Any])?["name"] as? String)
        }
        return out
    }
}

public final class JiraClient: JiraAPI, CredentialReloading {
    private let http: BasicAuthHTTP

    /// Nil when the Keychain item or its account is missing.
    public init?() {
        guard let h = BasicAuthHTTP(service: .jira) else { return nil }
        http = h
    }

    /// Uses the given credentials until `reload()`; no Keychain read.
    public init(account: String, token: String) {
        http = BasicAuthHTTP(service: .jira, account: account, token: token)
    }

    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    public func reload() { http.reload() }

    static func request(host: String, keys: [String], auth: String) -> URLRequest? {
        guard var req = request(host: host, path: "/rest/api/3/issue/bulkfetch", auth: auth),
              let body = try? JSONSerialization.data(withJSONObject: ["issueIdsOrKeys": keys, "fields": ["summary", "status"]])
        else { return nil }
        req.httpMethod = "POST"
        req.httpBody = body
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        return req
    }

    /// Nil for a host outside `*.atlassian.net`: the token must never go anywhere else.
    private static func request(host: String, path: String, auth: String) -> URLRequest? {
        guard Jira.isAtlassianHost(host), let url = URL(string: "https://\(host)\(path)"), url.host == host else { return nil }
        var req = URLRequest(url: url, timeoutInterval: 15)
        req.setValue(auth, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        return req
    }

    public func bulkFetch(host: String, keys: [String]) throws -> Data {
        guard let header = http.currentAuth(), let req = Self.request(host: host, keys: keys, auth: header) else { throw JiraError.transport }
        return try send(req)
    }

    public func myself(host: String) throws -> Data {
        guard let header = http.currentAuth(), let req = Self.request(host: host, path: "/rest/api/3/myself", auth: header) else { throw JiraError.transport }
        return try send(req)
    }

    private func send(_ req: URLRequest) throws -> Data {
        do {
            return try http.send(req)
        } catch BasicAuthHTTP.Failure.http(let code) {
            throw JiraError.http(code)
        } catch {
            throw JiraError.transport
        }
    }
}
