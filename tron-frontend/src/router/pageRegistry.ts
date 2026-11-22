import { type ComponentType, type LazyExoticComponent } from "react";
import { lazyRoute as lazy } from "./lazyRoute";

type RoutePageComponent = LazyExoticComponent<ComponentType>;

export const routePages: Record<string, RoutePageComponent> = {
  "capability.page": lazy(() => import("@/domain/capability-pages/CapabilityPage").then(m => ({ default: m.CapabilityPage }))),
  "assistants.index": lazy(() => import("@/domain/assistant/pages/AssistantsPage").then(module => ({ default: module.AssistantsPage }))),
  "notifications.index": lazy(() => import("@/domain/notification/pages/NotificationsPage").then(module => ({ default: module.NotificationsPage }))),
  "observability.logs": lazy(() => import("@/domain/observability/pages/LogsPage").then(module => ({default:module.LogsPage}))),
  "observability.dashboard": lazy(() => import("@/domain/observability/pages/DashboardPage").then(module => ({default:module.DashboardPage}))),
  "observability.traceDetail": lazy(() => import("@/domain/observability/pages/TraceDetailPage").then((module) => ({ default: module.TraceDetailPage }))),
  "observability.traces": lazy(() =>
    import("@/domain/observability/pages/TracesPage").then((module) => ({
      default: module.TracesPage,
    }))
  ),
  "observability.metrics": lazy(() =>
    import("@/domain/observability/pages/MetricsPage").then((module) => ({ default: module.MetricsPage }))
  ),
  "admin.sessionManagement": lazy(() =>
    import("@/domain/admin/pages/SessionManagementPage").then((module) => ({
      default: module.SessionManagementPage,
    }))
  ),
  "admin.auditLogs": lazy(() =>
    import("@/domain/admin/pages/AuditLogsPage").then((module) => ({
      default: module.AuditLogsPage,
    }))
  ),
  "admin.navigationDomains": lazy(() =>
    import("@/domain/navigation/pages/NavigationDomainsPage").then(
      (module) => ({ default: module.NavigationDomainsPage })
    )
  ),
  "admin.navigationMenus": lazy(() =>
    import("@/domain/navigation/pages/NavigationMenusPage").then((module) => ({
      default: module.NavigationMenusPage,
    }))
  ),
  "admin.navigationTree": lazy(() =>
    import("@/domain/navigation/pages/NavigationDomainMenuTreePage").then(
      (module) => ({ default: module.NavigationDomainMenuTreePage })
    )
  ),
  "admin.workspace": lazy(() =>
    import("@/domain/admin/pages/WorkspaceManagementPage").then((module) => ({
      default: module.WorkspaceManagementPage,
    }))
  ),
  "agent.main": lazy(() =>
    import("@/domain/agent/pages/MainAgentPage").then((module) => ({
      default: module.MainAgentPage,
    }))
  ),
  "agents.index": lazy(() =>
    import("@/domain/agent/pages/AgentsIndexRedirectPage").then((module) => ({
      default: module.AgentsIndexRedirectPage,
    }))
  ),
  "agents.internal": lazy(() =>
    import("@/domain/agent/pages/InternalAgentsPage").then((module) => ({
      default: module.InternalAgentsPage,
    }))
  ),
  "agents.system": lazy(() =>
    import("@/domain/agent/pages/SystemAgentsPage").then((module) => ({
      default: module.SystemAgentsPage,
    }))
  ),
  "apps.index": lazy(() =>
    import("@/domain/app/pages/AppsPage").then((module) => ({
      default: module.AppsPage,
    }))
  ),
  "apps.new": lazy(() =>
    import("@/domain/app/pages/CreateAppPage").then((module) => ({
      default: module.CreateAppPage,
    }))
  ),
  "apps.detail": lazy(() =>
    import("@/domain/app/pages/AppDetailsPage").then((module) => ({
      default: module.AppDetailsPage,
    }))
  ),
  "apps.edit": lazy(() =>
    import("@/domain/app/pages/EditAppPage").then((module) => ({
      default: module.EditAppPage,
    }))
  ),
  "automations.detail": lazy(() =>
    import("@/domain/automation/pages/AutomationDetailsPage").then(
      (module) => ({ default: module.AutomationDetailsPage })
    )
  ),
  "automations.executions": lazy(() =>
    import("@/domain/automation/pages/AutomationExecutionsPage").then(
      (module) => ({ default: module.AutomationExecutionsPage })
    )
  ),
  "automations.index": lazy(() =>
    import("@/domain/automation/pages/AutomationsPage").then((module) => ({
      default: module.AutomationsPage,
    }))
  ),
  "automations.new": lazy(() =>
    import("@/domain/automation/pages/CreateAutomationPage").then((module) => ({
      default: module.CreateAutomationPage,
    }))
  ),
  "chats.index": lazy(() => import("@/domain/chat/pages/Index")),
  "topics.detail": lazy(() => import("@/domain/project/pages/TopicPage").then((module) => ({ default: module.TopicPage }))),
  "chats.execution": lazy(() =>
    import("@/domain/chat/pages/ChatSessionExecutionPage").then((module) => ({
      default: module.ChatSessionExecutionPage,
    }))
  ),
  "chats.reasoning": lazy(() =>
    import("@/domain/chat/pages/ChatSessionReasoningPage").then((module) => ({
      default: module.ChatSessionReasoningPage,
    }))
  ),
  "knowledgeBase.index": lazy(() =>
    import("@/domain/knowledge-base/pages/KnowledgeBasePage").then(
      (module) => ({ default: module.KnowledgeBasePage })
    )
  ),
  "knowledgeBase.detail": lazy(() =>
    import("@/domain/knowledge-base/pages/KnowledgeBaseDetailPage").then(
      (module) => ({ default: module.KnowledgeBaseDetailPage })
    )
  ),
  "knowledgeGraph.domainFiltering": lazy(() =>
    import("@/domain/knowledge-graph/pages/KnowledgeGraphDomainPage").then(
      (module) => ({ default: module.KnowledgeGraphDomainPage })
    )
  ),
  "knowledgeGraph.index": lazy(() =>
    import("@/domain/knowledge-graph/pages/KnowledgeGraphPage").then(
      (module) => ({ default: module.KnowledgeGraphPage })
    )
  ),
  "memory.details": lazy(() =>
    import("@/domain/memory/pages/MemoryDetailsPage").then((module) => ({
      default: module.MemoryDetailsPage,
    }))
  ),
  "memory.index": lazy(() =>
    import("@/domain/memory/pages/MemoryPage").then((module) => ({
      default: module.MemoryPage,
    }))
  ),
  "memory.new": lazy(() =>
    import("@/domain/memory/pages/CreateMemoryPage").then((module) => ({
      default: module.CreateMemoryPage,
    }))
  ),
  "mcpServers.index": lazy(() =>
    import("@/domain/mcp/pages/WorkspaceMcpServersPage").then((module) => ({
      default: module.WorkspaceMcpServersPage,
    }))
  ),
  "rag.index": lazy(() =>
    import("@/domain/rag/pages/RagPage").then((module) => ({
      default: module.RagPage,
    }))
  ),
  "resources.index": lazy(() =>
    import("@/domain/resource/pages/ResourcesPage").then((module) => ({
      default: module.ResourcesPage,
    }))
  ),
  "resources.detail": lazy(() =>
    import("@/domain/resource/pages/ResourceEditorPage").then((module) => ({
      default: module.ResourceEditorPage,
    }))
  ),
  "resources.new": lazy(() =>
    import("@/domain/resource/pages/ResourceEditorPage").then((module) => ({
      default: module.ResourceEditorPage,
    }))
  ),
  "resourceManagement.files": lazy(() =>
    import("@/domain/resource/pages/ResourcesPage").then((module) => ({
      default: module.ResourcesPage,
    }))
  ),
  "resourceManagement.filesDetail": lazy(() =>
    import("@/domain/resource/pages/ResourceEditorPage").then((module) => ({
      default: module.ResourceEditorPage,
    }))
  ),
  "settings.agents": lazy(() =>
    import("@/domain/agent/pages/PlatformAgentsPage").then((module) => ({
      default: module.PlatformAgentsPage,
    }))
  ),
  "settings.codeRepositories": lazy(() =>
    import("@/domain/settings/pages/CodeRepositoriesPage").then((module) => ({ default: module.CodeRepositoriesPage }))
  ),
  "settings.appearance": lazy(() =>
    import("@/domain/settings/pages/SettingsAppearancePage").then((module) => ({
      default: module.SettingsAppearancePage,
    }))
  ),
  "settings.capabilityPacks": lazy(() =>
    import("@/domain/settings/pages/CapabilityPacksPage").then((module) => ({
      default: module.CapabilityPacksPage,
    }))
  ),
  "settings.capabilityPackDetail": lazy(() =>
    import("@/domain/settings/pages/CapabilityPackDetailPage").then(
      (module) => ({ default: module.CapabilityPackDetailPage })
    )
  ),
  "settings.capabilityConfiguration": lazy(() =>
    import("@/domain/settings/pages/CapabilityConfigurationPage").then(
      (module) => ({ default: module.CapabilityConfigurationPage })
    )
  ),
  "settings.general": lazy(() =>
    import("@/domain/settings/pages/SettingsGeneralPage").then((module) => ({
      default: module.SettingsGeneralPage,
    }))
  ),
  "settings.shortcuts": lazy(() =>
    import("@/domain/settings/pages/SettingsShortcutsPage").then((module) => ({
      default: module.SettingsShortcutsPage,
    }))
  ),
  "settings.mcpServersEdit": lazy(() =>
    import("@/domain/mcp/pages/EditPlatformMcpServerPage").then((module) => ({
      default: module.EditPlatformMcpServerPage,
    }))
  ),
  "settings.mcpServersNew": lazy(() =>
    import("@/domain/mcp/pages/CreatePlatformMcpServerPage").then((module) => ({
      default: module.CreatePlatformMcpServerPage,
    }))
  ),
  "settings.mcpServersDetail": lazy(() =>
    import("@/domain/mcp/pages/PlatformMcpServerDetailsPage").then(
      (module) => ({ default: module.PlatformMcpServerDetailsPage })
    )
  ),
  "settings.mcpServers": lazy(() =>
    import("@/domain/mcp/pages/PlatformMcpServersPage").then((module) => ({
      default: module.PlatformMcpServersPage,
    }))
  ),
  "settings.taskFlows": lazy(() =>
    import("@/domain/task/pages/TaskFlowsPage").then((module) => ({
      default: module.TaskFlowsPage,
    }))
  ),
  "settings.taskFlowsNew": lazy(() =>
    import("@/domain/task/pages/TaskFlowEditorPage").then((module) => ({
      default: module.TaskFlowEditorPage,
    }))
  ),
  "settings.taskFlowsEdit": lazy(() =>
    import("@/domain/task/pages/TaskFlowEditorPage").then((module) => ({
      default: module.TaskFlowEditorPage,
    }))
  ),
  "settings.automaticRouters": lazy(() => import("@/domain/model/pages/AutomaticRoutersPage").then(module => ({ default: module.AutomaticRoutersPage }))),
  "settings.contextLayout": lazy(() => import("@/domain/model/pages/ContextLayoutPage").then(module => ({ default: module.ContextLayoutPage }))),
  "settings.modelProviders": lazy(() =>
    import("@/domain/model/pages/ModelProvidersPage").then((module) => ({
      default: module.ModelProvidersPage,
    }))
  ),
  "settings.modelProviderNew": lazy(() =>
    import("@/domain/model/pages/ConfiguredProviderPage").then((module) => ({
      default: module.CreateConfiguredProviderPage,
    }))
  ),
  "settings.modelProviderDetail": lazy(() =>
    import("@/domain/model/pages/ModelProviderDetailsPage").then((module) => ({
      default: module.ModelProviderDetailsPage,
    }))
  ),
  "settings.modelProviderCatalog": lazy(() =>
    import("@/domain/model/pages/ModelProviderCatalogPage").then((module) => ({
      default: module.ModelProviderCatalogPage,
    }))
  ),
  "settings.modelProviderSyncRuns": lazy(() =>
    import("@/domain/model/pages/ModelProviderSyncRunsPage").then((module) => ({
      default: module.ModelProviderSyncRunsPage,
    }))
  ),
  "settings.prompts": lazy(() =>
    import("@/domain/prompt/pages/PlatformPromptsPage").then((module) => ({
      default: module.PlatformPromptsPage,
    }))
  ),
  "settings.resources": lazy(() =>
    import("@/domain/resource/pages/ResourcesPage").then((module) => ({
      default: module.ResourcesPage,
    }))
  ),
  "settings.resourcesDetail": lazy(() =>
    import("@/domain/resource/pages/ResourceEditorPage").then((module) => ({
      default: module.ResourceEditorPage,
    }))
  ),
  "settings.resourcesNew": lazy(() =>
    import("@/domain/resource/pages/ResourceEditorPage").then((module) => ({
      default: module.ResourceEditorPage,
    }))
  ),
  "settings.promptsDetail": lazy(() =>
    import("@/domain/prompt/pages/PlatformPromptDetailsPage").then(
      (module) => ({ default: module.PlatformPromptDetailsPage })
    )
  ),
  "settings.promptsEdit": lazy(() =>
    import("@/domain/prompt/pages/PlatformPromptEditPage").then((module) => ({
      default: module.PlatformPromptEditPage,
    }))
  ),
  "settings.promptsNew": lazy(() =>
    import("@/domain/prompt/pages/CreatePlatformPromptPage").then((module) => ({
      default: module.CreatePlatformPromptPage,
    }))
  ),
  "settings.memoryDomains": lazy(() =>
    import("@/domain/memory/pages/PlatformMemoryDomainsPage").then(
      (module) => ({ default: module.PlatformMemoryDomainsPage })
    )
  ),
  "settings.variables": lazy(() =>
    import("@/domain/variable/pages/PlatformVariablesPage").then((module) => ({
      default: module.PlatformVariablesPage,
    }))
  ),
  "settings.variablesDetail": lazy(() =>
    import("@/domain/variable/pages/PlatformVariableDetailsPage").then(
      (module) => ({ default: module.PlatformVariableDetailsPage })
    )
  ),
  "settings.variablesEdit": lazy(() =>
    import("@/domain/variable/pages/EditPlatformVariablePage").then(
      (module) => ({ default: module.EditPlatformVariablePage })
    )
  ),
  "settings.skills": lazy(() =>
    import("@/domain/skill/pages/InstalledSkillsPage").then((module) => ({
      default: module.InstalledSkillsPage,
    }))
  ),
  "settings.skillDetail": lazy(() =>
    import("@/domain/skill/pages/SkillDetailPage").then((module) => ({
      default: module.SkillDetailPage,
    }))
  ),
  "settings.skillsSync": lazy(() =>
    import("@/domain/skill/pages/CreateSkillPage").then((module) => ({
      default: module.CreateSkillPage,
    }))
  ),
  "settings.tools": lazy(() =>
    import("@/domain/agent/pages/PlatformToolsPage").then((module) => ({
      default: module.PlatformToolsPage,
    }))
  ),
  "settings.toolsNativeSchemes": lazy(() =>
    import("@/domain/agent/pages/NativeToolSchemesPage").then((module) => ({
      default: module.NativeToolSchemesPage,
    }))
  ),
  "tasks.detail": lazy(() =>
    import("@/domain/task/pages/TaskDetailsPage").then((module) => ({
      default: module.TaskDetailsPage,
    }))
  ),
  "tasks.index": lazy(() =>
    import("@/domain/task/pages/TasksKanbanPage").then((module) => ({
      default: module.TasksKanbanPage,
    }))
  ),
  "tasks.new": lazy(() =>
    import("@/domain/task/pages/CreateTaskPage").then((module) => ({
      default: module.CreateTaskPage,
    }))
  ),
  "variables.details": lazy(() =>
    import("@/domain/variable/pages/VariableDetailsPage").then((module) => ({
      default: module.VariableDetailsPage,
    }))
  ),
  "variables.index": lazy(() =>
    import("@/domain/variable/pages/VariablesPage").then((module) => ({
      default: module.VariablesPage,
    }))
  ),
  "variables.new": lazy(() =>
    import("@/domain/variable/pages/CreateVariablePage").then((module) => ({
      default: module.CreateVariablePage,
    }))
  ),
};
