import Foundation
import SwiftUI
import CodexMonitorRPC
import CodexMonitorModels

@MainActor
final class CodexStore: ObservableObject {
    enum ConnectionState: Equatable {
        case disconnected
        case connecting
        case connected
        case error(String)
    }

    struct ThreadActivityStatus: Hashable {
        var isProcessing: Bool = false
        var hasUnread: Bool = false
        var isReviewing: Bool = false
        var processingStartedAt: Date?
        var lastDurationMs: Double?
    }

    struct QueuedMessage: Identifiable, Hashable {
        var id: String
        var text: String
        var createdAt: Date
        var images: [String]
        var accessMode: AccessMode?
    }

    struct DebugEntry: Identifiable, Hashable {
        var id: String
        var timestamp: Date
        var source: String
        var label: String
        var payload: JSONValue?
    }

    @Published var connectionState: ConnectionState = .disconnected
    @Published var lastError: String?
    @Published var host: String
    @Published var port: String
    @Published var token: String

    @Published var workspaces: [WorkspaceInfo] = []
    @Published var activeWorkspaceId: String?
    @Published var activeThreadIdByWorkspace: [String: String?] = [:]
    @Published var threadsByWorkspace: [String: [ThreadSummary]] = [:]
    @Published var itemsByThread: [String: [ConversationItem]] = [:]
    @Published var approvals: [ApprovalRequest] = []
    @Published var threadStatusById: [String: ThreadActivityStatus] = [:]
    @Published var activeTurnIdByThread: [String: String] = [:]
    @Published var tokenUsageByThread: [String: ThreadTokenUsage] = [:]
    @Published var rateLimitsByWorkspace: [String: RateLimitSnapshot?] = [:]
    @Published var turnPlanByThread: [String: TurnPlan?] = [:]
    @Published var lastAgentMessageByThread: [String: (text: String, timestamp: Date)] = [:]
    @Published var gitStatusByWorkspace: [String: GitStatusResponse] = [:]
    @Published var gitDiffsByWorkspace: [String: [GitFileDiff]] = [:]
    @Published var gitLogByWorkspace: [String: GitLogResponse] = [:]
    @Published var gitHubIssuesByWorkspace: [String: [GitHubIssue]] = [:]
    @Published var gitHubPullsByWorkspace: [String: [GitHubPullRequest]] = [:]
    @Published var filesByWorkspace: [String: [String]] = [:]
    @Published var promptsByWorkspace: [String: [CustomPromptOption]] = [:]
    @Published var terminalOutputBySession: [String: String] = [:]
    @Published var usageSnapshot: LocalUsageSnapshot?
    @Published var domains: [Domain] = []
    @Published var domainTrendsByWorkspace: [String: DomainTrendSnapshot] = [:]
    @Published var lifeActiveDomain: LifeDomain? = nil
    @Published var lifeTimeRange: LifeTimeRange = .today
    @Published var deliveryDashboard: DeliveryDashboard?
    @Published var nutritionDashboard: NutritionDashboard?
    @Published var exerciseDashboard: ExerciseDashboard?
    @Published var mediaLibrary: MediaLibrary?
    @Published var youtubeLibrary: YouTubeLibrary?
    @Published var financeDashboard: FinanceDashboard?
    @Published var dashboardLoading: Bool = false
    @Published var dashboardError: String? = nil
    @Published var debugEntries: [DebugEntry] = []
    @Published var queuedByThread: [String: [QueuedMessage]] = [:]
    @Published var collaborationModesByWorkspace: [String: [CollaborationModeOption]] = [:]
    @Published var selectedCollaborationModeIdByWorkspace: [String: String?] = [:]

    var lifeStreamEventHandler: ((LifeStreamEvent) -> Void)?

    private var inFlightByThread: [String: QueuedMessage?] = [:]
    private var hasStartedByThread: [String: Bool] = [:]

    private let maxItemsPerThread = 500
    private let maxTerminalCharsPerSession = 50_000

    private let rpc = RPCClient()
    private var reconnectTask: Task<Void, Never>?
    private var retryDelay: TimeInterval = 1.0
    private var isBackgrounded = false

    private var api: CodexMonitorAPI { CodexMonitorAPI(rpc: rpc) }

    private let hostKey = "codex.monitor.host"
    private let portKey = "codex.monitor.port"
    private let tokenKey = "codex.monitor.token"

    var activeWorkspacePurpose: WorkspacePurpose? {
        guard let workspaceId = activeWorkspaceId else { return nil }
        return workspaces.first(where: { $0.id == workspaceId })?.settings.purpose
    }

    init() {
        let defaults = UserDefaults.standard
        host = defaults.string(forKey: hostKey) ?? ""
        let storedPort = defaults.string(forKey: portKey) ?? "4732"
        port = storedPort
        token = KeychainHelper.readToken(key: tokenKey) ?? ""

        Task {
            await rpc.setNotificationHandler { [weak self] notification in
                Task { @MainActor in
                    self?.handleNotification(notification)
                }
            }
        }
    }

    func handleScenePhase(_ phase: ScenePhase) {
        switch phase {
        case .background:
            isBackgrounded = true
            disconnect()
        case .active:
            isBackgrounded = false
            connect()
        default:
            break
        }
    }

    func saveSettings() {
        let defaults = UserDefaults.standard
        defaults.setValue(host, forKey: hostKey)
        defaults.setValue(port, forKey: portKey)
        KeychainHelper.saveToken(token, key: tokenKey)
    }

    func connect() {
        guard !isBackgrounded else { return }
        guard connectionState != .connecting else { return }
        let trimmedHost = host.trimmingCharacters(in: .whitespacesAndNewlines)
        let portValue = UInt16(port) ?? 4732
        let trimmedToken = token.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedHost.isEmpty, !trimmedToken.isEmpty else {
            connectionState = .error("Host and token required.")
            return
        }

        connectionState = .connecting
        saveSettings()
        reconnectTask?.cancel()

        let config = RPCClient.Config(host: trimmedHost, port: portValue, token: trimmedToken)
        Task {
            do {
                try await rpc.connect(config)
                connectionState = .connected
                lastError = nil
                retryDelay = 1.0
                await refreshAfterConnect()
            } catch {
                let message = error.localizedDescription
                connectionState = .error(message)
                lastError = message
                scheduleReconnect()
            }
        }
    }

    func ping() async -> Bool {
        do {
            let response = try await api.ping()
            return response.ok
        } catch {
            lastError = error.localizedDescription
            return false
        }
    }

    func disconnect() {
        Task {
            await rpc.disconnect()
        }
        connectionState = .disconnected
    }

    private func scheduleReconnect() {
        reconnectTask?.cancel()
        reconnectTask = Task { [weak self] in
            guard let self else { return }
            let delay = retryDelay
            retryDelay = min(retryDelay * 2, 30)
            try? await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000))
            await MainActor.run { self.connect() }
        }
    }

    func refreshAfterConnect() async {
        await refreshWorkspaces()
        guard let workspaceId = activeWorkspaceId ?? workspaces.first?.id else { return }
        await connectWorkspace(id: workspaceId)
        await refreshThreads(for: workspaceId)
        await refreshCollaborationModes(workspaceId: workspaceId)
        await refreshDomains()
        await refreshUsage()
    }

    func refreshWorkspaces() async {
        do {
            let list = try await api.listWorkspaces()
            workspaces = list
            if activeWorkspaceId == nil {
                activeWorkspaceId = list.first?.id
            }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshDomains() async {
        do {
            let list = try await api.domainsList()
            domains = list
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshDomainTrends(workspaceId: String, range: String = "7d") async {
        guard let domainId = workspaces.first(where: { $0.id == workspaceId })?.settings.domainId else {
            return
        }
        do {
            let snapshot = try await api.domainTrends(workspaceId: workspaceId, domainId: domainId, range: range)
            domainTrendsByWorkspace[workspaceId] = snapshot
        } catch {
            lastError = error.localizedDescription
        }
    }

    func fetchDeliveryDashboard(range: LifeTimeRange) async {
        guard let workspaceId = activeWorkspaceId else { return }
        dashboardLoading = true
        dashboardError = nil
        do {
            let dashboard = try await api.getDeliveryDashboard(workspaceId: workspaceId, range: range.rawValue)
            deliveryDashboard = dashboard
        } catch {
            dashboardError = error.localizedDescription
        }
        dashboardLoading = false
    }

    func fetchNutritionDashboard(range: LifeTimeRange) async {
        guard let workspaceId = activeWorkspaceId else { return }
        dashboardLoading = true
        dashboardError = nil
        do {
            let dashboard = try await api.getNutritionDashboard(workspaceId: workspaceId, range: range.rawValue)
            nutritionDashboard = dashboard
        } catch {
            dashboardError = error.localizedDescription
        }
        dashboardLoading = false
    }

    func fetchExerciseDashboard(range: LifeTimeRange) async {
        guard let workspaceId = activeWorkspaceId else { return }
        dashboardLoading = true
        dashboardError = nil
        do {
            let dashboard = try await api.getExerciseDashboard(workspaceId: workspaceId, range: range.rawValue)
            exerciseDashboard = dashboard
        } catch {
            dashboardError = error.localizedDescription
        }
        dashboardLoading = false
    }

    func fetchMediaLibrary() async {
        guard let workspaceId = activeWorkspaceId else { return }
        dashboardLoading = true
        dashboardError = nil
        do {
            let library = try await api.getMediaDashboard(workspaceId: workspaceId)
            mediaLibrary = library
        } catch {
            dashboardError = error.localizedDescription
        }
        dashboardLoading = false
    }

    func fetchYouTubeLibrary() async {
        guard let workspaceId = activeWorkspaceId else { return }
        dashboardLoading = true
        dashboardError = nil
        do {
            let library = try await api.getYouTubeDashboard(workspaceId: workspaceId)
            youtubeLibrary = library
        } catch {
            dashboardError = error.localizedDescription
        }
        dashboardLoading = false
    }

    func fetchFinanceDashboard(range: LifeTimeRange) async {
        guard let workspaceId = activeWorkspaceId else { return }
        dashboardLoading = true
        dashboardError = nil
        do {
            let dashboard = try await api.getFinanceDashboard(workspaceId: workspaceId, range: range.rawValue)
            financeDashboard = dashboard
        } catch {
            dashboardError = error.localizedDescription
        }
        dashboardLoading = false
    }

    // MARK: - Life Stream RPC
    func lifeStreamLoadDay(workspaceId: String, dateIso: String) async throws -> [StreamCard] {
        return try await api.lifeStreamLoadDay(workspaceId: workspaceId, dateIso: dateIso)
    }

    func lifeStreamSubmit(
        workspaceId: String,
        cardId: String,
        input: String,
        occurredAtIso: String? = nil,
        modelId: String? = nil,
        effort: String? = nil,
        accessMode: String? = nil,
        collaborationMode: JSONValue? = nil
    ) async throws {
        try await api.lifeStreamSubmit(
            workspaceId: workspaceId,
            cardId: cardId,
            input: input,
            occurredAtIso: occurredAtIso,
            modelId: modelId,
            effort: effort,
            accessMode: accessMode,
            collaborationMode: collaborationMode
        )
    }

    func lifeStreamCancel(workspaceId: String, cardId: String) async throws {
        try await api.lifeStreamCancel(workspaceId: workspaceId, cardId: cardId)
    }

    func lifeStreamRetry(workspaceId: String, cardId: String) async throws {
        try await api.lifeStreamRetry(workspaceId: workspaceId, cardId: cardId)
    }

    func lifeStreamClarify(workspaceId: String, cardId: String, optionId: String) async throws {
        try await api.lifeStreamClarify(workspaceId: workspaceId, cardId: cardId, optionId: optionId)
    }

    func lifeStreamReadLog(workspaceId: String, limit: Int = 200) async throws -> [String] {
        return try await api.lifeStreamReadLog(workspaceId: workspaceId, limit: limit)
    }

    func lifeStreamReadAsset(workspaceId: String, path: String) async throws -> LifeStreamAssetResponse {
        return try await api.lifeStreamReadAsset(workspaceId: workspaceId, path: path)
    }

    func callRaw(method: String, params: JSONValue? = nil) async throws -> JSONValue {
        return try await api.rpc.call(method: method, params: params)
    }

    func addWorkspace(path: String, codexBin: String?) async {
        do {
            _ = try await api.addWorkspace(path: path, codexBin: codexBin)
            await refreshWorkspaces()
        } catch {
            lastError = error.localizedDescription
        }
    }

    func connectWorkspace(id: String) async {
        do {
            try await api.connectWorkspace(id: id)
            await refreshCollaborationModes(workspaceId: id)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshThreads(for workspaceId: String) async {
        guard let workspace = workspaces.first(where: { $0.id == workspaceId }) else { return }
        do {
            let response = try await api.listThreads(workspaceId: workspaceId, cursor: nil, limit: 50)
            let matches = response.data.filter { record in
                guard let cwd = record.cwd, !cwd.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                    return true
                }
                return pathsEquivalent(cwd, workspace.path)
            }
            let filtered = matches.isEmpty ? response.data : matches
            if matches.isEmpty && !response.data.isEmpty {
                logDebug(
                    source: "threads",
                    label: "threads_filter_fallback",
                    payload: .object([
                        "workspacePath": .string(workspace.path),
                        "workspaceNormalized": .string(normalizeRootPath(workspace.path)),
                        "threadCount": .number(Double(response.data.count))
                    ])
                )
            }
            let summaries = filtered.map { record -> ThreadSummary in
                let name = record.name ?? record.title ?? record.preview ?? record.id
                let updated = record.updatedAt ?? record.updated_at ?? record.createdAt ?? record.created_at ?? Date().timeIntervalSince1970 * 1000
                return ThreadSummary(id: record.id, name: name, updatedAt: updated)
            }
            threadsByWorkspace[workspaceId] = summaries.sorted { $0.updatedAt > $1.updatedAt }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func startThread(in workspaceId: String) async -> String? {
        do {
            let response = try await api.startThread(workspaceId: workspaceId)
            let thread = response.thread ?? response.result?.thread
            guard let threadId = thread?.id else { return nil }
            await refreshThreads(for: workspaceId)
            activeWorkspaceId = workspaceId
            activeThreadIdByWorkspace[workspaceId] = threadId
            return threadId
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func resumeThread(workspaceId: String, threadId: String) async {
        do {
            let response = try await api.resumeThread(workspaceId: workspaceId, threadId: threadId)
            let thread = response.thread ?? response.result?.thread
            activeWorkspaceId = workspaceId
            activeThreadIdByWorkspace[workspaceId] = threadId
            if let preview = thread?.preview, !preview.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                updateThreadName(workspaceId: workspaceId, threadId: threadId, name: preview)
            }
            if let turns = thread?.turns {
                loadThreadHistory(workspaceId: workspaceId, threadId: threadId, turns: turns)
            }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func sendMessage(
        workspaceId: String,
        threadId: String,
        text: String,
        model: String? = nil,
        effort: String? = nil,
        accessMode: AccessMode? = nil,
        images: [String]? = nil
    ) async {
        do {
            _ = try await api.sendUserMessage(
                workspaceId: workspaceId,
                threadId: threadId,
                text: text,
                model: model,
                effort: effort,
                accessMode: accessMode,
                images: images,
                collaborationMode: collaborationModeValue(for: workspaceId)
            )
        } catch {
            lastError = error.localizedDescription
            appendSystemMessage(threadId: threadId, text: "Send failed: \(error.localizedDescription)")
        }
    }

    func queueMessage(
        workspaceId: String,
        threadId: String,
        text: String,
        accessMode: AccessMode? = nil,
        images: [String] = []
    ) async {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty || !images.isEmpty else { return }
        let item = QueuedMessage(
            id: UUID().uuidString,
            text: trimmed,
            createdAt: Date(),
            images: images,
            accessMode: accessMode
        )
        enqueueMessage(threadId: threadId, item: item)
        await flushQueueIfNeeded(workspaceId: workspaceId, threadId: threadId)
    }

    func interruptTurn(workspaceId: String, threadId: String) async {
        guard let turnId = activeTurnIdByThread[threadId] else { return }
        do {
            try await api.interruptTurn(workspaceId: workspaceId, threadId: threadId, turnId: turnId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func respondToApproval(_ approval: ApprovalRequest, decision: ApprovalDecision) async {
        do {
            try await api.respondToServerRequest(workspaceId: approval.workspaceId, requestId: approval.requestId, decision: decision)
            approvals.removeAll { $0.id == approval.id }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshGitStatus(workspaceId: String) async {
        do {
            let status = try await api.getGitStatus(workspaceId: workspaceId)
            gitStatusByWorkspace[workspaceId] = status
            let diffs = try await api.getGitDiffs(workspaceId: workspaceId)
            gitDiffsByWorkspace[workspaceId] = diffs
        } catch {
            lastError = error.localizedDescription
        }
    }

    func stageGitFile(workspaceId: String, path: String) async {
        do {
            try await api.stageGitFile(workspaceId: workspaceId, path: path)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func unstageGitFile(workspaceId: String, path: String) async {
        do {
            try await api.unstageGitFile(workspaceId: workspaceId, path: path)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func revertGitFile(workspaceId: String, path: String) async {
        do {
            try await api.revertGitFile(workspaceId: workspaceId, path: path)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func stageAll(workspaceId: String) async {
        do {
            try await api.stageGitAll(workspaceId: workspaceId)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func revertAll(workspaceId: String) async {
        do {
            try await api.revertGitAll(workspaceId: workspaceId)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func commit(workspaceId: String, message: String) async {
        do {
            try await api.commitGit(workspaceId: workspaceId, message: message)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func generateCommitMessage(workspaceId: String) async -> String? {
        do {
            return try await api.generateCommitMessage(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func pull(workspaceId: String) async {
        do {
            try await api.pullGit(workspaceId: workspaceId)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func push(workspaceId: String) async {
        do {
            try await api.pushGit(workspaceId: workspaceId)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func sync(workspaceId: String) async {
        do {
            try await api.syncGit(workspaceId: workspaceId)
            await refreshGitStatus(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshGitLog(workspaceId: String) async {
        do {
            let log = try await api.getGitLog(workspaceId: workspaceId, limit: 50)
            gitLogByWorkspace[workspaceId] = log
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshGitHubIssues(workspaceId: String) async {
        do {
            let response = try await api.getGitHubIssues(workspaceId: workspaceId)
            gitHubIssuesByWorkspace[workspaceId] = response.issues
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshGitHubPulls(workspaceId: String) async {
        do {
            let response = try await api.getGitHubPullRequests(workspaceId: workspaceId)
            gitHubPullsByWorkspace[workspaceId] = response.pullRequests
        } catch {
            lastError = error.localizedDescription
        }
    }

    func fetchPullRequestDiff(workspaceId: String, prNumber: Int) async -> [GitHubPullRequestDiff] {
        do {
            return try await api.getGitHubPullRequestDiff(workspaceId: workspaceId, prNumber: prNumber)
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func fetchPullRequestComments(workspaceId: String, prNumber: Int) async -> [GitHubPullRequestComment] {
        do {
            return try await api.getGitHubPullRequestComments(workspaceId: workspaceId, prNumber: prNumber)
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func refreshFiles(workspaceId: String) async {
        do {
            let files = try await api.listWorkspaceFiles(workspaceId: workspaceId)
            filesByWorkspace[workspaceId] = files
        } catch {
            lastError = error.localizedDescription
        }
    }

    func readFile(workspaceId: String, path: String) async -> WorkspaceFileResponse? {
        do {
            return try await api.readWorkspaceFile(workspaceId: workspaceId, path: path)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func refreshPrompts(workspaceId: String) async {
        do {
            let prompts = try await api.promptsList(workspaceId: workspaceId)
            promptsByWorkspace[workspaceId] = prompts
        } catch {
            lastError = error.localizedDescription
        }
    }

    func createPrompt(workspaceId: String, scope: PromptScope, name: String, description: String?, argumentHint: String?, content: String) async {
        do {
            _ = try await api.promptsCreate(
                workspaceId: workspaceId,
                scope: scope,
                name: name,
                description: description,
                argumentHint: argumentHint,
                content: content
            )
            await refreshPrompts(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func deletePrompt(workspaceId: String, path: String) async {
        do {
            try await api.promptsDelete(workspaceId: workspaceId, path: path)
            await refreshPrompts(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshUsage() async {
        do {
            usageSnapshot = try await api.localUsageSnapshot(days: 30)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func refreshCollaborationModes(workspaceId: String) async {
        do {
            let modes = try await api.collaborationModeList(workspaceId: workspaceId)
            collaborationModesByWorkspace[workspaceId] = modes
            if let selectedId = selectedCollaborationModeIdByWorkspace[workspaceId] ?? nil,
               !modes.contains(where: { $0.id == selectedId }) {
                selectedCollaborationModeIdByWorkspace[workspaceId] = nil
            }
        } catch {
            lastError = error.localizedDescription
        }
    }

    func togglePlanMode(workspaceId: String) {
        guard let plan = planModeOption(for: workspaceId) else { return }
        if isPlanModeEnabled(workspaceId: workspaceId) {
            selectedCollaborationModeIdByWorkspace[workspaceId] = nil
        } else {
            selectedCollaborationModeIdByWorkspace[workspaceId] = plan.id
        }
    }

    func isPlanModeEnabled(workspaceId: String) -> Bool {
        guard let plan = planModeOption(for: workspaceId) else { return false }
        return selectedCollaborationModeIdByWorkspace[workspaceId] == plan.id
    }

    func hasPlanMode(workspaceId: String) -> Bool {
        return planModeOption(for: workspaceId) != nil
    }

    private func planModeOption(for workspaceId: String) -> CollaborationModeOption? {
        let modes = collaborationModesByWorkspace[workspaceId] ?? []
        return modes.first { mode in
            let id = mode.id.lowercased()
            let rawMode = mode.mode.lowercased()
            return id == "plan" || rawMode == "plan"
        }
    }

    private func collaborationModeValue(for workspaceId: String) -> JSONValue? {
        guard let selectedId = selectedCollaborationModeIdByWorkspace[workspaceId] ?? nil else {
            return nil
        }
        let modes = collaborationModesByWorkspace[workspaceId] ?? []
        guard let selected = modes.first(where: { $0.id == selectedId }) else {
            return nil
        }
        return .object(selected.value)
    }

    // MARK: - Memory
    func memoryStatus() async -> MemoryStatus? {
        do {
            return try await api.memoryStatus()
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func memorySearch(query: String, limit: Int = 10) async -> [MemorySearchResult] {
        do {
            return try await api.memorySearch(query: query, limit: limit)
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func memoryBootstrap() async -> [MemorySearchResult] {
        do {
            return try await api.memoryBootstrap()
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func memoryAppend(
        type: MemoryType,
        content: String,
        tags: [String] = [],
        workspaceId: String? = nil
    ) async -> MemoryEntry? {
        do {
            return try await api.memoryAppend(type: type, content: content, tags: tags, workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func memoryFlushNow(workspaceId: String, threadId: String, force: Bool = false) async {
        do {
            _ = try await api.memoryFlushNow(workspaceId: workspaceId, threadId: threadId, force: force)
        } catch {
            lastError = error.localizedDescription
        }
    }

    // MARK: - Browser
    func browserCreateSession(headless: Bool = true, startUrl: String? = nil) async -> BrowserSessionCreated? {
        do {
            return try await api.browserCreateSession(headless: headless, startUrl: startUrl)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func browserListSessions() async -> [String] {
        do {
            let list = try await api.browserListSessions()
            return list.sessions
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func browserNavigate(sessionId: String, url: String) async {
        do {
            _ = try await api.browserNavigate(sessionId: sessionId, url: url)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func browserScreenshot(sessionId: String) async -> BrowserScreenshot? {
        do {
            return try await api.browserScreenshot(sessionId: sessionId)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func browserClick(sessionId: String, x: Double, y: Double) async {
        do {
            _ = try await api.browserClick(sessionId: sessionId, x: x, y: y)
        } catch {
            lastError = error.localizedDescription
        }
    }

    // MARK: - Skills
    func skillsValidate(workspaceId: String) async -> [SkillValidationResult] {
        do {
            return try await api.skillsValidate(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func skillsList(workspaceId: String) async -> [SkillOption] {
        do {
            let response = try await api.skillsList(workspaceId: workspaceId)
            let skills = extractSkills(from: response)
            return skills.map { item in
                SkillOption(
                    name: item["name"]?.asString() ?? "",
                    path: item["path"]?.asString() ?? "",
                    description: item["description"]?.asString()
                )
            }.filter { !$0.name.isEmpty }
        } catch {
            lastError = error.localizedDescription
            return []
        }
    }

    func skillsConfigWrite(workspaceId: String, enabled: [SkillOption], disabled: [SkillOption]) async {
        let enabledPayload = enabled.map { JSONValue.object(["name": .string($0.name), "path": .string($0.path)]) }
        let disabledPayload = disabled.map { JSONValue.object(["name": .string($0.name), "path": .string($0.path)]) }
        let config = JSONValue.object([
            "enabled": .array(enabledPayload),
            "disabled": .array(disabledPayload),
        ])
        do {
            _ = try await api.skillsConfigWrite(workspaceId: workspaceId, config: config)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func skillsConfigRead(workspaceId: String) async -> JSONValue? {
        do {
            return try await api.skillsConfigRead(workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
            return nil
        }
    }

    func skillsInstallFromGit(sourceUrl: String, target: String, workspaceId: String? = nil) async {
        do {
            _ = try await api.skillsInstallFromGit(sourceUrl: sourceUrl, target: target, workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func skillsUninstall(name: String, target: String, workspaceId: String? = nil) async {
        do {
            _ = try await api.skillsUninstall(name: name, target: target, workspaceId: workspaceId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    private func extractSkills(from response: JSONValue) -> [JSONValue] {
        if let skills = response["result"]?["skills"]?.arrayValue ?? response["skills"]?.arrayValue {
            return skills
        }
        if let buckets = response["result"]?["data"]?.arrayValue ?? response["data"]?.arrayValue {
            return buckets.flatMap { $0["skills"]?.arrayValue ?? [] }
        }
        return []
    }

    func openTerminal(workspaceId: String, terminalId: String, cols: Int, rows: Int) async {
        do {
            _ = try await api.terminalOpen(workspaceId: workspaceId, terminalId: terminalId, cols: cols, rows: rows)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func writeTerminal(workspaceId: String, terminalId: String, data: String) async {
        do {
            try await api.terminalWrite(workspaceId: workspaceId, terminalId: terminalId, data: data)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func closeTerminal(workspaceId: String, terminalId: String) async {
        do {
            try await api.terminalClose(workspaceId: workspaceId, terminalId: terminalId)
        } catch {
            lastError = error.localizedDescription
        }
    }

    func resizeTerminal(workspaceId: String, terminalId: String, cols: Int, rows: Int) async {
        do {
            try await api.terminalResize(workspaceId: workspaceId, terminalId: terminalId, cols: cols, rows: rows)
        } catch {
            lastError = error.localizedDescription
        }
    }

    // MARK: - Notifications
    private func handleNotification(_ notification: RPCNotification) {
        switch notification.method {
        case "app-server-event":
            if let params = notification.params,
               let event = try? params.decode(AppServerEvent.self) {
                handleAppServerEvent(event)
            }
        case "terminal-output":
            if let params = notification.params,
               let event = try? params.decode(TerminalOutputEvent.self) {
                handleTerminalOutput(event)
            }
        case "life_stream_event":
            if let params = notification.params,
               let event = try? params.decode(LifeStreamEvent.self) {
                lifeStreamEventHandler?(event)
            }
        default:
            break
        }
    }

    private func handleTerminalOutput(_ event: TerminalOutputEvent) {
        let key = "\(event.workspaceId)-\(event.terminalId)"
        let existing = terminalOutputBySession[key] ?? ""
        let combined = existing + event.data
        if combined.count > maxTerminalCharsPerSession {
            terminalOutputBySession[key] = String(combined.suffix(maxTerminalCharsPerSession))
        } else {
            terminalOutputBySession[key] = combined
        }
    }

    private func handleAppServerEvent(_ event: AppServerEvent) {
        guard case .object(let message) = event.message else { return }
        let method = message["method"]?.asString() ?? ""
        logDebug(source: "event", label: method.isEmpty ? "app-server-event" : method, payload: event.message)

        if method == "codex/connected" {
            Task { await refreshWorkspaces() }
            return
        }

        if method.contains("requestApproval"),
           let id = message["id"]?.asNumber().map(Int.init) {
            let params = message["params"]?.objectValue ?? [:]
            approvals.append(
                ApprovalRequest(
                    workspaceId: event.workspaceId,
                    requestId: id,
                    method: method,
                    params: params
                )
            )
            return
        }

        if method == "item/agentMessage/delta" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString()
                ?? params["thread_id"]?.asString()
                ?? ""
            let itemId = params["itemId"]?.asString()
                ?? params["item_id"]?.asString()
                ?? ""
            let delta = params["delta"]?.asString() ?? ""
            guard !threadId.isEmpty, !itemId.isEmpty, !delta.isEmpty else { return }
            ensureThread(workspaceId: event.workspaceId, threadId: threadId)
            markProcessing(workspaceId: event.workspaceId, threadId: threadId, isProcessing: true)
            appendAgentDelta(workspaceId: event.workspaceId, threadId: threadId, itemId: itemId, delta: delta)
            return
        }

        if method == "item/completed" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString()
                ?? params["thread_id"]?.asString()
                ?? ""
            let itemValue = params["item"]
            if let itemValue {
                if let item = ConversationHelpers.buildConversationItem(from: itemValue) {
                    upsertItem(threadId: threadId, item: item)
                }
                if case .object(let itemDict) = itemValue,
                   itemDict["type"]?.asString() == "agentMessage" {
                    let itemId = itemDict["id"]?.asString() ?? ""
                    let text = itemDict["text"]?.asString() ?? ""
                    if !itemId.isEmpty {
                        completeAgentMessage(workspaceId: event.workspaceId, threadId: threadId, itemId: itemId, text: text)
                    }
                }
            }
            return
        }

        if method == "item/started" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString()
                ?? params["thread_id"]?.asString()
                ?? ""
            if let itemValue = params["item"],
               let item = ConversationHelpers.buildConversationItem(from: itemValue) {
                upsertItem(threadId: threadId, item: item)
            }
            return
        }

        if method == "item/reasoning/summaryTextDelta" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString() ?? params["thread_id"]?.asString() ?? ""
            let itemId = params["itemId"]?.asString() ?? params["item_id"]?.asString() ?? ""
            let delta = params["delta"]?.asString() ?? ""
            appendReasoning(threadId: threadId, itemId: itemId, delta: delta, isSummary: true)
            return
        }

        if method == "item/reasoning/textDelta" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString() ?? params["thread_id"]?.asString() ?? ""
            let itemId = params["itemId"]?.asString() ?? params["item_id"]?.asString() ?? ""
            let delta = params["delta"]?.asString() ?? ""
            appendReasoning(threadId: threadId, itemId: itemId, delta: delta, isSummary: false)
            return
        }

        if method == "item/commandExecution/outputDelta" || method == "item/fileChange/outputDelta" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString() ?? params["thread_id"]?.asString() ?? ""
            let itemId = params["itemId"]?.asString() ?? params["item_id"]?.asString() ?? ""
            let delta = params["delta"]?.asString() ?? ""
            appendToolOutput(threadId: threadId, itemId: itemId, delta: delta)
            return
        }

        if method == "turn/started" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString()
                ?? params["thread_id"]?.asString()
                ?? params["turn"]?.objectValue?["threadId"]?.asString()
                ?? params["turn"]?.objectValue?["thread_id"]?.asString()
                ?? ""
            let turnId = params["turnId"]?.asString()
                ?? params["turn_id"]?.asString()
                ?? params["turn"]?.objectValue?["id"]?.asString()
                ?? ""
            if !threadId.isEmpty {
                markProcessing(workspaceId: event.workspaceId, threadId: threadId, isProcessing: true)
                if !turnId.isEmpty {
                    activeTurnIdByThread[threadId] = turnId
                }
            }
            return
        }

        if method == "turn/completed" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString()
                ?? params["thread_id"]?.asString()
                ?? params["turn"]?.objectValue?["threadId"]?.asString()
                ?? params["turn"]?.objectValue?["thread_id"]?.asString()
                ?? ""
            if !threadId.isEmpty {
                markProcessing(workspaceId: event.workspaceId, threadId: threadId, isProcessing: false)
                activeTurnIdByThread.removeValue(forKey: threadId)
            }
            return
        }

        if method == "turn/plan/updated" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString() ?? params["thread_id"]?.asString() ?? ""
            if let planValue = params["plan"],
               let plan = try? planValue.decode([TurnPlanStep].self) {
                let explanation = params["explanation"]?.asString()
                let turnId = params["turnId"]?.asString() ?? params["turn_id"]?.asString() ?? ""
                turnPlanByThread[threadId] = TurnPlan(turnId: turnId, explanation: explanation, steps: plan)
            }
            return
        }

        if method == "thread/tokenUsage/updated" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString() ?? params["thread_id"]?.asString() ?? ""
            let usageValue = params["tokenUsage"] ?? params["token_usage"]
            if let usageValue,
               let usage = try? usageValue.decode(ThreadTokenUsage.self) {
                tokenUsageByThread[threadId] = usage
            }
            return
        }

        if method == "account/rateLimits/updated" {
            let params = message["params"]?.objectValue ?? [:]
            if let rateLimitsValue = params["rateLimits"] ?? params["rate_limits"],
               let snapshot = try? rateLimitsValue.decode(RateLimitSnapshot.self) {
                rateLimitsByWorkspace[event.workspaceId] = snapshot
            }
            return
        }

        if method == "error" {
            let params = message["params"]?.objectValue ?? [:]
            let threadId = params["threadId"]?.asString() ?? params["thread_id"]?.asString() ?? ""
            let errorMessage = params["error"]?.objectValue?["message"]?.asString() ?? "Turn failed."
            markProcessing(workspaceId: event.workspaceId, threadId: threadId, isProcessing: false)
            appendSystemMessage(threadId: threadId, text: errorMessage)
            return
        }
    }

    // MARK: - Item helpers
    private func storeThreadItems(threadId: String, items: [ConversationItem]) {
        var trimmed = items
        if trimmed.count > maxItemsPerThread {
            trimmed = Array(trimmed.suffix(maxItemsPerThread))
        }
        itemsByThread[threadId] = ConversationHelpers.prepareThreadItems(trimmed)
    }

    private func ensureThread(workspaceId: String, threadId: String) {
        if threadsByWorkspace[workspaceId]?.contains(where: { $0.id == threadId }) == false {
            var list = threadsByWorkspace[workspaceId] ?? []
            list.insert(ThreadSummary(id: threadId, name: threadId, updatedAt: Date().timeIntervalSince1970 * 1000), at: 0)
            threadsByWorkspace[workspaceId] = list
        }
    }

    private func appendAgentDelta(workspaceId: String, threadId: String, itemId: String, delta: String) {
        var list = itemsByThread[threadId] ?? []
        if let index = list.firstIndex(where: { $0.id == itemId }) {
            var item = list[index]
            item.text = ConversationHelpers.mergeStreamingText(existing: item.text ?? "", delta: delta)
            list[index] = item
        } else {
            list.append(ConversationItem(id: itemId, kind: .message, role: .assistant, text: delta))
        }
        storeThreadItems(threadId: threadId, items: list)
    }

    private func completeAgentMessage(workspaceId: String, threadId: String, itemId: String, text: String) {
        var list = itemsByThread[threadId] ?? []
        if let index = list.firstIndex(where: { $0.id == itemId }) {
            var item = list[index]
            item.text = text.isEmpty ? item.text : text
            list[index] = item
        } else {
            list.append(ConversationItem(id: itemId, kind: .message, role: .assistant, text: text))
        }
        storeThreadItems(threadId: threadId, items: list)

        let timestamp = Date()
        lastAgentMessageByThread[threadId] = (text: text, timestamp: timestamp)
        markProcessing(workspaceId: workspaceId, threadId: threadId, isProcessing: false)
        updateThreadTimestamp(workspaceId: workspaceId, threadId: threadId, timestamp: timestamp)
        if activeThreadIdByWorkspace[workspaceId] ?? nil != threadId {
            markUnread(threadId: threadId, hasUnread: true)
        }
    }

    private func upsertItem(threadId: String, item: ConversationItem) {
        let list = itemsByThread[threadId] ?? []
        let next = ConversationHelpers.upsertItem(list, item: item)
        storeThreadItems(threadId: threadId, items: next)
    }

    private func appendReasoning(threadId: String, itemId: String, delta: String, isSummary: Bool) {
        guard !threadId.isEmpty, !itemId.isEmpty, !delta.isEmpty else { return }
        var list = itemsByThread[threadId] ?? []
        if let index = list.firstIndex(where: { $0.id == itemId }) {
            var item = list[index]
            if isSummary {
                item.summary = ConversationHelpers.mergeStreamingText(existing: item.summary ?? "", delta: delta)
            } else {
                item.content = ConversationHelpers.mergeStreamingText(existing: item.content ?? "", delta: delta)
            }
            list[index] = item
        } else {
            let item = ConversationItem(id: itemId, kind: .reasoning, summary: isSummary ? delta : "", content: isSummary ? "" : delta)
            list.append(item)
        }
        storeThreadItems(threadId: threadId, items: list)
    }

    private func appendToolOutput(threadId: String, itemId: String, delta: String) {
        guard !threadId.isEmpty, !itemId.isEmpty else { return }
        var list = itemsByThread[threadId] ?? []
        guard let index = list.firstIndex(where: { $0.id == itemId }) else { return }
        var item = list[index]
        item.output = ConversationHelpers.mergeStreamingText(existing: item.output ?? "", delta: delta)
        list[index] = item
        storeThreadItems(threadId: threadId, items: list)
    }

    private func appendSystemMessage(threadId: String, text: String) {
        guard !threadId.isEmpty else { return }
        var list = itemsByThread[threadId] ?? []
        list.append(ConversationItem(id: UUID().uuidString, kind: .message, role: .assistant, text: text))
        storeThreadItems(threadId: threadId, items: list)
    }

    private func loadThreadHistory(workspaceId: String, threadId: String, turns: [ThreadTurn]) {
        var items: [ConversationItem] = []
        for turn in turns {
            let turnItems = turn.items ?? []
            for raw in turnItems {
                if let item = ConversationHelpers.buildConversationItem(from: raw) {
                    items.append(item)
                }
            }
        }
        logDebug(
            source: "threads",
            label: "thread_history_loaded",
            payload: .object([
                "threadId": .string(threadId),
                "turnCount": .number(Double(turns.count)),
                "itemCount": .number(Double(items.count))
            ])
        )
        if !items.isEmpty {
            storeThreadItems(threadId: threadId, items: items)
            updateThreadTimestamp(workspaceId: workspaceId, threadId: threadId, timestamp: Date())
        }
    }

    private func markProcessing(workspaceId: String, threadId: String, isProcessing: Bool) {
        var status = threadStatusById[threadId] ?? ThreadActivityStatus()
        status.isProcessing = isProcessing
        if isProcessing {
            status.processingStartedAt = Date()
        } else if let started = status.processingStartedAt {
            status.lastDurationMs = Date().timeIntervalSince(started) * 1000
        }
        threadStatusById[threadId] = status

        if isProcessing {
            if inFlightByThread[threadId] != nil, hasStartedByThread[threadId] != true {
                hasStartedByThread[threadId] = true
            }
            return
        }

        if inFlightByThread[threadId] != nil, hasStartedByThread[threadId] == true {
            inFlightByThread[threadId] = nil
            hasStartedByThread[threadId] = false
        }

        Task {
            await flushQueueIfNeeded(workspaceId: workspaceId, threadId: threadId)
        }
    }

    private func enqueueMessage(threadId: String, item: QueuedMessage) {
        queuedByThread[threadId, default: []].append(item)
    }

    private func prependQueuedMessage(threadId: String, item: QueuedMessage) {
        queuedByThread[threadId, default: []].insert(item, at: 0)
    }

    private func dequeueNext(threadId: String) -> QueuedMessage? {
        guard let queue = queuedByThread[threadId], !queue.isEmpty else { return nil }
        let next = queue[0]
        queuedByThread[threadId] = Array(queue.dropFirst())
        return next
    }

    private func flushQueueIfNeeded(workspaceId: String, threadId: String) async {
        let isProcessing = threadStatusById[threadId]?.isProcessing == true
        if isProcessing {
            return
        }
        if inFlightByThread[threadId] != nil {
            return
        }
        guard let nextItem = dequeueNext(threadId: threadId) else { return }
        inFlightByThread[threadId] = nextItem
        hasStartedByThread[threadId] = false
        do {
            _ = try await api.sendUserMessage(
                workspaceId: workspaceId,
                threadId: threadId,
                text: nextItem.text,
                model: nil,
                effort: nil,
                accessMode: nextItem.accessMode,
                images: nextItem.images,
                collaborationMode: collaborationModeValue(for: workspaceId)
            )
        } catch {
            lastError = error.localizedDescription
            appendSystemMessage(threadId: threadId, text: "Queued send failed: \(error.localizedDescription)")
            inFlightByThread[threadId] = nil
            hasStartedByThread[threadId] = false
            prependQueuedMessage(threadId: threadId, item: nextItem)
        }
    }

    private func markUnread(threadId: String, hasUnread: Bool) {
        var status = threadStatusById[threadId] ?? ThreadActivityStatus()
        status.hasUnread = hasUnread
        threadStatusById[threadId] = status
    }

    private func updateThreadTimestamp(workspaceId: String, threadId: String, timestamp: Date) {
        guard var list = threadsByWorkspace[workspaceId] else { return }
        let timeMs = timestamp.timeIntervalSince1970 * 1000
        list = list.map { summary in
            if summary.id == threadId && summary.updatedAt < timeMs {
                return ThreadSummary(id: summary.id, name: summary.name, updatedAt: timeMs)
            }
            return summary
        }
        threadsByWorkspace[workspaceId] = list.sorted { $0.updatedAt > $1.updatedAt }
    }

    private func updateThreadName(workspaceId: String, threadId: String, name: String) {
        guard var list = threadsByWorkspace[workspaceId], !name.isEmpty else { return }
        var updated = false
        list = list.map { summary in
            if summary.id == threadId && summary.name != name {
                updated = true
                return ThreadSummary(id: summary.id, name: name, updatedAt: summary.updatedAt)
            }
            return summary
        }
        if updated {
            threadsByWorkspace[workspaceId] = list.sorted { $0.updatedAt > $1.updatedAt }
        }
    }

    private func logDebug(source: String, label: String, payload: JSONValue?) {
        debugEntries.append(DebugEntry(id: UUID().uuidString, timestamp: Date(), source: source, label: label, payload: payload))
        if debugEntries.count > 200 {
            debugEntries.removeFirst(debugEntries.count - 200)
        }
    }

    private func normalizeRootPath(_ path: String) -> String {
        let standardized = URL(fileURLWithPath: path).standardizedFileURL.path
        return standardized.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
    }

    private func pathsEquivalent(_ path1: String, _ path2: String) -> Bool {
        let url1 = URL(fileURLWithPath: path1).standardized.path
        let url2 = URL(fileURLWithPath: path2).standardized.path
        return url1 == url2
    }
}
