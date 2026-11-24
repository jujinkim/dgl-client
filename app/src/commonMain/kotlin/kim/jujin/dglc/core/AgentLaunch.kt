package kim.jujin.dglc.core

import kotlinx.serialization.Serializable

@Serializable enum class LaunchAgent(val label: String, val executable: String, val defaultBypass: String) {
    Claude("Claude Code", "claude", "--dangerously-skip-permissions"),
    Codex("Codex", "codex", "--yolo"),
    Agy("AGY", "agy", "--dangerously-skip-permissions"),
    Grok("Grok Build", "grok", "--always-approve");

    fun defaults() = AgentLaunch(defaultBypass, if (this == Claude) listOf(
        LaunchOption("Code Mode", t("launcher.code_mode_description"), OptionKind.Environment, "CLAUDE_CODE_EXPERIMENTAL_CODE_MODE=1")
    ) else emptyList())
}

@Serializable enum class OptionKind { Environment, Argument }

fun launchSingleLine(text: String) = text.none { it.isISOControl() || it == '\u2028' || it == '\u2029' }

private val envKey = Regex("[A-Za-z_][A-Za-z0-9_]*")
private val simpleValue = Regex("[A-Za-z0-9_./-]*")

@Serializable data class LaunchOption(
    val name: String = "", val description: String = "", val kind: OptionKind = OptionKind.Argument,
    val value: String = "", val checked: Boolean = false,
) {
    val help: String get() = if (description.isEmpty()) name else "$name\n$description"
    val valid: Boolean get() = name.isNotBlank() && launchSingleLine(name) && launchSingleLine(description) &&
        launchSingleLine(value) && when (kind) {
            OptionKind.Environment -> '=' in value && envKey.matches(value.substringBefore('='))
            OptionKind.Argument -> value.isNotBlank()
        }
}

@Serializable data class AgentLaunch(val bypass: String, val options: List<LaunchOption>, val bypassChecked: Boolean = false) {
    /** Preserve the user’s shell argument fragments on the selected host. */
    fun command(agent: LaunchAgent, powershell: Boolean = false): String? {
        val env = mutableListOf<String>()
        val assignments = mutableListOf<Pair<String, String>>()
        val words = mutableListOf(agent.executable)
        if (bypassChecked && bypass.isNotEmpty()) {
            if (!launchSingleLine(bypass)) return null
            words += bypass
        }
        for (option in options.filter { it.checked }) {
            if (!option.valid) return null
            when (option.kind) {
                OptionKind.Argument -> words += option.value
                OptionKind.Environment -> {
                    val value = option.value.substringAfter('=')
                    assignments += option.value.substringBefore('=') to value
                    val literal = if (simpleValue.matches(value)) value else "'" + value.replace("'", "'\\''") + "'"
                    env += option.value.substringBefore('=') + "=" + literal
                }
            }
        }
        if (powershell && assignments.isNotEmpty()) return buildString {
            append("& { \$dglPrevious = @(")
            append(assignments.joinToString(", ") { "\$env:" + it.first })
            append("); try { ")
            for ((key, value) in assignments) {
                append("\$env:$key = '")
                for (ch in value) {
                    if (ch == '\'' || ch in '\u2018'..'\u201b') append('\'')
                    append(ch)
                }
                append("'; ")
            }
            append(words.joinToString(" "))
            append(" } finally { ")
            assignments.forEachIndexed { i, (key, _) -> append("\$env:$key = \$dglPrevious[$i]; ") }
            append("} }\r")
        }
        return (env + words).joinToString(" ") + "\r"
    }
}

fun Preferences.launchFor(agent: LaunchAgent) = agentLaunch[agent] ?: agent.defaults()
fun Preferences.updateLaunch(agent: LaunchAgent, edit: (AgentLaunch) -> AgentLaunch) =
    copy(agentLaunch = agentLaunch + (agent to edit(launchFor(agent))))
