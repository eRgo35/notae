#![forbid(unsafe_code)]
//! Platform-independent accessibility snapshot of the Slint window.
//!
//! Slint's Android backend does not publish its accessibility tree to Android, so the app
//! bridges it itself. This module is the host-testable half: it walks the item tree through
//! `i-slint-core`'s public accessibility API (the same calls the desktop AccessKit adapter
//! uses), produces a flat [`Snapshot`], encodes it for the Java provider
//! (`NotaeAccessibility.java`) and routes Android actions back to Slint items.
//!
//! Threading: everything here touches Slint items and must run on the Slint event-loop
//! thread. `crate::android::accessibility` owns the thread hops.
//!
//! The wire layout below is mirrored by constants in `NotaeAccessibility.java`; change both
//! together and bump [`WIRE_VERSION`].

use std::collections::HashMap;

use i_slint_core::accessibility::{
    AccessibilityAction, AccessibleStringProperty, SupportedAccessibilityAction,
    accessible_descendents, find_text_input, find_text_input_with_rc, nearest_accessible_item,
};
use i_slint_core::input::FocusReason;
use i_slint_core::item_tree::{ItemTreeRc, ItemTreeRef, ItemWeak, ParentItemTraversalMode};
use i_slint_core::items::{AccessibleRole, InputType, ItemRc, TextInput};
use i_slint_core::lengths::{LogicalPoint, ScaleFactor};
use i_slint_core::window::{PopupWindowLocation, WindowInner};

/// Bumped whenever the layout of [`Snapshot::encode`] changes.
pub const WIRE_VERSION: i32 = 1;
/// Number of header ints before the first node record.
pub const WIRE_HEADER_INTS: usize = 3;
/// Ints per node record.
pub const WIRE_NODE_INTS: usize = 15;
/// Strings per node record: label, value, description, hint, author id.
pub const WIRE_NODE_STRINGS: usize = 5;
/// Parent id that stands for the Android host view.
pub const HOST_PARENT: i32 = -1;
/// Hard cap that keeps a runaway tree from exhausting memory or the JNI local-reference table.
pub const MAX_NODES: usize = 4000;
const MAX_DEPTH: usize = 96;

pub const FLAG_ENABLED: i32 = 1 << 0;
pub const FLAG_FOCUSED: i32 = 1 << 1;
pub const FLAG_CHECKABLE: i32 = 1 << 2;
pub const FLAG_CHECKED: i32 = 1 << 3;
pub const FLAG_EXPANDABLE: i32 = 1 << 4;
pub const FLAG_EXPANDED: i32 = 1 << 5;
pub const FLAG_CLICK: i32 = 1 << 6;
pub const FLAG_SET_VALUE: i32 = 1 << 7;
pub const FLAG_INCREMENT: i32 = 1 << 8;
pub const FLAG_DECREMENT: i32 = 1 << 9;
pub const FLAG_EXPAND: i32 = 1 << 10;
pub const FLAG_FOCUSABLE: i32 = 1 << 11;
pub const FLAG_EDITABLE: i32 = 1 << 12;
pub const FLAG_MULTILINE: i32 = 1 << 13;
pub const FLAG_PASSWORD: i32 = 1 << 14;
pub const FLAG_HAS_RANGE: i32 = 1 << 15;
pub const FLAG_HAS_SELECTION: i32 = 1 << 16;
pub const FLAG_SELECTED: i32 = 1 << 17;
pub const FLAG_READ_ONLY: i32 = 1 << 18;

/// Android-facing role. The numeric values are part of the wire format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Role {
    Generic = 0,
    Button = 1,
    CheckBox = 2,
    ComboBox = 3,
    Group = 4,
    List = 5,
    Slider = 6,
    SpinButton = 7,
    Tab = 8,
    TabList = 9,
    TabPanel = 10,
    Text = 11,
    Table = 12,
    Tree = 13,
    TextInput = 14,
    Progress = 15,
    Switch = 16,
    ListItem = 17,
    Image = 18,
    RadioButton = 19,
    RadioGroup = 20,
    Pane = 21,
}

impl Role {
    fn from_slint(role: AccessibleRole) -> Self {
        match role {
            AccessibleRole::Button => Role::Button,
            AccessibleRole::Checkbox => Role::CheckBox,
            AccessibleRole::Combobox => Role::ComboBox,
            AccessibleRole::Groupbox => Role::Group,
            AccessibleRole::List => Role::List,
            AccessibleRole::Slider => Role::Slider,
            AccessibleRole::Spinbox => Role::SpinButton,
            AccessibleRole::Tab => Role::Tab,
            AccessibleRole::TabList => Role::TabList,
            AccessibleRole::TabPanel => Role::TabPanel,
            AccessibleRole::Text => Role::Text,
            AccessibleRole::Table => Role::Table,
            AccessibleRole::Tree => Role::Tree,
            AccessibleRole::TextInput => Role::TextInput,
            AccessibleRole::ProgressIndicator => Role::Progress,
            AccessibleRole::Switch => Role::Switch,
            AccessibleRole::ListItem => Role::ListItem,
            AccessibleRole::Image => Role::Image,
            AccessibleRole::RadioButton => Role::RadioButton,
            AccessibleRole::RadioGroup => Role::RadioGroup,
            AccessibleRole::WindowTitleBar
            | AccessibleRole::Banner
            | AccessibleRole::Complementary
            | AccessibleRole::ContentInfo
            | AccessibleRole::Form
            | AccessibleRole::Main
            | AccessibleRole::Navigation
            | AccessibleRole::Region
            | AccessibleRole::Search => Role::Pane,
            _ => Role::Generic,
        }
    }
}

/// One exposed accessibility node. Bounds are in physical pixels relative to the Slint surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub id: i32,
    /// Id of the enclosing exposed node, or [`HOST_PARENT`].
    pub parent: i32,
    pub role: Role,
    pub flags: i32,
    /// `[left, top, right, bottom]`.
    pub bounds: [i32; 4],
    pub label: String,
    pub value: String,
    pub description: String,
    pub hint: String,
    pub author_id: String,
    /// UTF-16 selection of an editable node.
    pub selection: Option<(i32, i32)>,
    /// `(current, min, max)` of a range node.
    pub range: Option<(f32, f32, f32)>,
    /// `(index, count)` of a collection item.
    pub item: Option<(i32, i32)>,
}

impl Node {
    pub fn has(&self, flag: i32) -> bool {
        self.flags & flag != 0
    }
}

/// The exposed tree in pre-order (parents before their children, siblings in tree order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub nodes: Vec<Node>,
    /// Node carrying Slint's input focus, if exposed.
    pub focused: Option<i32>,
}

/// The ints and strings handed to Java in one JNI call each.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wire {
    pub ints: Vec<i32>,
    pub strings: Vec<String>,
}

impl Snapshot {
    pub fn encode(&self) -> Wire {
        let mut ints = Vec::with_capacity(WIRE_HEADER_INTS + self.nodes.len() * WIRE_NODE_INTS);
        let mut strings = Vec::with_capacity(self.nodes.len() * WIRE_NODE_STRINGS);
        ints.push(WIRE_VERSION);
        ints.push(self.nodes.len() as i32);
        ints.push(self.focused.unwrap_or(-1));
        for node in &self.nodes {
            let (selection_start, selection_end) = node.selection.unwrap_or((-1, -1));
            let (current, min, max) = node.range.unwrap_or((0.0, 0.0, 0.0));
            let (item_index, item_count) = node.item.unwrap_or((-1, -1));
            ints.extend_from_slice(&[
                node.id,
                node.parent,
                node.role as i32,
                node.flags,
                node.bounds[0],
                node.bounds[1],
                node.bounds[2],
                node.bounds[3],
                selection_start,
                selection_end,
                current.to_bits() as i32,
                min.to_bits() as i32,
                max.to_bits() as i32,
                item_index,
                item_count,
            ]);
            strings.push(node.label.clone());
            strings.push(node.value.clone());
            strings.push(node.description.clone());
            strings.push(node.hint.clone());
            strings.push(node.author_id.clone());
        }
        Wire { ints, strings }
    }

    pub fn node(&self, id: i32) -> Option<&Node> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

/// Stable node ids and the id -> Slint item map for the latest snapshot.
///
/// Ids are never reused: a repeater item that disappears and a new one that takes the same
/// slot get different ids, so Android never treats them as the same node. Items are held
/// weakly so the registry does not extend the life of destroyed components.
#[derive(Default)]
pub struct Registry {
    next_id: i32,
    ids: HashMap<(usize, u32), i32>,
    items: HashMap<i32, ItemWeak>,
}

impl Registry {
    fn key(item: &ItemRc) -> (usize, u32) {
        let tree = ItemTreeRef::as_ptr(ItemTreeRc::borrow(item.item_tree()));
        (tree.as_ptr() as usize, item.index())
    }

    fn id_for(
        &mut self,
        item: &ItemRc,
        seen: &mut HashMap<(usize, u32), i32>,
        items: &mut HashMap<i32, ItemWeak>,
    ) -> i32 {
        let key = Self::key(item);
        let id = match self.ids.get(&key) {
            Some(id) => *id,
            None => {
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1).max(0);
                id
            }
        };
        seen.insert(key, id);
        items.insert(id, item.downgrade());
        id
    }

    /// The live Slint item behind `id` in the latest snapshot.
    pub fn item(&self, id: i32) -> Option<ItemRc> {
        self.items.get(&id)?.upgrade()
    }
}

struct Walk<'a> {
    registry: &'a mut Registry,
    seen: HashMap<(usize, u32), i32>,
    items: HashMap<i32, ItemWeak>,
    nodes: Vec<Node>,
    scale: ScaleFactor,
    focus: Option<ItemRc>,
    popups: Vec<Popup>,
    focused: Option<i32>,
}

struct Popup {
    location: LogicalPoint,
    parent: ItemRc,
    component: ItemTreeRc,
}

/// Builds the snapshot of `window`'s item tree and refreshes `registry`.
///
/// Reading Slint properties here registers them with the caller's active `PropertyTracker`,
/// which is how the Android controller learns that the tree has to be rebuilt.
pub fn build(window: &slint::Window, registry: &mut Registry) -> Snapshot {
    let inner = WindowInner::from_pub(window);
    inner.ensure_tree_instantiated();
    let Some(component) = inner.try_component() else {
        registry.ids.clear();
        registry.items.clear();
        return Snapshot::default();
    };
    let root = ItemRc::new_root(component);
    let popups = inner
        .active_popups()
        .iter()
        .filter_map(|popup| {
            let PopupWindowLocation::ChildWindow(location) = popup.location else {
                return None;
            };
            Some(Popup {
                location,
                parent: nearest_accessible_item(popup.parent_item.upgrade()?),
                component: popup.component.clone(),
            })
        })
        .collect();
    let focus = inner
        .focus_item
        .borrow()
        .upgrade()
        .map(nearest_accessible_item);

    let mut walk = Walk {
        registry,
        seen: HashMap::new(),
        items: HashMap::new(),
        nodes: Vec::new(),
        scale: ScaleFactor::new(window.scale_factor()),
        focus,
        popups,
        focused: None,
    };
    walk.children_of(&root, HOST_PARENT, LogicalPoint::default(), 0);
    let Walk {
        registry,
        seen,
        items,
        nodes,
        focused,
        ..
    } = walk;
    registry.ids = seen;
    registry.items = items;
    Snapshot { nodes, focused }
}

impl Walk<'_> {
    fn children_of(&mut self, item: &ItemRc, parent: i32, offset: LogicalPoint, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let popup_roots: Vec<(LogicalPoint, ItemRc)> = self
            .popups
            .iter()
            .filter(|popup| popup.parent == *item)
            .map(|popup| (popup.location, ItemRc::new_root(popup.component.clone())))
            .collect();
        for child in accessible_descendents(item) {
            if self.nodes.len() >= MAX_NODES {
                return;
            }
            self.visit(&child, parent, offset, depth + 1);
        }
        for (location, popup_root) in popup_roots {
            self.children_of(&popup_root, parent, location, depth + 1);
        }
    }

    fn visit(&mut self, item: &ItemRc, parent: i32, offset: LogicalPoint, depth: usize) {
        if !item.is_visible() {
            return;
        }
        let node = self.node_for(item, parent, offset);
        let child_parent = match node {
            Some(node) => {
                let id = node.id;
                if node.has(FLAG_FOCUSED) && self.focused.is_none() {
                    self.focused = Some(id);
                }
                self.nodes.push(node);
                id
            }
            None => parent,
        };
        self.children_of(item, child_parent, offset, depth);
    }

    /// Builds the node for `item`, or `None` for items that only group (`accessible-role: none`
    /// without text or actions): their accessible descendants join the enclosing node.
    fn node_for(&mut self, item: &ItemRc, parent: i32, offset: LogicalPoint) -> Option<Node> {
        if is_wrapped_text_input(item) {
            return None;
        }
        let text_input = find_text_input_with_rc(item);
        let slint_role = item.accessible_role();
        let mut role = Role::from_slint(slint_role);
        let supported = item.supported_accessibility_actions();
        let property = |p: AccessibleStringProperty| {
            item.accessible_string_property(p)
                .map(|s| s.to_string())
                .unwrap_or_default()
        };
        let truthy = |p: AccessibleStringProperty| {
            item.accessible_string_property(p)
                .is_some_and(|s| s == "true")
        };

        let label = property(AccessibleStringProperty::Label);
        let mut value = property(AccessibleStringProperty::Value);
        // Slint strings often build descriptions as `part + ". more"`, which leaves a dangling
        // separator when the first part is empty.
        let description = property(AccessibleStringProperty::Description)
            .trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':'))
            .to_string();
        let hint = property(AccessibleStringProperty::PlaceholderText);
        let author_id = property(AccessibleStringProperty::Id);

        let mut flags = 0;
        if !item
            .accessible_string_property(AccessibleStringProperty::Enabled)
            .is_some_and(|s| s != "true")
        {
            flags |= FLAG_ENABLED;
        }
        if supported.contains(SupportedAccessibilityAction::Default) {
            flags |= FLAG_CLICK;
        }
        if supported.contains(SupportedAccessibilityAction::SetValue) {
            flags |= FLAG_SET_VALUE;
        }
        if supported.contains(SupportedAccessibilityAction::Increment) {
            flags |= FLAG_INCREMENT;
        }
        if supported.contains(SupportedAccessibilityAction::Decrement) {
            flags |= FLAG_DECREMENT;
        }
        if supported.contains(SupportedAccessibilityAction::Expand) {
            flags |= FLAG_EXPAND;
        }
        if truthy(AccessibleStringProperty::Checkable) {
            flags |= FLAG_CHECKABLE;
            if truthy(AccessibleStringProperty::Checked) {
                flags |= FLAG_CHECKED;
            }
        }
        if truthy(AccessibleStringProperty::Expandable) {
            flags |= FLAG_EXPANDABLE;
            if truthy(AccessibleStringProperty::Expanded) {
                flags |= FLAG_EXPANDED;
            }
        }
        if truthy(AccessibleStringProperty::ItemSelected) {
            flags |= FLAG_SELECTED;
        }
        if truthy(AccessibleStringProperty::ReadOnly) {
            flags |= FLAG_READ_ONLY;
        }

        let mut selection = None;
        if role == Role::TextInput {
            flags |= FLAG_EDITABLE | FLAG_FOCUSABLE;
            if let Some((_, input)) = &text_input {
                if !input.single_line.get_internal() {
                    flags |= FLAG_MULTILINE;
                }
                if input.input_type.get_internal() == InputType::Password {
                    flags |= FLAG_PASSWORD;
                }
                if input.has_focus.get_internal() {
                    flags |= FLAG_FOCUSED;
                }
                if value.is_empty() {
                    value = input.as_pin_ref().text().to_string();
                }
                let (start, end) = input.as_pin_ref().selection_anchor_and_cursor();
                selection = Some((utf16_index(&value, start), utf16_index(&value, end)));
                flags |= FLAG_HAS_SELECTION;
            }
            if flags & FLAG_PASSWORD != 0 {
                value = "\u{2022}".repeat(value.chars().count());
            }
        } else if matches!(role, Role::Slider | Role::SpinButton | Role::ComboBox) {
            flags |= FLAG_FOCUSABLE;
        }
        if self.focus.as_ref() == Some(item) {
            flags |= FLAG_FOCUSED;
        }

        let range = {
            let number = |p: AccessibleStringProperty| {
                item.accessible_string_property(p)
                    .and_then(|s| s.parse::<f32>().ok())
            };
            match (
                number(AccessibleStringProperty::Value),
                number(AccessibleStringProperty::ValueMinimum),
                number(AccessibleStringProperty::ValueMaximum),
            ) {
                (current, Some(min), Some(max)) if max > min => {
                    flags |= FLAG_HAS_RANGE;
                    Some((current.unwrap_or(min), min, max))
                }
                _ => None,
            }
        };
        if range.is_some() && matches!(role, Role::Progress | Role::Slider | Role::SpinButton) {
            // The numeric value is the range position, not text to read as a label.
            value.clear();
        }

        let item_position = {
            let number = |p: AccessibleStringProperty| {
                item.accessible_string_property(p)
                    .and_then(|s| s.parse::<i32>().ok())
            };
            match (
                number(AccessibleStringProperty::ItemIndex),
                number(AccessibleStringProperty::ItemCount),
            ) {
                (Some(index), Some(count)) => Some((index, count)),
                _ => None,
            }
        };

        let carries_meaning = role != Role::Generic
            || !label.is_empty()
            || !value.is_empty()
            || !description.is_empty()
            || flags
                & (FLAG_CLICK | FLAG_SET_VALUE | FLAG_INCREMENT | FLAG_DECREMENT | FLAG_EXPAND)
                != 0;
        if !carries_meaning {
            return None;
        }
        if role == Role::Generic && flags & FLAG_CLICK != 0 {
            role = Role::Button;
        }

        let geometry = item.geometry();
        let origin = item.map_to_window(geometry.origin) + offset.to_vector();
        let left = (origin.x * self.scale.get()).round() as i32;
        let top = (origin.y * self.scale.get()).round() as i32;
        let right = ((origin.x + geometry.size.width) * self.scale.get()).round() as i32;
        let bottom = ((origin.y + geometry.size.height) * self.scale.get()).round() as i32;

        let id = self.registry.id_for(item, &mut self.seen, &mut self.items);
        Some(Node {
            id,
            parent,
            role,
            flags,
            bounds: [left, top, right, bottom],
            label,
            value,
            description,
            hint,
            author_id,
            selection,
            range,
            item: item_position,
        })
    }
}

/// Whether `item` is the built-in `TextInput` that an enclosing accessible text-input wrapper
/// (a `LineEdit`-style component) already exposes. Android gets one editable node for the pair.
fn is_wrapped_text_input(item: &ItemRc) -> bool {
    if item.downcast::<TextInput>().is_none() {
        return false;
    }
    let Some(parent) = item.parent_item(ParentItemTraversalMode::StopAtPopups) else {
        return false;
    };
    let wrapper = nearest_accessible_item(parent);
    wrapper != *item
        && wrapper.is_accessible()
        && wrapper.accessible_role() == AccessibleRole::TextInput
        && find_text_input_with_rc(&wrapper).is_some_and(|(input, _)| input == *item)
}

/// UTF-16 offset of the UTF-8 byte offset `byte` in `text` (clamped to the text).
pub fn utf16_index(text: &str, byte: usize) -> i32 {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    text[..byte].encode_utf16().count() as i32
}

/// UTF-8 byte offset of the UTF-16 offset `index` in `text` (clamped to the text).
pub fn utf8_offset(text: &str, index: i32) -> usize {
    let mut remaining = index.max(0) as usize;
    for (byte, ch) in text.char_indices() {
        let units = ch.len_utf16();
        if remaining < units {
            return byte;
        }
        remaining -= units;
    }
    text.len()
}

/// An action Android asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Click,
    /// Move Slint's input focus to the node.
    Focus,
    SetText(String),
    /// UTF-16 selection of an editable node.
    SetSelection(i32, i32),
    Increment,
    Decrement,
    Expand,
}

/// Why an action was not performed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The node is gone: the tree changed since Android last read it.
    StaleNode,
    /// The node does not support the action.
    Unsupported,
}

/// Runs `action` on the Slint item behind `id`.
pub fn perform(
    window: &slint::Window,
    registry: &Registry,
    id: i32,
    action: &Action,
) -> Result<(), Refusal> {
    let item = registry.item(id).ok_or(Refusal::StaleNode)?;
    let supported = item.supported_accessibility_actions();
    let focus = |target: &ItemRc| {
        WindowInner::from_pub(window).set_focus_item(target, true, FocusReason::PointerClick);
    };
    match action {
        Action::Click if supported.contains(SupportedAccessibilityAction::Default) => {
            item.accessible_action(&AccessibilityAction::Default);
        }
        // Double tap on a text field: Android sends a click; the field needs focus (and with
        // it the on-screen keyboard) rather than an activation.
        Action::Click | Action::Focus => {
            let (target, _) = find_text_input_with_rc(&item).ok_or(Refusal::Unsupported)?;
            focus(&target);
        }
        Action::SetText(text) if supported.contains(SupportedAccessibilityAction::SetValue) => {
            item.accessible_action(&AccessibilityAction::SetValue(text.as_str().into()));
        }
        Action::SetSelection(start, end)
            if supported.contains(SupportedAccessibilityAction::SetSelectionOffsets) =>
        {
            let text = item
                .accessible_string_property(AccessibleStringProperty::Value)
                .map(|s| s.to_string())
                .or_else(|| {
                    find_text_input(&item).map(|input| input.as_pin_ref().text().to_string())
                })
                .unwrap_or_default();
            item.accessible_action(&AccessibilityAction::SetSelectionOffsets(
                utf8_offset(&text, *start) as i32,
                utf8_offset(&text, *end) as i32,
            ));
        }
        Action::Increment if supported.contains(SupportedAccessibilityAction::Increment) => {
            item.accessible_action(&AccessibilityAction::Increment);
        }
        Action::Decrement if supported.contains(SupportedAccessibilityAction::Decrement) => {
            item.accessible_action(&AccessibilityAction::Decrement);
        }
        Action::Expand if supported.contains(SupportedAccessibilityAction::Expand) => {
            item.accessible_action(&AccessibilityAction::Expand);
        }
        _ => return Err(Refusal::Unsupported),
    }
    Ok(())
}
