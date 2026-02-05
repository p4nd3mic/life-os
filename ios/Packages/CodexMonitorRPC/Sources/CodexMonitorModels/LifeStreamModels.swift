import Foundation

// MARK: - Life Stream Models

public enum CardState: String, Codable, Sendable, Hashable {
    case pending
    case processing
    case awaitingInput = "awaiting_input"
    case complete
    case error
    case cancelled
}

public enum CardType: String, Codable, Sendable, Hashable {
    case meal
    case deliveryOrder = "delivery_order"
    case deliverySession = "delivery_session"
    case mediaAdd = "media_add"
    case music
    case thought
    case query
    case codeTask = "code_task"
    case generic
}

public enum DomainId: String, Codable, Sendable, Hashable, CaseIterable {
    case nutrition
    case delivery
    case media
    case youtube
    case finance
    case fitness
    case general
}

public enum ImageStatus: String, Codable, Sendable, Hashable {
    case loading
    case ready
    case missing
    case uploadPrompt = "upload_prompt"
}

public struct CardImage: Codable, Hashable, Sendable {
    public var url: String?
    public var status: ImageStatus
    public var source: String?
}

public struct CardSource: Codable, Hashable, Sendable {
    public var streamFile: String?
    public var streamAnchor: String?
}

public struct EntityRef: Codable, Hashable, Sendable {
    public var entityType: String
    public var id: String?
    public var name: String
    public var link: String?

    enum CodingKeys: String, CodingKey {
        case entityType = "type"
        case id
        case name
        case link
    }
}

public struct CardRequestMeta: Codable, Hashable, Sendable {
    public var model: String?
    public var effort: String?
    public var accessMode: String?
}

public struct ExpandedSection: Codable, Hashable, Sendable {
    public var title: String
    public var body: String
}

public struct EntityLink: Codable, Hashable, Sendable {
    public var name: String
    public var path: String
    public var icon: String?
}

public struct CardAction: Codable, Hashable, Sendable {
    public var id: String
    public var label: String
    public var icon: String?
    public var style: String?
}

public struct ExpandedContent: Codable, Hashable, Sendable {
    public var originalInput: String?
    public var sections: [ExpandedSection]
    public var entityLinks: [EntityLink]?
    public var actions: [CardAction]

    enum CodingKeys: String, CodingKey {
        case originalInput
        case sections
        case entityLinks
        case actions
    }
}

public struct ClarificationOption: Codable, Hashable, Sendable {
    public var id: String
    public var label: String
    public var emoji: String?
}

public enum CardStatValue: Codable, Hashable, Sendable {
    case string(String)
    case integer(Int)
    case float(Double)
    case bool(Bool)
    case null

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Int.self) {
            self = .integer(value)
        } else if let value = try? container.decode(Double.self) {
            self = .float(value)
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else {
            self = .null
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let value):
            try container.encode(value)
        case .integer(let value):
            try container.encode(value)
        case .float(let value):
            try container.encode(value)
        case .bool(let value):
            try container.encode(value)
        case .null:
            try container.encodeNil()
        }
    }
}

public struct StreamCard: Codable, Hashable, Sendable, Identifiable {
    public var id: String
    public var occurredAt: String
    public var createdAt: String
    public var updatedAt: String
    public var version: Int

    public var cardType: CardType
    public var domain: DomainId
    public var emoji: String

    public var state: CardState
    public var processingStep: String?
    public var processingSteps: [String]?

    public var title: String
    public var subtitle: String?
    public var summary: String?
    public var durationMs: Int?

    public var image: CardImage?
    public var stats: [String: CardStatValue]?
    public var entities: [EntityRef]?

    public var originalInput: String?
    public var source: CardSource?
    public var assistantPreview: String?
    public var request: CardRequestMeta?
    public var expanded: ExpandedContent?
    public var clarificationOptions: [ClarificationOption]?
    public var errorMessage: String?

    public init(
        id: String,
        occurredAt: String,
        createdAt: String,
        updatedAt: String,
        version: Int,
        cardType: CardType,
        domain: DomainId,
        emoji: String,
        state: CardState,
        processingStep: String?,
        processingSteps: [String]?,
        title: String,
        subtitle: String?,
        summary: String?,
        durationMs: Int?,
        image: CardImage?,
        stats: [String: CardStatValue]?,
        entities: [EntityRef]?,
        originalInput: String?,
        source: CardSource?,
        assistantPreview: String?,
        request: CardRequestMeta?,
        expanded: ExpandedContent?,
        clarificationOptions: [ClarificationOption]?,
        errorMessage: String?
    ) {
        self.id = id
        self.occurredAt = occurredAt
        self.createdAt = createdAt
        self.updatedAt = updatedAt
        self.version = version
        self.cardType = cardType
        self.domain = domain
        self.emoji = emoji
        self.state = state
        self.processingStep = processingStep
        self.processingSteps = processingSteps
        self.title = title
        self.subtitle = subtitle
        self.summary = summary
        self.durationMs = durationMs
        self.image = image
        self.stats = stats
        self.entities = entities
        self.originalInput = originalInput
        self.source = source
        self.assistantPreview = assistantPreview
        self.request = request
        self.expanded = expanded
        self.clarificationOptions = clarificationOptions
        self.errorMessage = errorMessage
    }

    enum CodingKeys: String, CodingKey {
        case id
        case occurredAt
        case createdAt
        case updatedAt
        case version
        case cardType
        case domain
        case emoji
        case state
        case processingStep
        case processingSteps
        case title
        case subtitle
        case summary
        case durationMs
        case image
        case stats
        case entities
        case originalInput
        case source
        case assistantPreview
        case request
        case expanded
        case clarificationOptions
        case errorMessage
    }
}

public struct StreamCardPatch: Codable, Hashable, Sendable {
    public var state: CardState?
    public var title: String?
    public var subtitle: String?
    public var processingStep: String?
    public var processingSteps: [String]?
    public var durationMs: Int?
    public var errorMessage: String?
    public var assistantPreview: String?
    public var stats: [String: CardStatValue]?
    public var image: CardImage?
    public var expanded: ExpandedContent?
    public var clarificationOptions: [ClarificationOption]?
}

public enum LifeStreamEvent: Codable, Hashable, Sendable {
    case cardCreated(card: StreamCard)
    case cardStep(cardId: String, step: String, version: Int)
    case cardUpdated(cardId: String, patch: StreamCardPatch, version: Int)
    case cardCompleted(card: StreamCard)
    case cardError(cardId: String, message: String, version: Int)

    enum CodingKeys: String, CodingKey {
        case type
        case card
        case cardId
        case step
        case patch
        case version
        case message
    }

    enum EventType: String, Codable {
        case cardCreated = "card_created"
        case cardStep = "card_step"
        case cardUpdated = "card_updated"
        case cardCompleted = "card_completed"
        case cardError = "card_error"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(EventType.self, forKey: .type)
        switch type {
        case .cardCreated:
            let card = try container.decode(StreamCard.self, forKey: .card)
            self = .cardCreated(card: card)
        case .cardStep:
            let cardId = try container.decode(String.self, forKey: .cardId)
            let step = try container.decode(String.self, forKey: .step)
            let version = try container.decode(Int.self, forKey: .version)
            self = .cardStep(cardId: cardId, step: step, version: version)
        case .cardUpdated:
            let cardId = try container.decode(String.self, forKey: .cardId)
            let patch = try container.decode(StreamCardPatch.self, forKey: .patch)
            let version = try container.decode(Int.self, forKey: .version)
            self = .cardUpdated(cardId: cardId, patch: patch, version: version)
        case .cardCompleted:
            let card = try container.decode(StreamCard.self, forKey: .card)
            self = .cardCompleted(card: card)
        case .cardError:
            let cardId = try container.decode(String.self, forKey: .cardId)
            let message = try container.decode(String.self, forKey: .message)
            let version = try container.decode(Int.self, forKey: .version)
            self = .cardError(cardId: cardId, message: message, version: version)
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .cardCreated(let card):
            try container.encode(EventType.cardCreated, forKey: .type)
            try container.encode(card, forKey: .card)
        case .cardStep(let cardId, let step, let version):
            try container.encode(EventType.cardStep, forKey: .type)
            try container.encode(cardId, forKey: .cardId)
            try container.encode(step, forKey: .step)
            try container.encode(version, forKey: .version)
        case .cardUpdated(let cardId, let patch, let version):
            try container.encode(EventType.cardUpdated, forKey: .type)
            try container.encode(cardId, forKey: .cardId)
            try container.encode(patch, forKey: .patch)
            try container.encode(version, forKey: .version)
        case .cardCompleted(let card):
            try container.encode(EventType.cardCompleted, forKey: .type)
            try container.encode(card, forKey: .card)
        case .cardError(let cardId, let message, let version):
            try container.encode(EventType.cardError, forKey: .type)
            try container.encode(cardId, forKey: .cardId)
            try container.encode(message, forKey: .message)
            try container.encode(version, forKey: .version)
        }
    }
}

public struct LifeStreamAssetResponse: Codable, Hashable, Sendable {
    public var base64: String
    public var mime: String
}
