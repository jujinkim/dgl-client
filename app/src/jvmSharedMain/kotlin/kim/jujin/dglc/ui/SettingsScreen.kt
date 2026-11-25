package kim.jujin.dglc.ui

import androidx.compose.foundation.*
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kim.jujin.dglc.*
import kim.jujin.dglc.core.*

/** Settings categories in navigation order. [clusterStart] begins a new navigation group. */
internal enum class SettingsCategory(val titleKey: String, val descriptionKey: String?, val clusterStart: Boolean = false) {
    General("settings.category_general", "settings.general_description"),
    Terminal("settings.category_terminal", "settings.terminal_description"),
    Keyboard("settings.category_keyboard", "settings.keyboard_description"),
    Shortcuts("settings.category_shortcuts", "settings.shortcuts_description"),
    Connection("settings.connection", "settings.connection_description"),
    Agents("settings.category_agents", "settings.agents_description", clusterStart = true),
    Launch("settings.category_launch", "launcher.settings"),
    Sync("sync.export_and_sync", "sync.description", clusterStart = true),
    Security("settings.security_storage", "settings.security_description"),
    About("settings.about", null);

    /** The shortcut list is lazy and owns its scrolling; every other page scrolls in the detail column. */
    val scrollsItself: Boolean get() = this == Shortcuts
}

/** How settings open: a category, or null for the category list (compact) / General (two-pane). */
internal data class SettingsLaunch(val category: SettingsCategory?)

/** The category whose detail is on screen; null shows the compact category list. */
internal fun shownCategory(route: SettingsCategory?, twoPane: Boolean): SettingsCategory? =
    route ?: if (twoPane) SettingsCategory.General else null

internal enum class SettingsBack { List, Close }

/** Back from a compact detail returns to the list, even when opened straight into a category. */
internal fun settingsBack(route: SettingsCategory?, twoPane: Boolean): SettingsBack =
    if (twoPane || route == null) SettingsBack.Close else SettingsBack.List

/** Navigation clusters, separated by a gap. */
internal fun settingsClusters(): List<List<SettingsCategory>> =
    SettingsCategory.entries.fold(mutableListOf<MutableList<SettingsCategory>>()) { clusters, category ->
        if (clusters.isEmpty() || category.clusterStart) clusters += mutableListOf<SettingsCategory>()
        clusters.last() += category
        clusters
    }

@Composable internal fun SettingsScreen(model: AppModel, mobileLayout: Boolean, initial: SettingsCategory?, dismiss: () -> Unit) {
    val twoPane = !mobileLayout
    var route by remember { mutableStateOf(initial) }
    var restoreEntry by remember { mutableStateOf<SettingsCategory?>(null) }
    val listScroll = rememberScrollState()
    val navScroll = rememberScrollState()
    val entries = remember { SettingsCategory.entries.associateWith { FocusRequester() } }
    val shown = shownCategory(route, twoPane)
    fun leave(category: SettingsCategory?) { if (category == SettingsCategory.Sync) model.cloudSync?.cancelImport() }
    val back: () -> Unit = {
        when (settingsBack(route, twoPane)) {
            SettingsBack.List -> { leave(route); restoreEntry = route; route = null }
            SettingsBack.Close -> { leave(shown); dismiss() }
        }
    }
    val open: (SettingsCategory) -> Unit = { category ->
        if (category != shown) leave(shown)
        route = category
    }
    LaunchedEffect(shown) {
        val entry = restoreEntry
        if (shown == null && entry != null) {
            withFrameNanos { }
            entries.getValue(entry).requestFocus()
            restoreEntry = null
        }
    }
    val close: @Composable () -> Unit = {
        SettingsActionButton(t("common.close"), { leave(shown); dismiss() })
    }
    Dialog(back, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        if (!twoPane) Surface(Modifier.fillMaxSize()) {
            Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding()) {
                if (shown == null) {
                    Text(t("common.settings"), Modifier.padding(horizontal = SettingsSpacing.page, vertical = 12.dp).semantics { heading() },
                        style = MaterialTheme.typography.titleLarge)
                    Column(Modifier.weight(1f).fillMaxWidth().verticalScroll(listScroll).padding(horizontal = SettingsSpacing.page, vertical = 8.dp),
                        verticalArrangement = Arrangement.spacedBy(SettingsSpacing.groupGap)) {
                        settingsClusters().forEach { cluster ->
                            SettingsGroup(null) {
                                cluster.forEachIndexed { index, category ->
                                    if (index > 0) SettingsDivider()
                                    SettingsNavRow(t(category.titleKey), { open(category) },
                                        Modifier.focusProperties { canFocus = true }.focusRequester(entries.getValue(category)))
                                }
                            }
                        }
                    }
                } else {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SettingsActionButton(t("common.back"), back)
                        Text(t(shown.titleKey), Modifier.weight(1f).semantics { heading() }, style = MaterialTheme.typography.titleLarge)
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    Box(Modifier.weight(1f).fillMaxWidth()) {
                        SettingsDetail(model, shown, wide = false, showTitle = false)
                    }
                }
                HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                Row(Modifier.fillMaxWidth().padding(horizontal = SettingsSpacing.page, vertical = 8.dp), horizontalArrangement = Arrangement.End) { close() }
            }
        } else BoxWithConstraints(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(24.dp), contentAlignment = Alignment.Center) {
            val category = shown ?: SettingsCategory.General
            Surface(Modifier.widthIn(max = 960.dp).fillMaxWidth().height(minOf(maxHeight, 760.dp)), shape = RoundedCornerShape(16.dp)) {
                Column(Modifier.fillMaxSize()) {
                    Row(Modifier.weight(1f).fillMaxWidth()) {
                        Column(Modifier.width(SettingsSpacing.navWidth).fillMaxHeight().verticalScroll(navScroll).padding(12.dp)) {
                            Text(t("common.settings"), Modifier.padding(horizontal = 12.dp, vertical = 8.dp).semantics { heading() },
                                style = MaterialTheme.typography.titleLarge)
                            settingsClusters().forEachIndexed { index, cluster ->
                                if (index > 0) Spacer(Modifier.height(12.dp))
                                cluster.forEach { entry -> SettingsNavItem(t(entry.titleKey), entry == category) { open(entry) } }
                            }
                        }
                        VerticalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                        Box(Modifier.weight(1f).fillMaxHeight()) { SettingsDetail(model, category, wide = true, showTitle = true) }
                    }
                    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
                    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), horizontalArrangement = Arrangement.End) { close() }
                }
            }
        }
    }
}

/** Two-pane navigation item: a tab with its selection drawn as a filled, rounded row. */
@Composable private fun SettingsNavItem(label: String, selected: Boolean, select: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.navRow).clip(SettingsShapes.control)
        .background(if (selected) colors.secondaryContainer else androidx.compose.ui.graphics.Color.Transparent)
        .selectable(selected, role = Role.Tab, onClick = select).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, style = MaterialTheme.typography.bodyLarge, color = if (selected) colors.onSecondaryContainer else colors.onSurface)
    }
}

@Composable private fun SettingsDetail(model: AppModel, category: SettingsCategory, wide: Boolean, showTitle: Boolean) {
    val local by model.local.collectAsState()
    val padding = if (wide) SettingsSpacing.pageWide else SettingsSpacing.page
    val title = if (showTitle) t(category.titleKey) else null
    val description = category.descriptionKey?.let { t(it) }
    if (category.scrollsItself) {
        TerminalShortcutSettings(local.preferences.terminalShortcuts, contentPadding = PaddingValues(padding), header = {
            item { SettingsPageHeader(title, description) }
            item {
                // The lazy list spaces items by 6dp; make up the shared group gap like the shortcut sections do.
                SettingsGroup(t("settings.bottom_shortcuts"), modifier = Modifier.padding(top = SettingsSpacing.groupGap - 6.dp)) {
                    SettingsToggleRow(t("settings.bottom_shortcuts_two_rows"), local.preferences.mobileShortcutBarTwoRows,
                        t("settings.narrow_screen_note")) { enabled ->
                        model.modify { it.copy(preferences = it.preferences.copy(mobileShortcutBarTwoRows = enabled)) }
                    }
                }
            }
        }) { shortcuts -> model.modify { it.copy(preferences = it.preferences.copy(terminalShortcuts = shortcuts)) } }
        return
    }
    // A fresh scroll position per category; lazy or scrolling children would crash inside this column.
    val scroll = remember(category) { ScrollState(0) }
    Column(Modifier.fillMaxSize().verticalScroll(scroll).padding(padding), verticalArrangement = Arrangement.spacedBy(SettingsSpacing.groupGap)) {
        SettingsPageHeader(title, description)
        when (category) {
            SettingsCategory.General -> GeneralSettings(model, local.preferences)
            SettingsCategory.Terminal -> TerminalSettings(model, local.preferences)
            SettingsCategory.Keyboard -> KeyboardSettings(model, local.preferences)
            SettingsCategory.Shortcuts -> Unit
            SettingsCategory.Connection -> PlatformConnectionSettings(model)
            SettingsCategory.Agents -> { AgentNotificationSettingsUi(model); AgentStatusSettingsUi(model) }
            SettingsCategory.Launch -> LaunchSettings(local.preferences) { agent, edit ->
                model.modify { it.copy(preferences = it.preferences.updateLaunch(agent, edit)) }
            }
            SettingsCategory.Sync -> CloudSyncSettings(model)
            SettingsCategory.Security -> SecuritySettings(model, local)
            SettingsCategory.About -> AboutApp()
        }
    }
}

private fun AppModel.editPreferences(change: (Preferences) -> Preferences) = modify { it.copy(preferences = change(it.preferences)) }

@Composable private fun GeneralSettings(model: AppModel, preferences: Preferences) {
    SettingsGroup(null) {
        SettingsChoiceRow(t("settings.language"), listOf("system" to t("common.system")) + UiLanguage.all.map { it.code to it.nativeName },
            UiLanguage.of(preferences.language)?.code ?: "system") { value -> model.editPreferences { it.copy(language = value) } }
        SettingsDivider()
        SettingsChoiceRow(t("settings.theme"), listOf("system" to t("common.system"), "light" to t("settings.theme_light"), "dark" to t("settings.theme_dark")),
            preferences.theme) { value -> model.editPreferences { it.copy(theme = value) } }
    }
    SettingsGroup(t("accent.accent_color")) {
        AccentSettingsUi(preferences.accent) { accent -> model.editPreferences { it.copy(accent = accent) } }
    }
    SettingsGroup(t("settings.workspace")) {
        SettingsToggleRow(t("settings.show_single_windows_too"), preferences.showAllHierarchy, t("settings.show_single_windows_description")) { enabled ->
            model.editPreferences { it.copy(showAllHierarchy = enabled) }
        }
    }
}

@Composable private fun TerminalSettings(model: AppModel, preferences: Preferences) {
    // Text and display come first so size and transitions are reachable without scrolling.
    SettingsGroup(t("settings.group_text")) {
        SettingsRow(tr("settings.terminal_font_size", preferences.fontSize)) {
            SettingsActionButton(t("settings.smaller"), { model.editPreferences { it.copy(fontSize = (it.fontSize - 1).coerceAtLeast(10)) } })
            SettingsActionButton(t("settings.larger"), { model.editPreferences { it.copy(fontSize = (it.fontSize + 1).coerceAtMost(28)) } })
        }
        PlatformTerminalFontSettings(preferences.terminalFontFamily) { family -> model.editPreferences { it.copy(terminalFontFamily = family) } }
    }
    SettingsGroup(t("settings.group_display")) {
        SettingsToggleRow(t("settings.smooth_transitions"), preferences.smoothTerminalTransitions, t("settings.smooth_transitions_description")) { enabled ->
            model.editPreferences { it.copy(smoothTerminalTransitions = enabled) }
        }
        SettingsDivider()
        SettingsNote(tr("settings.scrollback_description", preferences.scrollback), Modifier.padding(vertical = 6.dp))
    }
    SettingsGroup(t("settings.scroll_assist_title"), t("settings.scroll_assist_description")) {
        SettingsChoiceRow(null, listOf("auto" to t("settings.scroll_assist_auto"), "on" to t("settings.scroll_assist_on"), "off" to t("settings.scroll_assist_off")),
            preferences.effectiveTerminalScrollAssist, description = t(when (preferences.effectiveTerminalScrollAssist) {
                "on" -> "settings.scroll_assist_on_description"
                "off" -> "settings.scroll_assist_off_description"
                else -> "settings.scroll_assist_auto_description"
            })) { value -> model.editPreferences { it.copy(terminalScrollAssist = value) } }
    }
}

@Composable private fun KeyboardSettings(model: AppModel, preferences: Preferences) {
    val always = preferences.androidKeyboardAlwaysVisible
    SettingsGroup(null) {
        SettingsToggleRow(t("launcher.always_show_keyboard"), always, t("settings.keyboard_always_description")) { enabled ->
            model.editPreferences { it.copy(androidKeyboardAlwaysVisible = enabled) }
        }
        SettingsDivider()
        // Moot while the keyboard always shows.
        SettingsToggleRow(t("settings.keyboard_on_touch"), preferences.androidKeyboardOnTouch, t("settings.keyboard_on_touch_description"),
            enabled = !always) { enabled -> model.editPreferences { it.copy(androidKeyboardOnTouch = enabled) } }
        SettingsDivider()
        SettingsChoiceRow(t("settings.keyboard_when_opens"),
            listOf("auto" to t("settings.keyboard_automatic"), "pan" to t("common.move_up"), "resize" to t("settings.keyboard_resize")),
            preferences.effectiveAndroidKeyboardMode, enabled = !always, description = t(when (preferences.effectiveAndroidKeyboardMode) {
                "pan" -> "settings.keyboard_pan_description"
                "resize" -> "settings.keyboard_resize_description"
                else -> "settings.keyboard_automatic_description"
            })) { value -> model.editPreferences { it.copy(androidKeyboardMode = value) } }
    }
    SettingsGroup(t("settings.input_correction")) {
        SettingsChoiceRow(null, listOf("off" to t("common.off"), "boundary" to t("settings.input_correction_boundary"),
            "edit" to t("settings.input_correction_edit")), preferences.effectiveAndroidInputCorrection,
            description = t(when (preferences.effectiveAndroidInputCorrection) {
                "boundary" -> "settings.input_correction_boundary_description"
                "edit" -> "settings.input_correction_edit_description"
                else -> "settings.input_correction_off_description"
            })) { value -> model.editPreferences { it.copy(androidInputCorrection = value) } }
    }
    PlatformInputSettings(preferences) { next -> model.update(model.local.value.copy(preferences = next)) }
}

@Composable private fun SecuritySettings(model: AppModel, local: LocalState) {
    var forget by remember { mutableStateOf<String?>(null) }
    SettingsGroup(t("settings.trusted_servers")) {
        if (local.trust.isEmpty()) SettingsNote(t("settings.no_trusted_servers"), Modifier.padding(vertical = 10.dp))
        local.trust.entries.forEachIndexed { index, (endpoint, fingerprint) ->
            if (index > 0) SettingsDivider()
            Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight).padding(horizontal = SettingsSpacing.rowH, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(SettingsSpacing.controlGap)) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(endpoint, style = MaterialTheme.typography.bodyLarge)
                    SelectionContainer {
                        Text(fingerprint, style = MaterialTheme.typography.labelSmall, fontFamily = FontFamily.Monospace,
                            color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                SettingsActionButton(t("settings.remove_trusted_key"), { forget = endpoint }, style = SettingsActionStyle.Danger,
                    modifier = Modifier.semantics { contentDescription = tr("settings.remove_trust_named", endpoint) })
            }
        }
    }
    SettingsGroup(t("settings.storage_location")) {
        SettingsNote(model.storageDescription, Modifier.padding(vertical = 10.dp))
    }
    forget?.let { endpoint ->
        ConfirmDialog(t("settings.remove_trusted_key"), tr("settings.remove_trust_confirm", endpoint),
            { model.modify { it.copy(trust = it.trust - endpoint) }; forget = null }, { forget = null })
    }
}

@Composable private fun AboutApp() {
    val uri = LocalUriHandler.current
    Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Image(AppIcon, contentDescription = "DaGaLi", modifier = Modifier.size(80.dp).clip(RoundedCornerShape(16.dp)))
        Text("DaGaLi", style = MaterialTheme.typography.headlineMedium)
        Text(tr("settings.about_version", APP_VERSION), style = MaterialTheme.typography.bodyLarge)
        Text("Reconnect less. Resume more.", style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
    SettingsGroup(null) {
        listOf(t("settings.about_website") to "https://ddal.ggak.life", t("settings.about_github") to "https://github.com/jujinkim/dgl-client")
            .forEachIndexed { index, (label, url) ->
                if (index > 0) SettingsDivider()
                // The URL stays in the row's text so it is announced and can be matched.
                Row(Modifier.fillMaxWidth().heightIn(min = SettingsSpacing.rowMinHeight).clickable(role = Role.Button) { uri.openUri(url) }
                    .padding(horizontal = SettingsSpacing.rowH, vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(label, style = MaterialTheme.typography.bodyLarge)
                        Text(url, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    UiGlyph(UiIcons.chevronRight, tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
    }
}
