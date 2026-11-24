# Release notes

For each release, add `## [version] - YYYY-MM-DD` at the top of the entries.
The version must match `revision` in `version.properties`. Use level-three headings
and Markdown lists within each entry. Maintain Chinese content in `CHANGELOG.md`
and English content here. Both files must contain the same versions, including the current version.
The build bundles all released versions for offline reading, ordered newest first by version number.
Startup reminders show only the installed version; Settings shows the full history.
The dialog follows the interface language.

Historical versions are reconstructed from changes to `version.properties`.
Dates refer to those commits, not verified installer publication dates. Independently
versioned frontend packages are not product releases.
`0.1.0-SNAPSHOT` describes the development baseline when unified versioning was introduced;
subsequent development is summarized under `1.0.0`. The `1.0.2` entry also includes later
commits with that version number and the release-notes feature.

<!-- History sources: 0.1.0-SNAPSHOT through a5c7b167; 1.0.0: a5c7b167..a430ff1c;
1.0.1: a430ff1c..c59763ac; committed 1.0.2 changes: c59763ac..3dd760d3.
Use direct HTTPS image URLs in Markdown, e.g. ![Feature demo](https://example.com/demo.gif).
Remote images require a network connection and are not bundled with the text. -->

## **[1.1.0] - 2026-11-24**

### **Agents & Chat**

- Introduced Plan Mode and deliverable file previews, and improved the visualization of chat context budgets, memory usage, and short-term memory.
- Added configurable native toolsets for agents. Agents can now use different native tools depending on their use cases, rather than relying exclusively on the `tool_search` mechanism, improving execution efficiency.
- Optimized file editing tools to prevent concurrent write conflicts.
- Fixed an issue where the intelligent terminal repeatedly issued the same commands.
- Fixed issues preventing tasks and automations from running.
- Renamed "Good Buddy" to "Personal Assistant" and improved task and session management.

### **Remote Access & Capability Packs**

- Added encrypted remote access over both local networks and the internet, allowing users to participate in chats, upload attachments, and manage paired devices from mobile devices. Download [PersonAI-Mobile](https://personai.chat/person-ai-index).
- Launched the Capability Center, enabling users to upload and share their own capability packs.
- Added support for queuing capability pack operations while agents are busy.
- Improved workspace deletion and cleanup, task boards, and capability directory loading performance.
- Introduced a computer-use capability pack powered by CUA Driver, supporting Windows, macOS, and Linux.

### **Internationalization & Stability**

- Completed missing localization strings.
- Improved remote connectivity, message streaming, and interaction history recovery. Fixed an issue where chat execution errors were not preserved in conversation history.
- Optimized the Windows startup process and long-path data cleanup.

### **Automation & Task Execution**

- Added support for sequential and parallel automation execution modes, with sequential execution enabled by default.
- Added support for permanently deleting automations.
- Added an **Execution History** sidebar to automation details, allowing users to view execution statuses, summaries, and associated sessions.
- Added a **Task History** sidebar to task details, allowing users to review task execution sessions.

## [1.0.1] - 2026-11-11

### Workspaces and conversations

- Expanded archive management for conversations, topics and projects, including viewing and restoring archived containers and cleaning up related tabs.
- Added file references and automatic note saving to topics to reduce lost edits when switching pages.
- Bound new workspaces to a task flow so task boards follow the workspace configuration.
- Added upcoming-run previews to automation settings to help verify schedules.
- Simplified sidebar navigation and Settings layouts, added hold-to-show shortcut hints, and standardized Enter and Esc behavior in dialogs.

### Models and extensions

- Made context-window sizes required for configured models and provided editable presets.
- Loaded capability-pack pages in isolation and showed their navigation entries according to activation status.
- Reused app-managed storage connections across runtime reloads and improved storage shutdown and reclamation.

### Desktop experience

- Unified startup and profile-selection screens with theme synchronization and reduced-motion preferences.
- Installed only missing WSL dependencies, skipped APT requests when dependencies were already available, and clarified network failure messages.
- Added diagnostic chats for backend-environment failures, including the failed stage, distribution and error details.
- Improved standalone WSL backend builds and Linux installer configuration.

## [1.0.0] - 2026-11-10

### Agents and collaboration

- Integrated the Agent Framework runtime to unify agent sessions, tool execution and capability management.
- Added PersonAI onboarding and configuration, background wakeups, delegated tasks and result notifications, with guidance during execution.
- Gave subagents independent sessions with background execution, status inspection and human intervention.
- Added explicit skill and goal commands in chat, along with active-page and working-directory context for agents.
- Added tool permissions, command-risk explanations and batch approvals, with private password entry for elevated commands.

### Models, memory and knowledge

- Expanded Person-AI profile selection, account authorization and device recovery, with automatic model routing and custom providers.
- Unified context budgets, block ordering and percentage allocations, with views of system prompts, tool definitions and context details.
- Unified chat memory and long-term memory retrieval, simplified memory management, and improved session summaries and compaction.
- Expanded knowledge-base parsing, indexing and retrieval, including embedding-model bindings and reranking-model routing.
- Centralized capability-pack, skill and tool management and integrated capability-pack menus into navigation configuration.

### Chat and workspace

- Added A2UI interactions, attachments and voice input, and improved streaming replies, history replay, question continuations and turn deletion.
- Expanded cascading archival and permanent deletion for topics, projects and conversations while protecting linked project directories when deleting topics or folders.
- Added native directory selection, working-directory switching, and tabs and sidebars that retain their state.
- Added paired light and dark themes, importable skins, background and motion settings, and improved Settings layouts and shortcuts.

### Desktop and runtime visibility

- Added online updates, plus tray residency and startup settings on Windows and Linux.
- Supported Windows local and WSL backends and improved macOS, Windows and Linux builds and installers.
- Unified desktop window controls, draggable toolbars and sidebar layouts, and fixed startup-overlay stalls.
- Added persistent traces, execution logs, process-memory and disk-usage views, with clearer failure reasons and call details.
- Added incremental SQLite space reclamation and optimized database reads, conversation-history loading and realtime event connections.

## [0.1.0-SNAPSHOT] - 2026-09-16

### Development baseline

- Introduced unified product versioning across the frontend, backend, desktop shell and API, and adjusted initial setup defaults.
- Provided foundational interfaces for workspaces, applications, chat, prompts, models, resources and navigation management.
- Integrated the Person-AI model provider, with conversation attachments, image previews and interactions that request user input.
- Added file-operation panels and external-editor entry points, with support for multiple file panels.
- Improved smart-terminal, SSH and nested-shell interactions and fixed Chinese input-method and command-history issues.
- Added font and shortcut settings, conversation-draft persistence and saved right-panel state.
