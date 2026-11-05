import Foundation

public enum JiraError: Error, Equatable { case http(Int), transport }

public protocol JiraAPI: AnyObject {
    /// Blocking: called only on the monitor's queue or a background queue.
    func bulkFetch(host: String, keys: [String]) throws -> Data
    /// Blocking: called only on a background queue.
    func myself(host: String) throws -> Data
    /// Blocking: called only on the monitor's queue. `url` is an issue type icon.
    func icon(url: URL) throws -> Data
}

public struct JiraIssue: Equatable {
    public let title: String?
    public let status: String?
    public let type: String?
    public let iconURL: URL?
    public let tone: Tone?

    public enum Tone: Equatable { case todo, doing, done }

    public init(title: String?, status: String?, type: String? = nil, iconURL: URL? = nil, tone: Tone? = nil) {
        self.title = title; self.status = status; self.type = type; self.iconURL = iconURL; self.tone = tone
    }
}

enum Jira {
    /// The only hosts that may receive the Jira token.
    static func isAtlassianHost(_ host: String) -> Bool { host.lowercased().hasSuffix(".atlassian.net") }

    /// Whether a newly seen host replaces the stored one: first seen wins, except an Atlassian host replaces a foreign one.
    static func replaces(_ stored: String?, with host: String) -> Bool {
        guard let stored else { return true }
        return isAtlassianHost(host) && !isAtlassianHost(stored)
    }

    /// Every requested key is in the result; one `bulkfetch` did not return gets no title and no status. Nil when `data` is not JSON.
    static func issues(from data: Data, requested: [String]) -> [String: JiraIssue]? {
        guard let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return nil }
        var out = Dictionary(uniqueKeysWithValues: Set(requested).map { ($0, JiraIssue(title: nil, status: nil)) })
        for issue in o["issues"] as? [[String: Any]] ?? [] {
            guard let key = issue["key"] as? String, out[key] != nil else { continue }
            let fields = issue["fields"] as? [String: Any]
            let status = fields?["status"] as? [String: Any]
            let type = fields?["issuetype"] as? [String: Any]
            let tone: JiraIssue.Tone?
            switch (status?["statusCategory"] as? [String: Any])?["key"] as? String {
            case "new": tone = .todo
            case "indeterminate": tone = .doing
            case "done": tone = .done
            default: tone = nil
            }
            out[key] = JiraIssue(title: fields?["summary"] as? String, status: status?["name"] as? String,
                                 type: type?["name"] as? String, iconURL: (type?["iconUrl"] as? String).flatMap { URL(string: $0) },
                                 tone: tone)
        }
        return out
    }
}

public final class JiraClient: JiraAPI, CredentialReloading {
    private let http: BasicAuthHTTP

    /// Uses the given credentials until `reload()`; no Keychain read.
    public init(account: String, token: String) {
        http = BasicAuthHTTP(service: .jira, account: account, token: token)
    }

    /// Re-reads the Keychain now, replacing any cached or explicit credentials.
    public func reload() { http.reload() }

    static func request(host: String, keys: [String], auth: String) -> URLRequest? {
        guard var req = request(host: host, path: "/rest/api/3/issue/bulkfetch", auth: auth),
              let body = try? JSONSerialization.data(withJSONObject: ["issueIdsOrKeys": keys, "fields": ["summary", "status", "issuetype"]])
        else { return nil }
        req.httpMethod = "POST"
        req.httpBody = body
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        return req
    }

    /// Nil for a host outside `*.atlassian.net`: the token must never go elsewhere. Asks Jira for a PNG rendering.
    static func iconRequest(url: URL, auth: String) -> URLRequest? {
        guard Jira.isAtlassianHost(url.host ?? ""), var c = URLComponents(url: url, resolvingAgainstBaseURL: false) else { return nil }
        c.queryItems = (c.queryItems ?? []).filter { $0.name != "format" } + [URLQueryItem(name: "format", value: "png")]
        guard let full = c.url, full.host == url.host else { return nil }
        var req = URLRequest(url: full, timeoutInterval: 15)
        req.setValue(auth, forHTTPHeaderField: "Authorization")
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

    public func icon(url: URL) throws -> Data {
        guard let header = http.currentAuth(), let req = Self.iconRequest(url: url, auth: header) else { throw JiraError.transport }
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
