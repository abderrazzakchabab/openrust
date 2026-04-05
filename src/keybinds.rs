use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

/// All possible actions that can be bound to keys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyAction {
    NewSession,
    SaveSession,
    ShowHelp,
    ShowSessions,
    Quit,
    SwitchModel,
    SwitchTheme,
    ToggleThinking,
    ShowDetails,
    ExportConversation,
    CompactHistory,
    OpenEditor,
    InitProject,
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
}

/// A single key binding
#[derive(Debug, Clone)]
pub struct KeyBinding {
    pub key: KeyCode,
    pub modifiers: KeyModifiers,
    pub requires_leader: bool,
    pub action: KeyAction,
    pub description: &'static str,
}

impl KeyBinding {
    fn new(
        key: KeyCode,
        modifiers: KeyModifiers,
        requires_leader: bool,
        action: KeyAction,
        description: &'static str,
    ) -> Self {
        KeyBinding {
            key,
            modifiers,
            requires_leader,
            action,
            description,
        }
    }
}

/// Manages keybindings with leader key support
pub struct KeybindManager {
    bindings: Vec<KeyBinding>,
    leader_key: (KeyCode, KeyModifiers),
    leader_active: bool,
    leader_timeout: Duration,
    leader_pressed_at: Option<Instant>,
}

impl KeybindManager {
    /// Create a new keybind manager with default bindings
    pub fn new() -> Self {
        let mut manager = KeybindManager {
            bindings: Vec::new(),
            leader_key: (KeyCode::Char('x'), KeyModifiers::CONTROL),
            leader_active: false,
            leader_timeout: Duration::from_secs(2),
            leader_pressed_at: None,
        };
        manager.register_defaults();
        manager
    }

    /// Register all default keybindings
    fn register_defaults(&mut self) {
        // Leader-based bindings (Ctrl+X then key)
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('n'),
            KeyModifiers::NONE,
            true,
            KeyAction::NewSession,
            "New session",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('s'),
            KeyModifiers::NONE,
            true,
            KeyAction::SaveSession,
            "Save session",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('h'),
            KeyModifiers::NONE,
            true,
            KeyAction::ShowHelp,
            "Show help",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('l'),
            KeyModifiers::NONE,
            true,
            KeyAction::ShowSessions,
            "Show session list",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
            true,
            KeyAction::Quit,
            "Quit",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('m'),
            KeyModifiers::NONE,
            true,
            KeyAction::SwitchModel,
            "Switch model",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('t'),
            KeyModifiers::NONE,
            true,
            KeyAction::SwitchTheme,
            "Switch theme",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('d'),
            KeyModifiers::NONE,
            true,
            KeyAction::ShowDetails,
            "Show details",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('e'),
            KeyModifiers::NONE,
            true,
            KeyAction::ExportConversation,
            "Export conversation",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('c'),
            KeyModifiers::NONE,
            true,
            KeyAction::CompactHistory,
            "Compact history",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('i'),
            KeyModifiers::NONE,
            true,
            KeyAction::InitProject,
            "Initialize project",
        ));

        // Direct bindings (no leader required)
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
            false,
            KeyAction::Quit,
            "Quit",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('n'),
            KeyModifiers::CONTROL,
            false,
            KeyAction::NewSession,
            "New session",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('s'),
            KeyModifiers::CONTROL,
            false,
            KeyAction::SaveSession,
            "Save session",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('l'),
            KeyModifiers::CONTROL,
            false,
            KeyAction::ShowSessions,
            "Show session list",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::F(1),
            KeyModifiers::NONE,
            false,
            KeyAction::ShowHelp,
            "Show help",
        ));
        self.bindings.push(KeyBinding::new(
            KeyCode::Char('?'),
            KeyModifiers::NONE,
            false,
            KeyAction::ShowHelp,
            "Show help",
        ));
    }

    /// Handle a key press and return the associated action if any
    pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Option<KeyAction> {
        // Check if this key press is the leader key
        if self.matches_leader(key, modifiers) {
            self.leader_active = true;
            self.leader_pressed_at = Some(Instant::now());
            return None;
        }

        // Check if leader is active
        if self.leader_active {
            // Check if leader timeout expired
            if let Some(pressed_at) = self.leader_pressed_at {
                if pressed_at.elapsed() > self.leader_timeout {
                    self.reset_leader();
                    // Fall through to check direct bindings
                } else {
                    // Leader is active and within timeout - look for leader binding
                    let action = self.find_leader_binding(key, modifiers);
                    self.reset_leader();
                    return action;
                }
            }
        }

        // Check direct bindings
        self.find_direct_binding(key, modifiers)
    }

    /// Check if the leader key is active
    pub fn is_leader_active(&self) -> bool {
        if !self.leader_active {
            return false;
        }

        // Check if timeout expired
        if let Some(pressed_at) = self.leader_pressed_at {
            pressed_at.elapsed() <= self.leader_timeout
        } else {
            false
        }
    }

    /// Get all bindings for display in help
    pub fn list_bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Cancel leader mode
    pub fn reset_leader(&mut self) {
        self.leader_active = false;
        self.leader_pressed_at = None;
    }

    /// Check if key + modifiers match the leader key
    fn matches_leader(&self, key: KeyCode, modifiers: KeyModifiers) -> bool {
        key == self.leader_key.0 && modifiers == self.leader_key.1
    }

    /// Find a leader-based binding
    fn find_leader_binding(&self, key: KeyCode, modifiers: KeyModifiers) -> Option<KeyAction> {
        self.bindings
            .iter()
            .find(|b| b.requires_leader && b.key == key && b.modifiers == modifiers)
            .map(|b| b.action.clone())
    }

    /// Find a direct (non-leader) binding
    fn find_direct_binding(&self, key: KeyCode, modifiers: KeyModifiers) -> Option<KeyAction> {
        self.bindings
            .iter()
            .find(|b| !b.requires_leader && b.key == key && b.modifiers == modifiers)
            .map(|b| b.action.clone())
    }

    /// Get the leader key as a string for display
    pub fn leader_key_display(&self) -> String {
        format_key_combo(self.leader_key.0, self.leader_key.1)
    }
}

impl Default for KeybindManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Format a key combination for display
pub fn format_key_combo(key: KeyCode, modifiers: KeyModifiers) -> String {
    let mut result = String::new();

    if modifiers.contains(KeyModifiers::CONTROL) {
        result.push_str("Ctrl+");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        result.push_str("Alt+");
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        result.push_str("Shift+");
    }

    match key {
        KeyCode::Char(c) => {
            if c == ' ' {
                result.push_str("Space");
            } else {
                result.push_str(&c.to_uppercase().to_string());
            }
        }
        KeyCode::F(n) => result.push_str(&format!("F{}", n)),
        KeyCode::Enter => result.push_str("Enter"),
        KeyCode::Esc => result.push_str("Esc"),
        KeyCode::Backspace => result.push_str("Backspace"),
        KeyCode::Tab => result.push_str("Tab"),
        KeyCode::Up => result.push_str("Up"),
        KeyCode::Down => result.push_str("Down"),
        KeyCode::Left => result.push_str("Left"),
        KeyCode::Right => result.push_str("Right"),
        KeyCode::Home => result.push_str("Home"),
        KeyCode::End => result.push_str("End"),
        KeyCode::PageUp => result.push_str("PgUp"),
        KeyCode::PageDown => result.push_str("PgDn"),
        _ => result.push_str("?"),
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_leader_key_sequence() {
        let mut manager = KeybindManager::new();

        // Press leader key (Ctrl+X)
        let result = manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(result, None);
        assert!(manager.is_leader_active());

        // Press 'n' for new session
        let result = manager.handle_key(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(result, Some(KeyAction::NewSession));
        assert!(!manager.is_leader_active());
    }

    #[test]
    fn test_direct_keybind() {
        let mut manager = KeybindManager::new();

        // Ctrl+Q should quit directly without leader
        let result = manager.handle_key(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(result, Some(KeyAction::Quit));
        assert!(!manager.is_leader_active());
    }

    #[test]
    fn test_leader_timeout() {
        let mut manager = KeybindManager::new();
        manager.leader_timeout = Duration::from_millis(100);

        // Press leader key
        let result = manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert_eq!(result, None);
        assert!(manager.is_leader_active());

        // Wait for timeout
        sleep(Duration::from_millis(150));

        // Leader should no longer be active
        assert!(!manager.is_leader_active());

        // Pressing 'n' should not trigger NewSession (timeout expired)
        let result = manager.handle_key(KeyCode::Char('n'), KeyModifiers::NONE);
        assert_eq!(result, None);
    }

    #[test]
    fn test_unknown_key_after_leader() {
        let mut manager = KeybindManager::new();

        // Press leader key
        manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);

        // Press an unknown key
        let result = manager.handle_key(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(result, None);
        assert!(!manager.is_leader_active());
    }

    #[test]
    fn test_leader_key_detection() {
        let mut manager = KeybindManager::new();

        // Initially not active
        assert!(!manager.is_leader_active());

        // Press leader key
        manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);

        // Should be active
        assert!(manager.is_leader_active());
    }

    #[test]
    fn test_default_bindings_count() {
        let manager = KeybindManager::new();
        let bindings = manager.list_bindings();

        // Should have leader bindings + direct bindings
        // Leader: n, s, h, l, q, m, t, d, e, c, i (11)
        // Direct: Ctrl+Q, Ctrl+N, Ctrl+S, Ctrl+L, F1, ? (6)
        assert_eq!(bindings.len(), 17);
    }

    #[test]
    fn test_list_bindings() {
        let manager = KeybindManager::new();
        let bindings = manager.list_bindings();

        // Verify all bindings are listed
        assert!(!bindings.is_empty());

        // Find a specific binding
        let quit_binding = bindings.iter().find(|b| {
            b.action == KeyAction::Quit
                && !b.requires_leader
                && b.key == KeyCode::Char('q')
                && b.modifiers.contains(KeyModifiers::CONTROL)
        });
        assert!(quit_binding.is_some());
    }

    #[test]
    fn test_reset_leader() {
        let mut manager = KeybindManager::new();

        // Activate leader
        manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(manager.is_leader_active());

        // Reset leader
        manager.reset_leader();
        assert!(!manager.is_leader_active());
    }

    #[test]
    fn test_format_key_combo() {
        assert_eq!(
            format_key_combo(KeyCode::Char('q'), KeyModifiers::CONTROL),
            "Ctrl+Q"
        );
        assert_eq!(
            format_key_combo(KeyCode::Char('n'), KeyModifiers::NONE),
            "N"
        );
        assert_eq!(format_key_combo(KeyCode::F(1), KeyModifiers::NONE), "F1");
        assert_eq!(
            format_key_combo(
                KeyCode::Char('x'),
                KeyModifiers::CONTROL | KeyModifiers::SHIFT
            ),
            "Ctrl+Shift+X"
        );
    }

    #[test]
    fn test_multiple_leader_sequences() {
        let mut manager = KeybindManager::new();

        // First sequence: Ctrl+X, s (save)
        manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
        let result = manager.handle_key(KeyCode::Char('s'), KeyModifiers::NONE);
        assert_eq!(result, Some(KeyAction::SaveSession));

        // Second sequence: Ctrl+X, h (help)
        manager.handle_key(KeyCode::Char('x'), KeyModifiers::CONTROL);
        let result = manager.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);
        assert_eq!(result, Some(KeyAction::ShowHelp));
    }

    #[test]
    fn test_leader_display() {
        let manager = KeybindManager::new();
        assert_eq!(manager.leader_key_display(), "Ctrl+X");
    }
}
