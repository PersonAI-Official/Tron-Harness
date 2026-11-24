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

## [1.1.0] - 2026-11-24

### Agents and chat

- Added plan mode and delivered-file display, and improved context budgets, memory usage, and run details.
- Added configurable native tool exposure schemes and improved tool selection, coordinated file edits, and command output viewing.
- Fixed repeated terminal commands after tool execution and missing context budgets in task and automation runs.
- Fixed context budget calculations for tasks and automations using conversation automatic routing.
- Renamed Dude to PersonAI and improved task management, session management, and assistant defaults.
- Improved file editing tools to prevent concurrent write conflicts.
- Fixed tasks and automations that could not run.

### Remote access and capability packs

- Added encrypted LAN and public remote access, including mobile chat, attachment uploads, and paired-device management.
- Added the PersonAI-Mobile companion app for mobile access: [PersonAI-Mobile](https://personai.chat/person-ai-index).
- Enhanced the capability center and desktop-control packs with platform-specific distribution and queued operations while agents are busy.
- Improved workspace deletion cleanup, the task board, and capability catalog loading speed.

### Localization and reliability

- Centralized translations for backend messages and improved knowledge retrieval notices and multilingual support.
- Improved remote connections, message streams, and interaction history recovery; fixed lost run errors in chat history.
- Improved startup after Windows updates and cleanup of long-path data.
- Completed missing localization strings.

### Automations and task execution

- Persisted task execution records and automation runs with linked sessions, and recover interrupted runs left in progress at startup.
- Added serial and parallel automation modes. Serial is the default; starting a new run stops the previous unfinished run.
- Automation details now open with the "Execution instances" tab in the right sidebar, showing run status, summary, and linked session.
- Hid the execution history and properties sidebar from the automation list; it remains available on automation details.
- Completed Chinese and English labels for automation modes, execution history, and workspace fields.
- Added permanent automation deletion, including its execution records.
- Localized the automation detail “Edit” button for Chinese.
- Moved automation metadata into the “Properties” tab in the right sidebar.
- Added a task history sidebar to task details, with access to execution sessions.

## [1.0.2] - 2026-11-11

### Added

- Release notes for the installed version appear in a centered dialog when you open the app.
- Select “Got it” to stop automatic reminders for that version. New versions show their own notes.
- Use “View release notes” in the version section of Settings to browse all versions, newest first.
- Release notes support Chinese and English and follow the interface language.

### Experience

- Release notes are included with the app and can be read offline.
- Close the dialog to read later. Unacknowledged notes appear again the next time you open the app.
- The dialog stays centered with a maximum height. Long notes scroll inside it while the title and acknowledgement button remain visible.

### Fixes and improvements

- Fixed HTTPS download and redirect validation for the WSL backend, with download timeout and cancellation handling.
- Fixed Windows update downloads, installer-helper startup and app restarts so failed downloads or helper errors do not close the app.
- Clarified manual installation for macOS and Linux updates, with access to the downloaded installer or its folder.
- Improved backend-environment troubleshooting so Settings can open a chat with diagnostic details.
- Made speed, cache and context statistics adapt to the chat composer's available width.
- Fixed authorization remaining in a waiting state after approval in the browser.
- Clarified model input capabilities and converted bundled agent avatars to WebP to reduce asset size.

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
