import SwiftUI

/// Every colour, font, spacing and icon of the widget's expanded card.
enum Theme {
    enum Colors {
        static let canvas = Color(red: 0x1C / 255, green: 0x1C / 255, blue: 0x1E / 255)
        static let card = Color(red: 0x2C / 255, green: 0x2C / 255, blue: 0x2E / 255)
        static let cardHover = Color(red: 0x3A / 255, green: 0x3A / 255, blue: 0x3C / 255)
        static let field = Color.white.opacity(0.10)
        static let stroke = Color.white.opacity(0.10)
        static let primaryText = Color.white.opacity(0.92)
        static let secondaryText = Color.white.opacity(0.65)
        static let tertiaryText = Color.white.opacity(0.50)
        static let primaryAction = Color(red: 0x30 / 255, green: 0xD1 / 255, blue: 0x58 / 255)
        static let onPrimary = Color.black
        static let fresh = Color.yellow
        static let freshPR = Color.purple

        static func status(_ colorStatus: String) -> Color {
            switch colorStatus {
            case "waiting": Color(red: 0xFF / 255, green: 0x45 / 255, blue: 0x3A / 255)
            case "idle": Color(red: 0xFF / 255, green: 0x9F / 255, blue: 0x0A / 255)
            case "busy": Color(red: 0x0A / 255, green: 0x84 / 255, blue: 0xFF / 255)
            default: .gray
            }
        }
    }

    enum Fonts {
        static let title = Font.system(size: 13, weight: .semibold)
        static let body = Font.system(size: 11.5)
        static let meta = Font.system(size: 10.5)
        static let mono = Font.system(size: 11, design: .monospaced)

        static func mono(bold: Bool) -> Font {
            Font.system(size: 11, weight: bold ? .bold : .regular).monospaced()
        }
    }

    enum Space {
        static let gap: CGFloat = 6
        static let indent: CGFloat = 22
        static let rightColumn: CGFloat = 64
    }

    enum Symbols {
        static let comments = "bubble.left"
        static let rabbit = "hare"
        static let agents = "cpu"
        static let fresh = "sparkle"
        static let menu = "list.bullet"
        static let activity = "chevron.right"
        static let link = "arrow.up.right"
        static let pr = "arrow.triangle.pull"
        static let checkOff = "square"
        static let checkOn = "checkmark.square.fill"
        static let radioOff = "circle"
        static let radioOn = "largecircle.fill.circle"
        static let reply = "arrowshape.turn.up.left"
        static let stop = "stop.fill"

        static func status(_ s: Session) -> String {
            if s.status == "waiting" { return s.isPermissionPrompt ? "exclamationmark.triangle.fill" : "questionmark.circle.fill" }
            if (s.backgroundAgents ?? 0) > 0 { return "hourglass" }
            switch s.shownStatus {
            case "idle": return "checkmark.circle.fill"
            case "busy": return "circle.dotted"
            default: return "circle"
            }
        }
    }
}
