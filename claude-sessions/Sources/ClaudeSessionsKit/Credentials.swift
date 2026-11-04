import Foundation

public enum Service: CaseIterable {
    case bitbucket, jira

    public var keychainService: String {
        switch self {
        case .bitbucket: "bitbucket-api-token"
        case .jira: "jira-api-token"
        }
    }

    public var displayName: String {
        switch self {
        case .bitbucket: "Bitbucket"
        case .jira: "Jira"
        }
    }

    public static let tokenPage = URL(string: "https://id.atlassian.com/manage-profile/security/api-tokens")!
}

public enum ConnectionState: Equatable {
    case checking, ok, rejected, unreachable

    /// The state after one call; `error` is nil on success. Errors that say nothing about the connection keep `previous`.
    public static func after(_ error: Error?, previous: ConnectionState) -> ConnectionState {
        guard let error else { return .ok }
        let code: Int?
        switch error {
        case let e as BitbucketError:
            if case .http(let c) = e { code = c } else { return .unreachable }
        case let e as JiraError:
            if case .http(let c) = e { code = c } else { return .unreachable }
        default:
            return .unreachable
        }
        switch code {
        case 401: return .rejected
        case let c? where c == 429 || c >= 500: return .unreachable
        default: return previous
        }
    }
}

public enum Credentials {
    public static func read(_ s: Service) -> (account: String, token: String)? {
        guard let user = account(s),
              let pw = runProcess("/usr/bin/security", ["find-generic-password", "-s", s.keychainService, "-w"], timeout: 2)
        else { return nil }
        let token = pw.trimmingCharacters(in: .whitespacesAndNewlines)
        return token.isEmpty ? nil : (user, token)
    }

    /// The account of the Keychain item, never the token.
    public static func account(_ s: Service) -> String? {
        runProcess("/usr/bin/security", ["find-generic-password", "-s", s.keychainService], timeout: 2)
            .flatMap(account(fromAttributes:))
    }

    /// The value of the `"acct"` attribute line of `security find-generic-password` output.
    static func account(fromAttributes out: String) -> String? {
        for raw in out.split(separator: "\n") where raw.contains("\"acct\"<blob>=") {
            let line = raw.trimmingCharacters(in: .whitespaces)
            guard line.hasSuffix("\""), let eq = line.range(of: "<blob>=\"") else { return nil }
            return String(line[eq.upperBound..<line.index(before: line.endIndex)])
        }
        return nil
    }

    /// A `security -i` command line, or nil when a value holds a quote, backslash or newline.
    static func addScript(service: String, account: String, token: String) -> String? {
        for v in [service, account, token] where v.contains(where: { $0 == "\"" || $0 == "\\" || $0.isNewline }) { return nil }
        return "add-generic-password -s \"\(service)\" -a \"\(account)\" -w \"\(token)\"\n"
    }

    /// Replaces any item of the service. The token travels on stdin only, never in an argument list.
    public static func write(_ s: Service, account: String, token: String) -> Bool {
        guard let script = addScript(service: s.keychainService, account: account, token: token) else { return false }
        for _ in 0..<10 {
            if runProcess("/usr/bin/security", ["delete-generic-password", "-s", s.keychainService], timeout: 2) == nil { break }
        }
        guard runProcess("/usr/bin/security", ["-i"], timeout: 5, input: Data(script.utf8)) != nil else { return false }
        return Self.account(s) == account
    }
}
