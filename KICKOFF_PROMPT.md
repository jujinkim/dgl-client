# dgl client — Master Development Prompt

## 0. Your Role and Mission

You are the principal software architect and senior cross-platform application engineer responsible for designing and implementing **dgl client**.

You are expected to make sound architectural decisions, investigate technical uncertainties, implement production-quality software, write tests, and maintain clear project documentation.

Use existing, mature technologies whenever possible. Avoid unnecessary abstractions, infrastructure, complexity, and reinvention.

Prioritize correctness, reliability, security, maintainability, and usability over feature count.

The ultimate goal is a lightweight, robust, cross-platform SSH and terminal client optimized for tmux and CLI-based AI coding agents.

**Core philosophy: Keep the CLI. Simplify the workflow.**

---

# 1. Project Identity

**Project Name:** dgl client  
**Full Name:** Ddal-Ggak Life Client  
**Package / Bundle Identifier:** `kim.jujin.rovessh`

**Category:** Cross-platform SSH & Terminal Client for AI Coding Agents

**Primary Target Platforms:**
- Windows
- Android

**Additional Target Platforms:**
- macOS (Apple Silicon)
- Linux (GUI)
- iOS
- iPadOS

Windows and Android are the primary development and testing targets.

However, architecture and technology choices should preserve the feasibility of supporting all listed platforms.

Do not sacrifice the quality of the primary platforms merely to claim premature cross-platform support.

---

# 2. Product Vision

dgl client is an SSH and terminal client designed specifically for developers who use tmux and CLI-based AI coding agents such as Claude Code and OpenAI Codex.

Users should be able to:

- Connect to remote SSH servers.
- Browse and manage existing tmux sessions visually.
- Create project-specific workspaces.
- Launch and interact with AI coding agents.
- Switch between agents, sessions, windows, and panes.
- Disconnect without terminating their running processes.
- Reconnect automatically after network interruptions.
- Resume the same working environment from another device.
- Continue using standard SSH and tmux clients at any time.

The application is not intended to replace existing CLI interfaces.

Claude Code, Codex, and other CLI applications already provide capable interactive terminal interfaces.

dgl client should render those interfaces faithfully rather than replacing them with proprietary chat interfaces.

The GUI should focus on connection management, tmux management, navigation, input convenience, and usability.

## 2.1 Motivation

Traditional SSH clients provide basic terminal access but are inconvenient for intensive tmux usage.

Common issues include:

1. Users must manually enter tmux commands to create, discover, attach to, and manage sessions.
2. Mobile network changes frequently disconnect SSH connections.
3. After reconnecting, users must manually locate and reattach to their tmux sessions.
4. Window and pane management requires memorizing keyboard shortcuts.
5. Managing multiple projects and workspaces is cumbersome.
6. Mobile keyboards are poorly suited to terminal shortcuts.
7. AI coding agents may run for extended periods and should remain accessible independently of the client.

Some existing terminal wrappers and AI coding assistants attempt to solve these issues using proprietary session systems and extensive custom interfaces.

These solutions can be useful, but they may introduce additional complexity, unnecessary dependencies, and new failure modes.

dgl client takes a different approach.

**Use the existing tmux server as the session-management foundation, and build a convenient graphical client around it.**

---

# 3. Fundamental Architectural Principles

These principles are mandatory.

## 3.1 tmux Is the Single Source of Truth

tmux must be the authoritative source for:

- Sessions
- Windows
- Panes
- Layouts
- Active selections
- Attachment information
- Session-related metadata
- Relevant tmux operating modes

Do not maintain a separate authoritative session database.

Do not implement a proprietary session manager that duplicates tmux functionality.

Client-side caches are permitted for performance and UI rendering, but they must be reconcilable with the actual tmux server state.

Local application preferences, SSH host profiles, credentials, themes, and shortcut settings are exceptions: these belong to the application rather than tmux.

The client may also persist the last-selected host/session/window/pane as a navigation preference, but this must never be treated as authoritative remote state.

## 3.2 Preserve Standard tmux Compatibility

Sessions created by dgl client must remain accessible through ordinary SSH terminals.

Sessions created through ordinary tmux commands must be accessible through dgl client.

Examples:

```bash
tmux new-session -s example
tmux attach -t example
tmux list-sessions
```

These commands must remain valid independently of dgl client.

Do not require proprietary session formats.

Do not modify tmux in ways that break standard usage.

Do not require users to launch all sessions through dgl client.

## 3.3 No Required Remote Daemon

A standard remote installation should require only:

- SSH server
- tmux
- The user's existing shell and CLI applications

No dedicated dgl server process should be mandatory.

Do not require a continuously running custom Node.js, Python, or other remote service for core functionality.

Optional future extensions may be considered, but they must not become prerequisites for the core SSH/tmux experience.

## 3.4 Client and Process Lifecycle Independence

SSH connection lifetime and tmux session lifetime must remain separate.

Closing dgl client must not terminate the user's tmux sessions.

Losing the network connection must not terminate running AI agents.

A normal application disconnect must not execute destructive tmux commands.

Explicitly distinguish:

- Disconnect SSH
- Detach from tmux
- Terminate pane
- Terminate window
- Kill session

Destructive actions require deliberate user interaction.

## 3.5 CLI Native, Not an AI Chat Wrapper

Do not build a replacement chat interface for Claude Code or Codex.

Do not reproduce agent-specific terminal interfaces as custom GUI components.

Render the terminal output faithfully.

GUI elements should supplement terminal interaction rather than replace the CLI.

## 3.6 Minimalism

Avoid feature bloat.

Do not add functionality merely because it is technically possible.

Every feature should contribute meaningfully to at least one of:

- SSH usability
- tmux management
- AI agent interaction
- Reliability
- Security
- Cross-device continuity

---

# 4. Platform Requirements

| Platform | Remote SSH | Local Terminal / tmux | Priority |
|---|---|---|---|
| Windows | Required | WSL integration | P0 |
| Android | Required | Not required | P0 |
| macOS Apple Silicon | Required | Native local tmux | P1 |
| Linux Desktop | Required | Native local tmux | P1 |
| iOS | Required | Not required | P1 |
| iPadOS | Required | Not required | P1 |

## 4.1 Windows

Support remote SSH connections and tmux management.

Additionally, investigate support for locally installed tmux through WSL.

Windows should not be assumed to have native tmux installed.

Detect available WSL environments and provide a reasonable configuration workflow.

## 4.2 Android

Remote SSH is mandatory.

Local terminal execution and local tmux are not required.

Pay particular attention to:

- Software keyboard interaction
- Touch input
- Network changes
- Background/foreground transitions
- Display resizing
- Screen orientation changes
- Foldable displays
- Long-running remote sessions

## 4.3 macOS and Linux

Support remote SSH and local terminal sessions.

Where tmux is installed locally, support the same session management capabilities without an SSH connection.

## 4.4 iOS and iPadOS

Remote SSH only.

Support touch-optimized terminal input and session management.

Respect the platform's application lifecycle and security limitations.

Do not assume unrestricted background execution.

---

# 5. Recommended Technology Stack

Prefer:

**Kotlin Multiplatform + Compose Multiplatform**

The project should maximize shared business logic and UI while preserving native integrations where appropriate.

However, architecture must be validated before selecting every dependency.

## 5.1 Logical Architecture

Organize the application into distinct logical layers.

### Presentation Layer
- Connection UI
- Host Manager UI
- Workspace Explorer
- Session/Window/Pane Navigator
- Terminal View
- Shortcut Toolbar
- Settings
- Theme System

### Application Layer
- Connection Lifecycle Manager
- Host Profile Manager
- Workspace Manager
- tmux Session Controller
- Agent Launcher
- Input Controller
- Shortcut Manager
- Reconnection Coordinator

### Protocol and Terminal Layer
- SSH Transport
- SSH Authentication
- tmux Control Mode Protocol
- tmux Event Parser
- tmux State Repository
- Terminal Emulator Adapter
- Terminal Input Encoder

### Platform Layer
- Secure Credential Storage
- Keyboard / IME Integration
- Clipboard
- Local Process Execution
- WSL Integration
- Network Lifecycle
- OS-Specific File Access

These are logical responsibilities, not a requirement to create excessive modules or interfaces.

Use the simplest maintainable architecture that satisfies actual implementation needs.

## 5.2 SSH Implementation

Research mature SSH client libraries compatible with the target platforms.

Evaluate:

- Platform coverage
- Maintenance status
- Licensing
- Modern SSH algorithms
- Key formats
- Host-key verification
- Authentication capabilities
- Connection stability
- Integration complexity

Do not implement SSH cryptography manually.

Platform-specific adapters are acceptable where a universal SSH library is insufficient.

## 5.3 Terminal Emulator

Research suitable terminal emulation solutions.

Candidates may include:

- xterm.js inside a WebView
- Native terminal rendering engines
- Existing embeddable terminal emulators

Evaluate:

- Cross-platform consistency
- Performance
- UTF-8 and Unicode handling
- ANSI/VT compatibility
- Alternate-screen support
- Terminal resizing
- Clipboard interaction
- IME input
- Mouse reporting
- Mobile responsiveness
- Memory usage

Do not implement a new terminal escape-sequence parser or renderer unless existing solutions are demonstrably inadequate.

Do not finalize the rendering architecture before validating the tmux Control Mode integration.

---

# 6. SSH Client Requirements

## 6.1 Host Management

Implement:

- Hostname / IP address
- Port
- Username
- Authentication method
- Host alias
- Create/edit/delete host profiles
- Recent connections
- Favorites
- Connection status
- Multiple SSH hosts
- Multiple connections where appropriate
- Standard SSH shell access without tmux

A user must be able to use dgl client as a conventional SSH client even when tmux is unavailable.

## 6.2 Authentication

Support:

- Password authentication
- SSH public-key authentication
- Importing existing private keys
- Generating new SSH keys
- Encrypted private keys and passphrases
- Host public-key verification

Prefer modern and widely supported key types, including Ed25519, while considering compatibility with existing RSA keys.

## 6.3 Security

Use platform security facilities wherever appropriate, including:

- Android Keystore
- Apple Keychain
- Windows DPAPI or suitable credential protection
- Appropriate Linux secure credential storage

Protect private keys at rest.

Do not store sensitive credentials in plaintext configuration files.

Do not log credentials, passphrases, private keys, or sensitive authentication payloads.

Implement strict SSH host-key validation with an explicit first-use trust workflow and appropriate key-change warnings.

Never silently disable host-key verification.

Allow users to inspect and manage trusted host keys.

Use standard SSH transport encryption.

Do not invent a separate encryption layer unnecessarily.

SSH encryption protects communication between the SSH client and SSH server; do not misrepresent it as encryption that excludes the remote host from access.

## 6.4 Additional SSH Features

Consider after core functionality is stable:

- OpenSSH config support
- SSH agent integration
- ProxyJump / Bastion Host
- Port forwarding
- SFTP
- Keepalive configuration
- Connection timeouts
- Advanced SSH options

These are secondary to reliable core SSH/tmux operation.

---

# 7. tmux Control Mode Integration

This is the most important technical component.

Use tmux Control Mode as the primary integration mechanism where practical.

Reference:

https://github.com/tmux/tmux/wiki/Control-Mode

Investigate the exact protocol, version compatibility, and runtime behavior before implementing a production parser.

A typical invocation is:

```bash
tmux -C
```

The actual startup, attachment, and control-channel lifecycle should be determined through technical validation.

Do not assume that `-C` and `-CC` are interchangeable in every integration context.

## 7.1 Control Mode Responsibilities

The implementation should support:

- Listing sessions
- Creating sessions
- Attaching to existing sessions
- Detaching safely
- Renaming sessions
- Killing sessions
- Listing windows
- Creating windows
- Renaming windows
- Selecting windows
- Closing windows
- Listing panes
- Selecting panes
- Splitting panes
- Resizing panes
- Closing panes
- Receiving pane output
- Sending terminal input
- Receiving and interpreting tmux state-change events

Use tmux commands and Control Mode notifications as appropriate.

Avoid using repeated `tmux list-*` shell executions as the primary mechanism for all real-time state updates if Control Mode events provide a more reliable method.

## 7.2 Control Mode Protocol Parser

Implement a proper protocol parser.

Handle:

- Command response boundaries
- `%begin`, `%end`, and `%error`
- Asynchronous notifications
- `%output`
- Escaping and encoding rules
- Partial SSH reads
- Multiple messages in a single read
- Output from multiple panes
- Protocol errors
- Connection interruption
- Version-specific differences

Do not parse Control Mode output using naive line splitting without a correct framing and escaping strategy.

The parser must distinguish command responses from unsolicited events.

Do not assume output arrives in the same structure as a conventional interactive SSH shell.

## 7.3 Terminal Rendering

Control Mode output is not a fully rendered terminal screen.

The application is responsible for feeding the correct output into a terminal emulator.

Implement:

- Per-pane terminal state where necessary
- ANSI/VT processing through the selected emulator
- Output routing by pane identifier
- Correct handling of pane switching
- Terminal resizing
- Alternate-screen applications
- Interactive TUI support

Claude Code and Codex must render and behave correctly.

## 7.4 tmux State Inspection

Investigate and use tmux format variables and layout information.

Relevant examples include:

```text
pane_id
pane_in_mode
pane_mode
pane_current_command
pane_width
pane_height
pane_left
pane_top
window_layout
session_attached
client_prefix
client_key_table
```

Actual availability must be checked against the selected tmux version.

Expose useful information through the GUI:

- Current session
- Current window
- Current pane
- Pane layout
- Pane sizes
- Attached clients
- Running command
- tmux mode
- Copy Mode state where observable

Not all state variables are necessarily available or meaningful in all contexts.

Avoid inventing state when tmux cannot provide it.

## 7.5 Visual tmux Management

Provide GUI controls to manage tmux without requiring keyboard shortcuts.

Examples:

- Session selection
- Window selection
- Pane selection
- Horizontal split
- Vertical split
- Pane resize
- Close pane
- Close window
- Rename session/window
- Create workspace

Where practical, visually represent the tmux layout.

The UI must reflect actual tmux state, including modifications made externally by other SSH clients.

## 7.6 Copy Mode

Study the limitations of tmux Control Mode regarding Copy Mode.

Copy Mode output may not behave like normal pane output.

Investigate whether the application should:

1. Control native tmux Copy Mode.
2. Provide terminal-local scrolling and selection.
3. Use `capture-pane` for history retrieval.
4. Combine these approaches.

Do not assume Copy Mode is automatically fully represented by `%output`.

## 7.7 Multiple Clients

Allow simultaneous connections to the same tmux session.

Consider:

- Concurrent keyboard input
- Active pane selection
- Different terminal dimensions
- tmux window sizing policies
- Output synchronization
- Per-client state

Do not unnecessarily detach other clients when attaching dgl client.

---

# 8. Projects, Workspaces, and Session Naming

dgl client should organize tmux sessions by project and workspace.

## 8.1 Default Naming

Use:

```text
dgl-<project-name>-<workspace-name>
```

Examples:

```text
dgl-grayin-main
dgl-grayin-feature-auth
dgl-miniearthure-dev
```

Users may override session names.

Existing non-dgl tmux sessions must remain accessible.

## 8.2 Metadata

Investigate tmux User Options such as:

```text
@dgl_project
@dgl_workspace
```

These may provide explicit workspace metadata without a separate database.

If implemented:

- Store metadata in tmux.
- Keep it optional.
- Do not modify unrelated sessions unnecessarily.
- Preserve compatibility with ordinary tmux clients.
- Handle missing metadata.
- Handle naming collisions.
- Handle unusual project/workspace names safely.

The GUI may maintain temporary derived groupings, but tmux remains authoritative for session state.

## 8.3 Workspace Explorer

Provide a straightforward hierarchy:

```text
SSH Host
  Project
    Workspace / Session
      Window
        Pane
```

Users should be able to navigate this structure without memorizing tmux commands.

---

# 9. Connection Lifecycle and Recovery

Reliable connection recovery is one of the highest-priority features.

## 9.1 Supported Scenarios

Handle:

- Wi-Fi to cellular transition
- Cellular to Wi-Fi transition
- Temporary network loss
- SSH timeout
- App background/foreground transitions
- Device screen locking
- Application restart
- SSH transport failure

Respect mobile OS restrictions.

Do not promise continuous background SSH connectivity when the operating system cannot guarantee it.

Instead, optimize for reliable restoration when the application becomes active.

## 9.2 Recovery Procedure

On reconnection:

1. Establish a new SSH transport.
2. Verify the SSH host identity.
3. Reconnect to the existing tmux server.
4. Query current tmux sessions.
5. Find the previously selected session.
6. Find the previous window and pane where possible.
7. Restore terminal display state.
8. Resume live output.
9. Re-enable input.
10. Update the GUI to reflect the actual state.

If the original session no longer exists, report that clearly.

Do not silently create a replacement and pretend it is the previous session.

## 9.3 Screen Restoration

Investigate tmux `capture-pane` as part of the screen restoration strategy.

However, carefully handle race conditions between:

- Snapshot capture
- New live output
- Window resizing
- Pane switching
- Reconnection

Avoid:

- Missing output
- Duplicating output
- Replaying output in the wrong order
- Rendering stale snapshots after fresh data
- Applying a snapshot to the wrong pane

Develop and test a clearly defined synchronization strategy.

Do not assume that a snapshot combined with live output automatically reconstructs an identical terminal state.

Document unavoidable limitations.

## 9.4 Safe Disconnection

Closing the client should leave tmux sessions alive.

Normal disconnection must not send `kill-session`, terminate running processes, or send accidental control characters to the agent.

Provide a separate explicit action for destructive operations.

## 9.5 Recovery Boundaries

tmux preserves sessions across client disconnections, not across arbitrary server failures.

A remote host reboot, tmux server termination, or agent process crash may destroy the running process.

Do not claim that client reconnection can restore processes that no longer exist.

---

# 10. Terminal User Experience

## 10.1 Terminal Compatibility

Support:

- UTF-8
- Korean and Japanese text
- Unicode wide characters
- ANSI colors and formatting
- VT control sequences
- Alternate screen
- Cursor control
- Terminal resizing
- Scrollback
- Selection and clipboard
- Keyboard input
- Mouse reporting where applicable
- Full-screen terminal applications

Interactive CLI programs must work correctly.

## 10.2 Input Modes

Implement two modes.

### Mode A — Live Input

Keyboard input is transmitted directly to the terminal.

This should behave like a normal terminal emulator.

Support interactive applications, shortcuts, modifier keys, and navigation.

### Mode B — Textbox Input

Provide a separate editable text field.

The user composes text locally and presses Send to transmit it.

Support:

- Editing before sending
- Multiline input
- Clipboard paste
- Unicode and IME composition
- Long prompts
- Explicit Send action

Carefully distinguish sending text from submitting it with Enter.

The application should not accidentally execute incomplete commands or submit multiline text unexpectedly.

For bulk text insertion, investigate safe terminal paste semantics, including bracketed paste support where appropriate.

Do not construct tmux commands through unsafe string concatenation of arbitrary user input.

## 10.3 Shortcut Toolbar

Provide a configurable shortcut toolbar.

Default shortcuts may include:

```text
Esc
Tab
Ctrl
Alt
↑
↓
←
→
Home
End
Ctrl+C
Ctrl+D
Ctrl+L
tmux Prefix
Next Window
Previous Window
Split Horizontal
Split Vertical
Copy Mode
```

Users must be able to:

- Add shortcuts
- Remove shortcuts
- Reorder shortcuts
- Customize key combinations
- Customize multi-key sequences
- Configure tmux actions

Distinguish direct terminal key input from tmux management commands.

Prefer invoking tmux controls directly for GUI-driven operations rather than emulating `Ctrl+B` whenever possible.

## 10.4 Mobile Layout

The shortcut toolbar must remain at the bottom of the terminal area.

When the software keyboard appears, position the toolbar immediately above it.

Requirements:

- Default one-row layout
- Horizontal scrolling
- Optional two-row layout
- Configurable ordering
- Touch-friendly buttons
- Minimal obstruction of the terminal

Ensure correct behavior during keyboard appearance, disappearance, and display resizing.

## 10.5 Desktop Input

Support normal hardware keyboards and standard terminal keyboard shortcuts.

Do not intercept combinations unnecessarily.

Preserve system-level keyboard functionality where possible.

---

# 11. AI Coding Agent Support

## 11.1 Initial Agents

Support:

- Claude Code
- OpenAI Codex CLI
- Standard shell sessions

Do not make the application dependent on either specific AI agent.

## 11.2 Agent Launching

Provide basic GUI actions to:

- Open a shell
- Launch Claude Code
- Launch Codex
- Attach to an existing agent session
- Switch between running agent panes
- Return to a shell where appropriate

Do not require special modifications to the agents.

Do not inject proprietary agent wrappers unless a clearly justified optional integration requires them.

## 11.3 Agent Status Detection

Investigate optional state detection.

Potential states:

- Running
- Idle
- Waiting for Input
- Permission Required
- Exited
- Unknown

tmux alone does not expose the full internal state of Claude Code or Codex.

Do not infer precise agent states merely from the existence of a tmux pane.

Investigate agent-supported hooks, structured signals, and process information where appropriate.

Avoid fragile terminal-output scraping as a mandatory foundational dependency.

If precise detection is unavailable, display `Unknown`.

Status detection is secondary to reliable terminal operation.

## 11.4 Process Reliability

The application must not terminate an agent simply because:

- The client is closed.
- SSH disconnects.
- A mobile network changes.
- The user switches devices.
- The UI stops observing a pane.

Agent process ownership belongs to the remote shell/tmux environment, not the client application.

---

# 12. Application UI and Design

## 12.1 Visual Direction

Design language:

**Modern, Flat, Minimal, Readable**

Requirements:

- Light theme
- Dark theme
- System theme following
- Clear information hierarchy
- Readability as the highest priority
- Appropriate spacing
- Simple iconography
- Minimal unnecessary animation
- Efficient use of screen space
- Consistent controls across platforms

Avoid overly decorative dashboards.

The terminal should remain the primary workspace.

## 12.2 Main Screens

### Connection Manager

- SSH hosts
- Aliases
- Favorites
- Recent connections
- Connection status
- Add/edit/remove profiles

### Workspace Explorer

- Host
- Project
- Workspace/session
- Windows
- Panes
- Session actions
- Window and pane actions

### Terminal

- Terminal rendering surface
- Current session context
- Session/window/pane navigation
- Shortcut toolbar
- Input mode switch
- Connection state indicator
- Reconnect controls where needed

### Settings

- Authentication and security
- SSH options
- Connection behavior
- Shortcuts
- Terminal appearance
- Themes
- Reconnection behavior

## 12.3 Terminal Appearance

Support terminal settings separately from the application UI theme.

Consider:

- Font family
- Font size
- Line spacing
- Cursor appearance
- Color palette
- Dark/light terminal themes

Use reasonable defaults.

---

# 13. Performance and Reliability

Treat dgl client as a long-running developer utility.

Requirements:

- Efficient SSH stream processing
- Incremental tmux event handling
- Responsive terminal rendering
- Appropriate buffering and backpressure
- Bounded memory usage
- Correct lifecycle cleanup
- No unnecessary high-frequency polling
- Safe handling of large output volumes
- Stable behavior across repeated reconnects
- Graceful recovery after protocol failures

Do not block the UI thread with SSH communication, output parsing, or terminal processing.

Avoid uncontrolled logging of terminal output, which may contain confidential source code or credentials.

---

# 14. Testing Strategy

Testing must cover protocol correctness, user interaction, security, and reconnection behavior.

## 14.1 Unit Tests

Cover:

- Control Mode protocol parsing
- Event routing
- Escaped output handling
- Session/window/pane state reconciliation
- Workspace naming
- Metadata encoding
- Shortcut processing
- Connection state transitions
- Input encoding
- Reconnection state machines

## 14.2 Integration Tests

Use real SSH and tmux environments where practical.

Verify:

- Creating sessions
- Attaching to sessions
- Switching windows
- Splitting panes
- Resizing panes
- Receiving output
- Sending input
- Detaching
- Reattaching
- Multiple clients
- External tmux modifications
- Client crashes
- Network disconnections

Test with supported tmux versions.

## 14.3 Security Tests

Include:

- Host-key mismatch behavior
- Encrypted key handling
- Incorrect password/passphrase
- Credential storage inspection
- Input escaping
- Session-name injection attempts
- Malformed protocol messages
- Sensitive-log exposure

## 14.4 UI Tests

Verify:

- Windows keyboard interaction
- Android software keyboard behavior
- Shortcut toolbar position
- Terminal resizing
- Orientation changes
- Foldable display resizing
- Theme switching
- Session navigation
- Connection recovery UX

## 14.5 Agent Compatibility

Manually or automatically validate actual Claude Code and Codex CLI behavior.

Pay attention to:

- Terminal rendering
- Interactive prompts
- Permission requests
- Keyboard navigation
- Full-screen updates
- Long-running commands
- Large terminal output
- Reconnection during active execution

---

# 15. Development Roadmap

Develop incrementally.

Do not attempt to implement all platforms and features simultaneously.

## Stage 0 — Technical Feasibility / Proof of Concept

Before designing the full application, validate the critical integration path.

**Target:**

```text
SSH
  ↓
tmux Control Mode
  ↓
Protocol Parser
  ↓
Terminal Emulator
  ↕
Keyboard Input
```

Implement a minimal working prototype capable of:

1. Connecting to a remote SSH host.
2. Starting or attaching to a tmux Control Mode client.
3. Listing existing tmux sessions.
4. Selecting a session.
5. Receiving real pane output.
6. Displaying output in a working terminal.
7. Sending interactive input.
8. Disconnecting without terminating tmux.
9. Reconnecting.
10. Restoring the terminal display as accurately as possible.

Validate interactive TUI applications early.

This stage is the primary architectural risk-reduction milestone.

If a fundamental limitation is discovered, document it and adjust the architecture before continuing.

## Stage 1 — Core MVP (P0)

Primary platforms: Windows and Android.

Implement:

- SSH host management
- Password authentication
- Key authentication
- Secure credential storage
- Host-key verification
- Reliable SSH transport
- tmux Control Mode integration
- Session list
- Session creation/attachment
- Terminal rendering
- Live terminal input
- Basic shortcut toolbar
- Connection recovery
- Session preservation
- Light/dark themes
- Basic Claude Code/Codex CLI compatibility

Deliver an actually usable application, not merely a collection of isolated components.

## Stage 2 — Advanced tmux UX (P1)

Implement:

- Project/workspace management
- Session naming conventions
- tmux metadata support
- Window/pane GUI
- Visual layout management
- Customizable shortcuts
- Textbox input
- Agent launch actions
- Optional agent status detection
- Improved terminal customization

Extend supported platforms:

- macOS
- Linux
- iOS
- iPadOS

## Stage 3 — Advanced SSH and Polish (P2)

Consider:

- OpenSSH config support
- ProxyJump
- SSH Agent
- Port forwarding
- SFTP
- Expanded terminal configuration
- Accessibility improvements
- Advanced multi-client behavior
- Performance profiling and optimization
- Production release hardening

---

# 16. Acceptance Criteria

The initial implementation should satisfy the following end-to-end scenarios.

### Scenario A — Windows to Android Continuity

1. Open dgl client on Windows.
2. Connect to a remote SSH server.
3. Create a tmux session.
4. Start Codex in that session.
5. Close dgl client.
6. Open dgl client on Android.
7. Connect to the same SSH server.
8. Locate the existing session in the GUI.
9. Attach to it.
10. Continue the running Codex interaction.

The original Codex process must not be restarted unnecessarily.

### Scenario B — Mobile Network Change

1. Connect from Android.
2. Attach to an active tmux session.
3. Switch between Wi-Fi and cellular data.
4. Allow the SSH connection to break.
5. Recover the connection.
6. Reattach to the original session.
7. Restore the terminal display.
8. Resume normal interaction.

The underlying tmux session must survive the client disconnection.

### Scenario C — Standard SSH Compatibility

1. Create a session through dgl client.
2. Disconnect dgl client.
3. Connect using a conventional SSH terminal.
4. Run standard tmux commands.
5. Attach to the same session.
6. Confirm the original processes are available.

Then repeat in reverse:

1. Create a tmux session using a conventional terminal.
2. Connect through dgl client.
3. Discover the existing session.
4. Manage it through the GUI.

### Scenario D — GUI Session Management

The user should be able to:

- Create sessions
- Rename sessions
- Select windows
- Split panes
- Switch panes
- Resize panes
- Detach
- Reattach

without manually entering tmux commands.

### Scenario E — Process Safety

Closing, disconnecting, or crashing the client must not inadvertently kill the tmux session or agent process.

A destructive action must require an explicit user request.

### Scenario F — Terminal Compatibility

Claude Code and Codex must remain usable through their existing interactive CLI interfaces.

Keyboard input, screen updates, cursor positioning, terminal colors, and permission prompts must function correctly.

---

# 17. Non-Goals

Do not implement the following unless explicitly requested later:

- A custom AI coding agent
- A replacement Codex/Claude chat interface
- A proprietary terminal multiplexer
- A separate authoritative session database
- Mandatory remote daemons
- Cloud-hosted session management
- Account registration requirements
- Proprietary AI inference services
- A full IDE
- Source code editing features unrelated to terminal use
- Complex multi-agent orchestration
- Workflow automation unrelated to SSH/tmux
- Excessive analytics and dashboards

Do not expand the product scope without a clear user benefit.

---

# 18. Engineering Process and Deliverables

Follow these instructions throughout development.

## 18.1 Inspect Before Implementing

If an existing repository is provided:

- Inspect its structure.
- Understand existing implementations.
- Preserve working components.
- Avoid unnecessary rewrites.

If the repository is empty, initialize it with a maintainable project structure.

## 18.2 Research Uncertain Technical Details

Before choosing key dependencies, validate their current capabilities, maintenance status, licenses, and platform support using authoritative documentation.

Do not rely on assumptions about tmux internals or cross-platform library compatibility.

When uncertain, create a small executable experiment.

## 18.3 Avoid Premature Architecture

Do not build elaborate abstractions before confirming that the underlying SSH, tmux, and terminal integration works.

Start with a working vertical slice.

Abstract repeated or platform-specific behavior when actual requirements justify doing so.

## 18.4 Deliver Working Code

Do not stop after generating an architecture document or a collection of interfaces.

The project should progress toward executable, tested software.

Maintain buildable checkpoints wherever practical.

Run available builds, tests, and static analysis after substantial changes.

Do not claim a platform is supported until its application has been built and tested.

## 18.5 Maintain Documentation

Maintain concise project documentation covering:

- Architecture
- Dependencies
- Platform support
- Build instructions
- SSH setup
- tmux compatibility
- Security model
- Known limitations
- Testing procedures
- Implementation progress

Clearly distinguish implemented features, planned features, and experimental functionality.

## 18.6 Decision-Making

Make reasonable implementation decisions independently.

Prefer simple, maintainable, well-supported solutions.

When alternatives exist, evaluate technical trade-offs rather than choosing based on popularity alone.

Do not introduce unnecessary third-party services.

Preserve the project's core design principles.

---

# 19. Initial Execution Instructions

Begin with the following sequence.

### Step 1 — Technical Assessment

Evaluate:

1. Kotlin Multiplatform and Compose Multiplatform feasibility for all target platforms.
2. Suitable SSH libraries and required platform adapters.
3. Suitable terminal emulators and rendering approaches.
4. tmux Control Mode protocol capabilities and limitations.
5. Mobile keyboard and connection-lifecycle requirements.
6. Windows WSL integration options.
7. Security-sensitive components and their platform implementations.

Identify major risks and unresolved questions.

### Step 2 — Architecture Proposal

Produce a concise technical architecture describing:

- Main components
- Responsibility boundaries
- Data flow
- SSH connection lifecycle
- tmux Control Mode lifecycle
- Terminal rendering pipeline
- Input pipeline
- Reconnection strategy
- Credential security
- Cross-platform implementation strategy

Separate confirmed facts from assumptions.

### Step 3 — Critical Proof of Concept

Implement the smallest functional application or experimental code needed to validate:

**SSH → tmux Control Mode → terminal rendering → input → disconnection → reconnection.**

This is the first implementation priority.

Do not spend time implementing advanced UI or optional agent integrations before the fundamental pipeline is validated.

### Step 4 — MVP Implementation

After validating the architecture, implement the Windows and Android MVP.

Keep the application usable throughout development.

### Step 5 — Verification

Test the scenarios described in Section 16.

Identify defects, limitations, and incomplete features.

Do not report success for functionality that has not actually been exercised.

### Step 6 — Report Results

At each meaningful milestone, report:

- What was implemented
- What was tested
- What works
- What remains incomplete
- Any significant technical decisions
- Any architectural risks or blockers
- Recommended next implementation steps

Use concise technical reporting.

---

# 20. Final Architectural Constraints

The following requirements override implementation convenience:

1. **tmux remains the authoritative session manager.**
2. **Standard SSH/tmux interoperability is mandatory.**
3. **No mandatory proprietary remote daemon.**
4. **No replacement GUI for existing AI agent CLIs.**
5. **Client disconnection must not kill running agent processes.**
6. **Security must not be weakened to simplify development.**
7. **Windows and Android take priority.**
8. **Architecture should permit later expansion to the remaining target platforms.**
9. **Reliability and simplicity take precedence over feature count.**
10. **Technical assumptions must be validated through working code and tests.**

---

# 21. Product Definition

**dgl client — Ddal-Ggak Life Client**

A cross-platform SSH and terminal client optimized for tmux and AI coding agents.

It enables developers to manage persistent terminal sessions, interact with existing coding agents, and continue work seamlessly across devices without replacing their established CLI workflows.

It uses standard SSH and tmux, avoids proprietary session infrastructure, and prioritizes reliability, security, clarity, and minimalism.

**Keep the CLI. Simplify the workflow.**

Begin with the technical assessment and the critical proof of concept. Continue toward a working Windows/Android MVP using the staged development plan above.

