import Foundation
import CodexMonitorModels

public struct CodexMonitorAPI: Sendable {
    public let rpc: RPCClient

    public init(rpc: RPCClient) {
        self.rpc = rpc
    }

    private func decode<T: Decodable>(_ value: JSONValue, as type: T.Type) throws -> T {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .millisecondsSince1970
        return try value.decode(type, decoder: decoder)
    }

    private func call<T: Decodable>(_ method: String, params: JSONValue? = nil, as type: T.Type) async throws -> T {
        let value = try await rpc.call(method: method, params: params)
        return try decode(value, as: type)
    }

    private func callVoid(_ method: String, params: JSONValue? = nil) async throws {
        _ = try await rpc.call(method: method, params: params)
    }

    private func unwrapResult(_ value: JSONValue) -> JSONValue {
        if case .object(let dict) = value, let inner = dict["result"] {
            return inner
        }
        return value
    }

    public func ping() async throws -> PingResponse {
        return try await call("ping", as: PingResponse.self)
    }

    // MARK: - Settings & Workspaces
    public func listWorkspaces() async throws -> [WorkspaceInfo] {
        return try await call("list_workspaces", as: [WorkspaceInfo].self)
    }

    public func addWorkspace(path: String, codexBin: String?) async throws -> WorkspaceInfo {
        var dict: [String: JSONValue] = ["path": .string(path)]
        if let codexBin {
            dict["codex_bin"] = .string(codexBin)
        }
        return try await call("add_workspace", params: .object(dict), as: WorkspaceInfo.self)
    }

    public func isWorkspacePathDir(_ path: String) async throws -> Bool {
        let value = try await rpc.call(method: "is_workspace_path_dir", params: .object(["path": .string(path)]))
        return value.boolValue ?? false
    }

    public func addClone(sourceWorkspaceId: String, copiesFolder: String, copyName: String) async throws -> WorkspaceInfo {
        return try await call(
            "add_clone",
            params: .object([
                "sourceWorkspaceId": .string(sourceWorkspaceId),
                "copiesFolder": .string(copiesFolder),
                "copyName": .string(copyName),
            ]),
            as: WorkspaceInfo.self
        )
    }

    public func addWorktree(parentId: String, branch: String) async throws -> WorkspaceInfo {
        return try await call(
            "add_worktree",
            params: .object(["parentId": .string(parentId), "branch": .string(branch)]),
            as: WorkspaceInfo.self
        )
    }

    public func updateWorkspaceSettings(id: String, settings: WorkspaceSettings) async throws -> WorkspaceInfo {
        let settingsValue = try JSONValue.fromEncodable(settings)
        return try await call(
            "update_workspace_settings",
            params: .object(["id": .string(id), "settings": settingsValue]),
            as: WorkspaceInfo.self
        )
    }

    public func updateWorkspaceCodexBin(id: String, codexBin: String?) async throws -> WorkspaceInfo {
        var dict: [String: JSONValue] = ["id": .string(id)]
        if let codexBin {
            dict["codex_bin"] = .string(codexBin)
        }
        return try await call("update_workspace_codex_bin", params: .object(dict), as: WorkspaceInfo.self)
    }

    public func removeWorkspace(id: String) async throws {
        try await callVoid("remove_workspace", params: .object(["id": .string(id)]))
    }

    public func removeWorktree(id: String) async throws {
        try await callVoid("remove_worktree", params: .object(["id": .string(id)]))
    }

    public func renameWorktree(id: String, branch: String) async throws -> WorkspaceInfo {
        return try await call(
            "rename_worktree",
            params: .object(["id": .string(id), "branch": .string(branch)]),
            as: WorkspaceInfo.self
        )
    }

    public func renameWorktreeUpstream(id: String, oldBranch: String, newBranch: String) async throws {
        try await callVoid(
            "rename_worktree_upstream",
            params: .object(["id": .string(id), "oldBranch": .string(oldBranch), "newBranch": .string(newBranch)])
        )
    }

    public func applyWorktreeChanges(workspaceId: String) async throws {
        try await callVoid("apply_worktree_changes", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func connectWorkspace(id: String) async throws {
        try await callVoid("connect_workspace", params: .object(["id": .string(id)]))
    }

    public func getAppSettings() async throws -> AppSettings {
        return try await call("get_app_settings", as: AppSettings.self)
    }

    public func updateAppSettings(_ settings: AppSettings) async throws -> AppSettings {
        let settingsValue = try JSONValue.fromEncodable(settings)
        return try await call(
            "update_app_settings",
            params: .object(["settings": settingsValue]),
            as: AppSettings.self
        )
    }

    public func codexDoctor(codexBin: String? = nil) async throws -> CodexDoctorResult {
        var dict: [String: JSONValue] = [:]
        if let codexBin {
            dict["codexBin"] = .string(codexBin)
        }
        return try await call("codex_doctor", params: dict.isEmpty ? nil : .object(dict), as: CodexDoctorResult.self)
    }

    // MARK: - Domains
    public func domainsList() async throws -> [Domain] {
        return try await call("domains_list", as: [Domain].self)
    }

    public func domainsCreate(_ domain: Domain) async throws -> Domain {
        let value = try JSONValue.fromEncodable(domain)
        return try await call("domains_create", params: value, as: Domain.self)
    }

    public func domainsUpdate(_ domain: Domain) async throws -> Domain {
        let value = try JSONValue.fromEncodable(domain)
        return try await call("domains_update", params: value, as: Domain.self)
    }

    public func domainsDelete(_ domainId: String) async throws {
        try await callVoid("domains_delete", params: .object(["domainId": .string(domainId)]))
    }

    public func domainTrends(
        workspaceId: String,
        domainId: String,
        range: String
    ) async throws -> DomainTrendSnapshot {
        return try await call(
            "domain_trends",
            params: .object([
                "workspaceId": .string(workspaceId),
                "domainId": .string(domainId),
                "range": .string(range),
            ]),
            as: DomainTrendSnapshot.self
        )
    }

    public func getDeliveryDashboard(workspaceId: String, range: String) async throws -> DeliveryDashboard {
        return try await call(
            "get_delivery_dashboard",
            params: .object([
                "workspaceId": .string(workspaceId),
                "range": .string(range),
            ]),
            as: DeliveryDashboard.self
        )
    }

    public func getNutritionDashboard(workspaceId: String, range: String) async throws -> NutritionDashboard {
        return try await call(
            "get_nutrition_dashboard",
            params: .object([
                "workspaceId": .string(workspaceId),
                "range": .string(range),
            ]),
            as: NutritionDashboard.self
        )
    }

    public func getExerciseDashboard(workspaceId: String, range: String) async throws -> ExerciseDashboard {
        return try await call(
            "get_exercise_dashboard",
            params: .object([
                "workspaceId": .string(workspaceId),
                "range": .string(range),
            ]),
            as: ExerciseDashboard.self
        )
    }

    public func getMediaDashboard(workspaceId: String) async throws -> MediaLibrary {
        return try await call(
            "get_media_dashboard",
            params: .object([
                "workspaceId": .string(workspaceId),
            ]),
            as: MediaLibrary.self
        )
    }

    public func getYouTubeDashboard(workspaceId: String) async throws -> YouTubeLibrary {
        return try await call(
            "get_youtube_dashboard",
            params: .object([
                "workspaceId": .string(workspaceId),
            ]),
            as: YouTubeLibrary.self
        )
    }

    public func getFinanceDashboard(workspaceId: String, range: String) async throws -> FinanceDashboard {
        return try await call(
            "get_finance_dashboard",
            params: .object([
                "workspaceId": .string(workspaceId),
                "range": .string(range),
            ]),
            as: FinanceDashboard.self
        )
    }

    // MARK: - Life Stream
    public func lifeStreamLoadDay(workspaceId: String, dateIso: String) async throws -> [StreamCard] {
        return try await call(
            "life_stream_load_day",
            params: .object([
                "workspaceId": .string(workspaceId),
                "dateIso": .string(dateIso),
            ]),
            as: [StreamCard].self
        )
    }

    public func lifeStreamSubmit(
        workspaceId: String,
        cardId: String,
        input: String,
        occurredAtIso: String? = nil,
        modelId: String? = nil,
        effort: String? = nil,
        accessMode: String? = nil,
        collaborationMode: JSONValue? = nil
    ) async throws {
        var params: [String: JSONValue] = [
            "workspaceId": .string(workspaceId),
            "cardId": .string(cardId),
            "input": .string(input),
        ]
        if let occurredAtIso { params["occurredAtIso"] = .string(occurredAtIso) }
        if let modelId { params["modelId"] = .string(modelId) }
        if let effort { params["effort"] = .string(effort) }
        if let accessMode { params["accessMode"] = .string(accessMode) }
        if let collaborationMode { params["collaborationMode"] = collaborationMode }
        try await callVoid("life_stream_submit", params: .object(params))
    }

    public func lifeStreamCancel(workspaceId: String, cardId: String) async throws {
        try await callVoid(
            "life_stream_cancel",
            params: .object([
                "workspaceId": .string(workspaceId),
                "cardId": .string(cardId),
            ])
        )
    }

    public func lifeStreamRetry(workspaceId: String, cardId: String) async throws {
        try await callVoid(
            "life_stream_retry",
            params: .object([
                "workspaceId": .string(workspaceId),
                "cardId": .string(cardId),
            ])
        )
    }

    public func lifeStreamClarify(
        workspaceId: String,
        cardId: String,
        optionId: String
    ) async throws {
        try await callVoid(
            "life_stream_clarify",
            params: .object([
                "workspaceId": .string(workspaceId),
                "cardId": .string(cardId),
                "optionId": .string(optionId),
            ])
        )
    }

    public func lifeStreamReadLog(workspaceId: String, limit: Int = 200) async throws -> [String] {
        return try await call(
            "life_stream_read_log",
            params: .object([
                "workspaceId": .string(workspaceId),
                "limit": .number(Double(limit)),
            ]),
            as: [String].self
        )
    }

    public func lifeStreamReadAsset(
        workspaceId: String,
        path: String
    ) async throws -> LifeStreamAssetResponse {
        return try await call(
            "life_stream_read_asset",
            params: .object([
                "workspaceId": .string(workspaceId),
                "path": .string(path),
            ]),
            as: LifeStreamAssetResponse.self
        )
    }

    // MARK: - Memory
    public func memoryStatus() async throws -> MemoryStatus {
        return try await call("memory_status", as: MemoryStatus.self)
    }

    public func memorySearch(query: String, limit: Int = 10) async throws -> [MemorySearchResult] {
        return try await call(
            "memory_search",
            params: .object([
                "query": .string(query),
                "limit": .number(Double(limit)),
            ]),
            as: [MemorySearchResult].self
        )
    }

    public func memoryAppend(
        type: MemoryType,
        content: String,
        tags: [String] = [],
        workspaceId: String? = nil
    ) async throws -> MemoryEntry {
        var params: [String: JSONValue] = [
            "type": .string(type.rawValue),
            "content": .string(content),
            "tags": .array(tags.map { .string($0) }),
        ]
        if let workspaceId {
            params["workspace_id"] = .string(workspaceId)
        }
        return try await call("memory_append", params: .object(params), as: MemoryEntry.self)
    }

    public func memoryBootstrap() async throws -> [MemorySearchResult] {
        return try await call("memory_bootstrap", params: .object([:]), as: [MemorySearchResult].self)
    }

    public func memoryFlushNow(workspaceId: String, threadId: String, force: Bool = false) async throws -> JSONValue {
        return try await rpc.call(
            method: "memory_flush_now",
            params: .object([
                "workspaceId": .string(workspaceId),
                "threadId": .string(threadId),
                "force": .bool(force),
            ])
        )
    }

    // MARK: - Browser
    public func browserCreateSession(headless: Bool = true, viewport: [String: JSONValue]? = nil, userDataDir: String? = nil, startUrl: String? = nil) async throws -> BrowserSessionCreated {
        var params: [String: JSONValue] = ["headless": .bool(headless)]
        if let viewport { params["viewport"] = .object(viewport) }
        if let userDataDir { params["userDataDir"] = .string(userDataDir) }
        if let startUrl { params["startUrl"] = .string(startUrl) }
        return try await call("browser_create_session", params: .object(params), as: BrowserSessionCreated.self)
    }

    public func browserListSessions() async throws -> BrowserSessionList {
        return try await call("browser_list_sessions", params: .object([:]), as: BrowserSessionList.self)
    }

    public func browserCloseSession(sessionId: String) async throws -> JSONValue {
        return try await rpc.call(method: "browser_close_session", params: .object(["sessionId": .string(sessionId)]))
    }

    public func browserNavigate(sessionId: String, url: String, waitUntil: String? = nil, timeoutMs: Int? = nil) async throws -> JSONValue {
        var params: [String: JSONValue] = ["sessionId": .string(sessionId), "url": .string(url)]
        if let waitUntil { params["waitUntil"] = .string(waitUntil) }
        if let timeoutMs { params["timeoutMs"] = .number(Double(timeoutMs)) }
        return try await rpc.call(method: "browser_navigate", params: .object(params))
    }

    public func browserScreenshot(sessionId: String, fullPage: Bool = true) async throws -> BrowserScreenshot {
        return try await call(
            "browser_screenshot",
            params: .object(["sessionId": .string(sessionId), "fullPage": .bool(fullPage)]),
            as: BrowserScreenshot.self
        )
    }

    public func browserClick(sessionId: String, selector: String? = nil, x: Double? = nil, y: Double? = nil) async throws -> JSONValue {
        var params: [String: JSONValue] = ["sessionId": .string(sessionId)]
        if let selector { params["selector"] = .string(selector) }
        if let x { params["x"] = .number(x) }
        if let y { params["y"] = .number(y) }
        return try await rpc.call(method: "browser_click", params: .object(params))
    }

    public func browserType(sessionId: String, selector: String, text: String, clearFirst: Bool = false) async throws -> JSONValue {
        return try await rpc.call(
            method: "browser_type",
            params: .object([
                "sessionId": .string(sessionId),
                "selector": .string(selector),
                "text": .string(text),
                "clearFirst": .bool(clearFirst),
            ])
        )
    }

    public func browserPress(sessionId: String, key: String) async throws -> JSONValue {
        return try await rpc.call(method: "browser_press", params: .object(["sessionId": .string(sessionId), "key": .string(key)]))
    }

    public func browserEvaluate(sessionId: String, js: String) async throws -> JSONValue {
        return try await rpc.call(method: "browser_evaluate", params: .object(["sessionId": .string(sessionId), "js": .string(js)]))
    }

    public func browserSnapshot(sessionId: String, fullPage: Bool = true) async throws -> BrowserSnapshot {
        return try await call(
            "browser_snapshot",
            params: .object(["sessionId": .string(sessionId), "fullPage": .bool(fullPage)]),
            as: BrowserSnapshot.self
        )
    }

    // MARK: - Skills
    public func skillsList(workspaceId: String) async throws -> JSONValue {
        return try await rpc.call(method: "skills_list", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func skillsConfigWrite(workspaceId: String, config: JSONValue) async throws -> JSONValue {
        return try await rpc.call(
            method: "skills_config_write",
            params: .object(["workspaceId": .string(workspaceId), "config": config])
        )
    }

    public func skillsConfigRead(workspaceId: String) async throws -> JSONValue {
        return try await rpc.call(
            method: "skills_config_read",
            params: .object(["workspaceId": .string(workspaceId)])
        )
    }

    public func skillsValidate(workspaceId: String) async throws -> [SkillValidationResult] {
        return try await call("skills_validate", params: .object(["workspaceId": .string(workspaceId)]), as: [SkillValidationResult].self)
    }

    public func skillsInstallFromGit(sourceUrl: String, target: String, workspaceId: String? = nil) async throws -> JSONValue {
        var params: [String: JSONValue] = [
            "sourceUrl": .string(sourceUrl),
            "target": .string(target),
        ]
        if let workspaceId { params["workspaceId"] = .string(workspaceId) }
        return try await rpc.call(method: "skills_install_from_git", params: .object(params))
    }

    public func skillsUninstall(name: String, target: String, workspaceId: String? = nil) async throws -> JSONValue {
        var params: [String: JSONValue] = [
            "name": .string(name),
            "target": .string(target),
        ]
        if let workspaceId { params["workspaceId"] = .string(workspaceId) }
        return try await rpc.call(method: "skills_uninstall", params: .object(params))
    }

    // MARK: - Threads / Codex
    public func startThread(workspaceId: String) async throws -> ThreadStartResponse {
        let value = try await rpc.call(method: "start_thread", params: .object(["workspaceId": .string(workspaceId)]))
        return try decode(unwrapResult(value), as: ThreadStartResponse.self)
    }

    public func resumeThread(workspaceId: String, threadId: String) async throws -> ThreadResumeResponse {
        let value = try await rpc.call(
            method: "resume_thread",
            params: .object(["workspaceId": .string(workspaceId), "threadId": .string(threadId)])
        )
        return try decode(unwrapResult(value), as: ThreadResumeResponse.self)
    }

    public func listThreads(workspaceId: String, cursor: String? = nil, limit: Int? = nil) async throws -> ThreadListResponse {
        var dict: [String: JSONValue] = ["workspaceId": .string(workspaceId)]
        if let cursor {
            dict["cursor"] = .string(cursor)
        }
        if let limit {
            dict["limit"] = .number(Double(limit))
        }
        let value = try await rpc.call(method: "list_threads", params: .object(dict))
        return try decode(unwrapResult(value), as: ThreadListResponse.self)
    }

    public func archiveThread(workspaceId: String, threadId: String) async throws {
        try await callVoid("archive_thread", params: .object(["workspaceId": .string(workspaceId), "threadId": .string(threadId)]))
    }

    public func sendUserMessage(
        workspaceId: String,
        threadId: String,
        text: String,
        model: String? = nil,
        effort: String? = nil,
        accessMode: AccessMode? = nil,
        images: [String]? = nil,
        collaborationMode: JSONValue? = nil
    ) async throws -> JSONValue {
        var dict: [String: JSONValue] = [
            "workspaceId": .string(workspaceId),
            "threadId": .string(threadId),
            "text": .string(text),
        ]
        if let model {
            dict["model"] = .string(model)
        }
        if let effort {
            dict["effort"] = .string(effort)
        }
        if let accessMode {
            dict["accessMode"] = .string(accessMode.rawValue)
        }
        if let images {
            dict["images"] = .array(images.map { .string($0) })
        }
        if let collaborationMode {
            dict["collaborationMode"] = collaborationMode
        }
        return try await rpc.call(method: "send_user_message", params: .object(dict))
    }

    public func interruptTurn(workspaceId: String, threadId: String, turnId: String) async throws {
        try await callVoid(
            "turn_interrupt",
            params: .object([
                "workspaceId": .string(workspaceId),
                "threadId": .string(threadId),
                "turnId": .string(turnId),
            ])
        )
    }

    public func startReview(workspaceId: String, threadId: String, target: ReviewTarget, delivery: ReviewDelivery? = nil) async throws -> JSONValue {
        var dict: [String: JSONValue] = [
            "workspaceId": .string(workspaceId),
            "threadId": .string(threadId),
            "target": try JSONValue.fromEncodable(target),
        ]
        if let delivery {
            dict["delivery"] = .string(delivery.rawValue)
        }
        return try await rpc.call(method: "start_review", params: .object(dict))
    }

    public func respondToServerRequest(workspaceId: String, requestId: Int, decision: ApprovalDecision) async throws {
        try await callVoid(
            "respond_to_server_request",
            params: .object([
                "workspaceId": .string(workspaceId),
                "requestId": .number(Double(requestId)),
                "result": .object(["decision": .string(decision.rawValue)]),
            ])
        )
    }

    public func rememberApprovalRule(workspaceId: String, command: [String]) async throws {
        try await callVoid(
            "remember_approval_rule",
            params: .object([
                "workspaceId": .string(workspaceId),
                "command": .array(command.map { .string($0) }),
            ])
        )
    }

    public func modelList(workspaceId: String) async throws -> [ModelOption] {
        return try await call("model_list", params: .object(["workspaceId": .string(workspaceId)]), as: [ModelOption].self)
    }

    public func collaborationModeList(workspaceId: String) async throws -> [CollaborationModeOption] {
        return try await call("collaboration_mode_list", params: .object(["workspaceId": .string(workspaceId)]), as: [CollaborationModeOption].self)
    }

    public func accountRateLimits(workspaceId: String) async throws -> [RateLimitSnapshot] {
        return try await call("account_rate_limits", params: .object(["workspaceId": .string(workspaceId)]), as: [RateLimitSnapshot].self)
    }

    public func getCommitMessagePrompt(workspaceId: String) async throws -> String {
        return try await call("get_commit_message_prompt", params: .object(["workspaceId": .string(workspaceId)]), as: String.self)
    }

    public func generateCommitMessage(workspaceId: String) async throws -> String {
        return try await call("generate_commit_message", params: .object(["workspaceId": .string(workspaceId)]), as: String.self)
    }

    // MARK: - Git
    public func listGitRoots(workspaceId: String, depth: Int) async throws -> [String] {
        return try await call(
            "list_git_roots",
            params: .object(["workspaceId": .string(workspaceId), "depth": .number(Double(depth))]),
            as: [String].self
        )
    }

    public func getGitStatus(workspaceId: String) async throws -> GitStatusResponse {
        return try await call("get_git_status", params: .object(["workspaceId": .string(workspaceId)]), as: GitStatusResponse.self)
    }

    public func getGitDiffs(workspaceId: String) async throws -> [GitFileDiff] {
        return try await call("get_git_diffs", params: .object(["workspaceId": .string(workspaceId)]), as: [GitFileDiff].self)
    }

    public func getGitLog(workspaceId: String, limit: Int? = nil) async throws -> GitLogResponse {
        var dict: [String: JSONValue] = ["workspaceId": .string(workspaceId)]
        if let limit {
            dict["limit"] = .number(Double(limit))
        }
        return try await call("get_git_log", params: .object(dict), as: GitLogResponse.self)
    }

    public func getGitCommitDiff(workspaceId: String, sha: String) async throws -> [GitCommitDiff] {
        return try await call(
            "get_git_commit_diff",
            params: .object(["workspaceId": .string(workspaceId), "sha": .string(sha)]),
            as: [GitCommitDiff].self
        )
    }

    public func getGitRemote(workspaceId: String) async throws -> String? {
        return try await call("get_git_remote", params: .object(["workspaceId": .string(workspaceId)]), as: String?.self)
    }

    public func stageGitFile(workspaceId: String, path: String) async throws {
        try await callVoid("stage_git_file", params: .object(["workspaceId": .string(workspaceId), "path": .string(path)]))
    }

    public func stageGitAll(workspaceId: String) async throws {
        try await callVoid("stage_git_all", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func unstageGitFile(workspaceId: String, path: String) async throws {
        try await callVoid("unstage_git_file", params: .object(["workspaceId": .string(workspaceId), "path": .string(path)]))
    }

    public func revertGitFile(workspaceId: String, path: String) async throws {
        try await callVoid("revert_git_file", params: .object(["workspaceId": .string(workspaceId), "path": .string(path)]))
    }

    public func revertGitAll(workspaceId: String) async throws {
        try await callVoid("revert_git_all", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func commitGit(workspaceId: String, message: String) async throws {
        try await callVoid("commit_git", params: .object(["workspaceId": .string(workspaceId), "message": .string(message)]))
    }

    public func pullGit(workspaceId: String) async throws {
        try await callVoid("pull_git", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func pushGit(workspaceId: String) async throws {
        try await callVoid("push_git", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func syncGit(workspaceId: String) async throws {
        try await callVoid("sync_git", params: .object(["workspaceId": .string(workspaceId)]))
    }

    public func listGitBranches(workspaceId: String) async throws -> [BranchInfo] {
        return try await call("list_git_branches", params: .object(["workspaceId": .string(workspaceId)]), as: [BranchInfo].self)
    }

    public func checkoutGitBranch(workspaceId: String, name: String) async throws {
        try await callVoid("checkout_git_branch", params: .object(["workspaceId": .string(workspaceId), "name": .string(name)]))
    }

    public func createGitBranch(workspaceId: String, name: String) async throws {
        try await callVoid("create_git_branch", params: .object(["workspaceId": .string(workspaceId), "name": .string(name)]))
    }

    public func getGitHubIssues(workspaceId: String) async throws -> GitHubIssuesResponse {
        return try await call("get_github_issues", params: .object(["workspaceId": .string(workspaceId)]), as: GitHubIssuesResponse.self)
    }

    public func getGitHubPullRequests(workspaceId: String) async throws -> GitHubPullRequestsResponse {
        return try await call("get_github_pull_requests", params: .object(["workspaceId": .string(workspaceId)]), as: GitHubPullRequestsResponse.self)
    }

    public func getGitHubPullRequestDiff(workspaceId: String, prNumber: Int) async throws -> [GitHubPullRequestDiff] {
        return try await call(
            "get_github_pull_request_diff",
            params: .object(["workspaceId": .string(workspaceId), "prNumber": .number(Double(prNumber))]),
            as: [GitHubPullRequestDiff].self
        )
    }

    public func getGitHubPullRequestComments(workspaceId: String, prNumber: Int) async throws -> [GitHubPullRequestComment] {
        return try await call(
            "get_github_pull_request_comments",
            params: .object(["workspaceId": .string(workspaceId), "prNumber": .number(Double(prNumber))]),
            as: [GitHubPullRequestComment].self
        )
    }

    // MARK: - Files
    public func listWorkspaceFiles(workspaceId: String) async throws -> [String] {
        return try await call("list_workspace_files", params: .object(["workspaceId": .string(workspaceId)]), as: [String].self)
    }

    public func readWorkspaceFile(workspaceId: String, path: String) async throws -> WorkspaceFileResponse {
        return try await call(
            "read_workspace_file",
            params: .object(["workspaceId": .string(workspaceId), "path": .string(path)]),
            as: WorkspaceFileResponse.self
        )
    }

    // MARK: - Prompts
    public func promptsList(workspaceId: String) async throws -> [CustomPromptOption] {
        return try await call("prompts_list", params: .object(["workspaceId": .string(workspaceId)]), as: [CustomPromptOption].self)
    }

    public func promptsCreate(workspaceId: String, scope: PromptScope, name: String, description: String?, argumentHint: String?, content: String) async throws -> CustomPromptOption {
        var dict: [String: JSONValue] = [
            "workspaceId": .string(workspaceId),
            "scope": .string(scope.rawValue),
            "name": .string(name),
            "content": .string(content),
        ]
        if let description {
            dict["description"] = .string(description)
        }
        if let argumentHint {
            dict["argumentHint"] = .string(argumentHint)
        }
        return try await call("prompts_create", params: .object(dict), as: CustomPromptOption.self)
    }

    public func promptsUpdate(workspaceId: String, path: String, name: String, description: String?, argumentHint: String?, content: String) async throws -> CustomPromptOption {
        var dict: [String: JSONValue] = [
            "workspaceId": .string(workspaceId),
            "path": .string(path),
            "name": .string(name),
            "content": .string(content),
        ]
        if let description {
            dict["description"] = .string(description)
        }
        if let argumentHint {
            dict["argumentHint"] = .string(argumentHint)
        }
        return try await call("prompts_update", params: .object(dict), as: CustomPromptOption.self)
    }

    public func promptsDelete(workspaceId: String, path: String) async throws {
        try await callVoid("prompts_delete", params: .object(["workspaceId": .string(workspaceId), "path": .string(path)]))
    }

    public func promptsMove(workspaceId: String, path: String, scope: PromptScope) async throws -> CustomPromptOption {
        return try await call(
            "prompts_move",
            params: .object(["workspaceId": .string(workspaceId), "path": .string(path), "scope": .string(scope.rawValue)]),
            as: CustomPromptOption.self
        )
    }

    public func promptsWorkspaceDir(workspaceId: String) async throws -> String {
        return try await call("prompts_workspace_dir", params: .object(["workspaceId": .string(workspaceId)]), as: String.self)
    }

    public func promptsGlobalDir() async throws -> String {
        return try await call("prompts_global_dir", as: String.self)
    }

    // MARK: - Terminal
    public func terminalOpen(workspaceId: String, terminalId: String, cols: Int, rows: Int) async throws -> TerminalSessionInfo {
        return try await call(
            "terminal_open",
            params: .object([
                "workspaceId": .string(workspaceId),
                "terminalId": .string(terminalId),
                "cols": .number(Double(cols)),
                "rows": .number(Double(rows)),
            ]),
            as: TerminalSessionInfo.self
        )
    }

    public func terminalWrite(workspaceId: String, terminalId: String, data: String) async throws {
        try await callVoid(
            "terminal_write",
            params: .object([
                "workspaceId": .string(workspaceId),
                "terminalId": .string(terminalId),
                "data": .string(data),
            ])
        )
    }

    public func terminalResize(workspaceId: String, terminalId: String, cols: Int, rows: Int) async throws {
        try await callVoid(
            "terminal_resize",
            params: .object([
                "workspaceId": .string(workspaceId),
                "terminalId": .string(terminalId),
                "cols": .number(Double(cols)),
                "rows": .number(Double(rows)),
            ])
        )
    }

    public func terminalClose(workspaceId: String, terminalId: String) async throws {
        try await callVoid(
            "terminal_close",
            params: .object([
                "workspaceId": .string(workspaceId),
                "terminalId": .string(terminalId),
            ])
        )
    }

    // MARK: - Usage
    public func localUsageSnapshot(days: Int, workspacePath: String? = nil) async throws -> LocalUsageSnapshot {
        var dict: [String: JSONValue] = ["days": .number(Double(days))]
        if let workspacePath {
            dict["workspacePath"] = .string(workspacePath)
        }
        return try await call("local_usage_snapshot", params: .object(dict), as: LocalUsageSnapshot.self)
    }
}
