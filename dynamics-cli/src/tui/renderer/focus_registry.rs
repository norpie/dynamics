use crate::tui::command::DispatchTarget;
use crate::tui::element::FocusId;
use crossterm::event::KeyEvent;
use ratatui::layout::Rect;

/// Information about a focusable element
pub struct FocusableInfo<Msg> {
    pub id: FocusId,
    pub rect: Rect,
    pub on_key: Box<dyn Fn(KeyEvent) -> DispatchTarget<Msg> + Send>,
    pub on_focus: Option<Msg>,
    pub on_blur: Option<Msg>,
    pub inside_panel: bool, // True if this element is inside a Panel
}

/// Focus context for a single layer in the UI
pub struct LayerFocusContext<Msg> {
    pub layer_index: usize,
    pub focusables: Vec<FocusableInfo<Msg>>,
    pub focused_id: Option<FocusId>, // Focused element in this layer
}

/// Stores focus information for UI elements, organized by layer
pub struct FocusRegistry<Msg> {
    layers: Vec<LayerFocusContext<Msg>>,
    /// Saved focus states from previous frame (used to restore focus across layer changes)
    saved_focus_states: Vec<Option<FocusId>>,
}

impl<Msg: Clone> FocusRegistry<Msg> {
    pub fn new() -> Self {
        Self {
            layers: vec![LayerFocusContext {
                layer_index: 0,
                focusables: Vec::new(),
                focused_id: None,
            }],
            saved_focus_states: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        log::debug!(
            "FocusRegistry::clear() - before: {} layers, saved_states: {:?}",
            self.layers.len(),
            self.saved_focus_states
                .iter()
                .enumerate()
                .map(|(i, s)| format!("[{}]={:?}", i, s))
                .collect::<Vec<_>>()
        );

        // Update saved_focus_states with current layers' states, but preserve states
        // for layers not currently in the registry (e.g., base layer when modal is on top)
        for layer in &self.layers {
            let idx = layer.layer_index;
            log::debug!("  Saving layer {} focus: {:?}", idx, layer.focused_id);
            // Extend the vec if needed
            while self.saved_focus_states.len() <= idx {
                self.saved_focus_states.push(None);
            }
            self.saved_focus_states[idx] = layer.focused_id.clone();
        }

        log::debug!(
            "FocusRegistry::clear() - after save, saved_states: {:?}",
            self.saved_focus_states
                .iter()
                .enumerate()
                .map(|(i, s)| format!("[{}]={:?}", i, s))
                .collect::<Vec<_>>()
        );

        self.layers.clear();
        let layer0_focus = self.saved_focus_states.get(0).cloned().flatten();
        log::debug!(
            "FocusRegistry::clear() - restoring layer 0 with focus: {:?}",
            layer0_focus
        );

        self.layers.push(LayerFocusContext {
            layer_index: 0,
            focusables: Vec::new(),
            // Restore layer 0's focus if it existed
            focused_id: layer0_focus,
        });
    }

    pub fn push_layer(&mut self, layer_index: usize) {
        let restored_focus = self.saved_focus_states.get(layer_index).cloned().flatten();
        log::debug!(
            "FocusRegistry::push_layer({}) - restoring focus: {:?}",
            layer_index,
            restored_focus
        );
        self.layers.push(LayerFocusContext {
            layer_index,
            focusables: Vec::new(),
            // Restore focus from previous frame if it exists for this layer
            focused_id: restored_focus,
        });
    }

    pub fn pop_layer(&mut self) {
        if self.layers.len() > 1 {
            let popped = self.layers.pop();
            log::debug!(
                "FocusRegistry::pop_layer() - popped layer {:?}",
                popped.as_ref().map(|l| l.layer_index)
            );
        }
    }

    fn current_layer_mut(&mut self) -> &mut LayerFocusContext<Msg> {
        self.layers
            .last_mut()
            .expect("FocusRegistry should always have at least one layer")
    }

    pub fn register_focusable(&mut self, info: FocusableInfo<Msg>) {
        // Check for duplicate IDs and warn/panic
        if self
            .current_layer_mut()
            .focusables
            .iter()
            .any(|f| f.id == info.id)
        {
            #[cfg(debug_assertions)]
            panic!(
                "Duplicate FocusId detected: {:?}. Each focusable element must have a unique ID within its layer.",
                info.id
            );

            #[cfg(not(debug_assertions))]
            eprintln!(
                "WARNING: Duplicate FocusId: {:?} - last registration wins",
                info.id
            );
        }

        self.current_layer_mut().focusables.push(info);
    }

    pub fn active_layer(&self) -> Option<&LayerFocusContext<Msg>> {
        self.layers.last()
    }

    pub fn iter_active_layer(&self) -> impl Iterator<Item = &FocusableInfo<Msg>> {
        self.active_layer()
            .map(|layer| layer.focusables.iter())
            .into_iter()
            .flatten()
    }

    pub fn find_in_active_layer(&self, id: &FocusId) -> Option<&FocusableInfo<Msg>> {
        self.active_layer()?.focusables.iter().find(|f| &f.id == id)
    }

    pub fn focusable_ids_in_active_layer(&self) -> Vec<FocusId> {
        self.active_layer()
            .map(|layer| layer.focusables.iter().map(|f| f.id.clone()).collect())
            .unwrap_or_default()
    }

    pub fn find_at_position(&self, x: u16, y: u16) -> Option<FocusId> {
        self.active_layer()?
            .focusables
            .iter()
            .rev()
            .find(|f| self.point_in_rect(x, y, f.rect))
            .map(|f| f.id.clone())
    }

    pub fn contains(&self, id: &FocusId) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.focusables.iter().any(|f| &f.id == id))
    }

    fn point_in_rect(&self, x: u16, y: u16, rect: Rect) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    /// Move focus to the next focusable element in the active layer
    /// Returns the new focused ID, or None if there are no focusables
    pub fn next_focus(&self, current: Option<&FocusId>) -> Option<FocusId> {
        let layer = self.active_layer()?;
        if layer.focusables.is_empty() {
            return None;
        }

        match current {
            None => {
                // No current focus, focus the first element
                Some(layer.focusables[0].id.clone())
            }
            Some(id) => {
                // Find current index
                let current_index = layer.focusables.iter().position(|f| &f.id == id)?;
                // Wrap around to next
                let next_index = (current_index + 1) % layer.focusables.len();
                Some(layer.focusables[next_index].id.clone())
            }
        }
    }

    /// Move focus to the previous focusable element in the active layer
    /// Returns the new focused ID, or None if there are no focusables
    pub fn prev_focus(&self, current: Option<&FocusId>) -> Option<FocusId> {
        let layer = self.active_layer()?;
        if layer.focusables.is_empty() {
            return None;
        }

        match current {
            None => {
                // No current focus, focus the last element
                Some(layer.focusables[layer.focusables.len() - 1].id.clone())
            }
            Some(id) => {
                // Find current index
                let current_index = layer.focusables.iter().position(|f| &f.id == id)?;
                // Wrap around to previous
                let prev_index = if current_index == 0 {
                    layer.focusables.len() - 1
                } else {
                    current_index - 1
                };
                Some(layer.focusables[prev_index].id.clone())
            }
        }
    }

    /// Dispatch a key event to the focused element
    /// Returns Some(Msg) if the element handled the key and produced a message
    pub fn dispatch_key(&self, focused_id: &FocusId, key_event: KeyEvent) -> Option<Msg> {
        use crate::tui::command::DispatchTarget;

        let focusable = self.find_in_active_layer(focused_id)?;
        match (focusable.on_key)(key_event) {
            DispatchTarget::AppMsg(msg) => Some(msg),
            DispatchTarget::WidgetEvent(_) => None, // Widget events not supported in global focus
            DispatchTarget::PassThrough => None,
        }
    }

    /// Save the current focused element ID in the active layer
    pub fn save_layer_focus(&mut self, focused_id: Option<FocusId>) {
        if let Some(layer) = self.layers.last_mut() {
            let layer_idx = layer.layer_index;
            log::debug!(
                "FocusRegistry::save_layer_focus() - saving {:?} to layer {}",
                focused_id,
                layer_idx
            );
            layer.focused_id = focused_id.clone();

            // Also update saved_focus_states to ensure it stays in sync
            // Extend the vec if needed
            while self.saved_focus_states.len() <= layer_idx {
                self.saved_focus_states.push(None);
            }
            self.saved_focus_states[layer_idx] = focused_id;
            log::debug!(
                "  saved_states now: {:?}",
                self.saved_focus_states
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("[{}]={:?}", i, s))
                    .collect::<Vec<_>>()
            );
        }
    }

    /// Get the focused element ID from the active layer
    pub fn get_layer_focus(&self) -> Option<FocusId> {
        self.layers.last()?.focused_id.clone()
    }

    /// Restore focus from the deepest layer that has a focused element
    /// Used when layers are removed to restore previous layer's focus
    /// Only returns IDs that actually exist in the current active layer's focusables
    pub fn restore_focus_from_layers(&self) -> Option<FocusId> {
        log::debug!(
            "FocusRegistry::restore_focus_from_layers() - searching {} layers",
            self.layers.len()
        );
        for layer in self.layers.iter().rev() {
            log::debug!(
                "  layer {}: focused_id = {:?}",
                layer.layer_index,
                layer.focused_id
            );
        }

        // Get active layer's focusable IDs
        let valid_ids: Vec<FocusId> = self.focusable_ids_in_active_layer();
        log::debug!("  valid IDs in active layer: {:?}", valid_ids);

        // Search from deepest to shallowest layer for a focused element that STILL EXISTS
        let result = self.layers.iter().rev().find_map(|layer| {
            layer.focused_id.as_ref().and_then(|id| {
                // Only return this ID if it exists in current focusables
                if valid_ids.contains(id) {
                    log::debug!("  Found valid saved focus: {:?}", id);
                    Some(id.clone())
                } else {
                    log::debug!(
                        "  Skipping stale saved focus: {:?} (not in current focusables)",
                        id
                    );
                    None
                }
            })
        });

        log::debug!("  -> restored focus: {:?}", result);
        result
    }
}
