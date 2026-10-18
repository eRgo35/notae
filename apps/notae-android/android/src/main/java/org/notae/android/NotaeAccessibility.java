package org.notae.android;

import android.app.Activity;
import android.graphics.Rect;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.util.SparseIntArray;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityNodeProvider;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/**
 * Publishes the Slint accessibility tree to Android.
 *
 * Slint's Android backend draws everything into one native surface and publishes no
 * accessibility tree, so TalkBack and uiautomator would see a single unlabelled view. This class
 * installs an {@link AccessibilityNodeProvider} on the NativeActivity's content view and answers
 * it from an immutable snapshot that the Rust side (`src/accessibility.rs`) pushes whenever the
 * Slint tree changes.
 *
 * Threads: Slint lives on the native `android_main` thread; Android calls the provider on the UI
 * thread. The provider therefore never calls into Slint synchronously: it reads the last
 * snapshot, and actions are posted to the Slint thread through {@link #nativePerformAction}.
 *
 * The wire layout is mirrored by `src/accessibility.rs`; change both together.
 */
final class NotaeAccessibility extends View.AccessibilityDelegate {
    private static final int WIRE_VERSION = 1;
    private static final int HEADER_INTS = 3;
    private static final int NODE_INTS = 15;
    private static final int NODE_STRINGS = 5;

    private static final int FLAG_ENABLED = 1;
    private static final int FLAG_FOCUSED = 1 << 1;
    private static final int FLAG_CHECKABLE = 1 << 2;
    private static final int FLAG_CHECKED = 1 << 3;
    private static final int FLAG_EXPANDABLE = 1 << 4;
    private static final int FLAG_EXPANDED = 1 << 5;
    private static final int FLAG_CLICK = 1 << 6;
    private static final int FLAG_SET_VALUE = 1 << 7;
    private static final int FLAG_INCREMENT = 1 << 8;
    private static final int FLAG_DECREMENT = 1 << 9;
    private static final int FLAG_EXPAND = 1 << 10;
    private static final int FLAG_FOCUSABLE = 1 << 11;
    private static final int FLAG_EDITABLE = 1 << 12;
    private static final int FLAG_MULTILINE = 1 << 13;
    private static final int FLAG_PASSWORD = 1 << 14;
    private static final int FLAG_HAS_RANGE = 1 << 15;
    private static final int FLAG_HAS_SELECTION = 1 << 16;
    private static final int FLAG_SELECTED = 1 << 17;
    private static final int FLAG_READ_ONLY = 1 << 18;

    private static final int ROLE_BUTTON = 1;
    private static final int ROLE_CHECKBOX = 2;
    private static final int ROLE_COMBOBOX = 3;
    private static final int ROLE_GROUP = 4;
    private static final int ROLE_LIST = 5;
    private static final int ROLE_SLIDER = 6;
    private static final int ROLE_SPINBUTTON = 7;
    private static final int ROLE_TAB = 8;
    private static final int ROLE_TABLIST = 9;
    private static final int ROLE_TABPANEL = 10;
    private static final int ROLE_TEXT = 11;
    private static final int ROLE_TABLE = 12;
    private static final int ROLE_TREE = 13;
    private static final int ROLE_TEXTINPUT = 14;
    private static final int ROLE_PROGRESS = 15;
    private static final int ROLE_SWITCH = 16;
    private static final int ROLE_LISTITEM = 17;
    private static final int ROLE_IMAGE = 18;
    private static final int ROLE_RADIOBUTTON = 19;
    private static final int ROLE_RADIOGROUP = 20;
    private static final int ROLE_PANE = 21;

    // Action kinds sent to Rust; mirrored by `android/accessibility.rs`.
    private static final int KIND_CLICK = 0;
    private static final int KIND_FOCUS = 1;
    private static final int KIND_SET_TEXT = 2;
    private static final int KIND_SET_SELECTION = 3;
    private static final int KIND_INCREMENT = 4;
    private static final int KIND_DECREMENT = 5;
    private static final int KIND_EXPAND = 6;

    private static final long FIRST_SNAPSHOT_WAIT_MS = 400;
    // A snapshot that keeps fewer than this share of the previous node ids is a new screen.
    private static final int SCREEN_CHANGE_KEPT_PERCENT = 40;
    private static final int TITLE_SEARCH_NODES = 3;

    private static final String TAG = "NotaeA11y";

    /**
     * The bridge is optional: whatever goes wrong here (a missing native symbol, a framework
     * quirk, a bug) is logged and swallowed so it can never stop the app from starting or
     * crash it. Every entry point Android or Rust can call is wrapped with this.
     */
    private static void failOpen(String where, Throwable error) {
        try {
            Log.e(TAG, where + " failed; continuing without it", error);
        } catch (Throwable ignored) {
            // Logging must not throw either.
        }
    }

    private static native void nativeSetActive(boolean active);

    private static native void nativePerformAction(
        int id, int kind, String text, int first, int second);

    private static final class Node {
        int id;
        int parent;
        int role;
        int flags;
        final Rect bounds = new Rect();
        String label;
        String value;
        String description;
        String hint;
        String authorId;
        int selectionStart;
        int selectionEnd;
        float rangeCurrent;
        float rangeMin;
        float rangeMax;
        int itemIndex;
        int itemCount;
        final ArrayList<Node> children = new ArrayList<>();

        boolean has(int flag) {
            return (flags & flag) != 0;
        }
    }

    private static final class Snapshot {
        static final Snapshot EMPTY = new Snapshot(new Node[0], new ArrayList<Node>(), -1);
        final Node[] nodes;
        final ArrayList<Node> roots;
        final int focusedId;
        private final SparseIntArray indexById = new SparseIntArray();

        Snapshot(Node[] nodes, ArrayList<Node> roots, int focusedId) {
            this.nodes = nodes;
            this.roots = roots;
            this.focusedId = focusedId;
            for (int i = 0; i < nodes.length; i++) {
                indexById.put(nodes[i].id, i);
            }
        }

        Node byId(int id) {
            int index = indexById.get(id, -1);
            return index < 0 ? null : nodes[index];
        }
    }

    private final Activity activity;
    private final View host;
    private final AccessibilityManager manager;
    private final CountDownLatch firstSnapshot = new CountDownLatch(1);
    private volatile Snapshot current = Snapshot.EMPTY;
    private volatile boolean installed;

    // UI-thread state.
    private Snapshot delivered = Snapshot.EMPTY;
    private int accessibilityFocusedId = View.NO_ID;
    private int hoveredId = View.NO_ID;
    private boolean contentChangePosted;
    private boolean active;
    private boolean waitedForFirstSnapshot;

    private final AccessibilityManager.AccessibilityStateChangeListener stateListener =
        new AccessibilityManager.AccessibilityStateChangeListener() {
            @Override
            public void onAccessibilityStateChanged(boolean enabled) {
                try {
                    setActive(enabled);
                } catch (Throwable error) {
                    failOpen("onAccessibilityStateChanged", error);
                }
            }
        };

    private final AccessibilityNodeProvider provider = new AccessibilityNodeProvider() {
        @Override
        public AccessibilityNodeInfo createAccessibilityNodeInfo(int virtualViewId) {
            try {
                activateOnFirstQuery();
                if (virtualViewId == HOST_VIEW_ID) {
                    return createHostInfo();
                }
                Node node = current.byId(virtualViewId);
                return node == null ? null : createNodeInfo(node);
            } catch (Throwable error) {
                failOpen("createAccessibilityNodeInfo", error);
                return null;
            }
        }

        @Override
        public boolean performAction(int virtualViewId, int action, Bundle arguments) {
            try {
                if (virtualViewId == HOST_VIEW_ID) {
                    return NotaeAccessibility.super.performAccessibilityAction(
                        host, action, arguments);
                }
                Node node = current.byId(virtualViewId);
                return node != null && performNodeAction(node, action, arguments);
            } catch (Throwable error) {
                failOpen("performAction", error);
                return false;
            }
        }

        @Override
        public List<AccessibilityNodeInfo> findAccessibilityNodeInfosByText(
                String text, int virtualViewId) {
            ArrayList<AccessibilityNodeInfo> found = new ArrayList<>();
            try {
                if (text == null || text.isEmpty()) {
                    return found;
                }
                String needle = text.toLowerCase();
                for (Node node : current.nodes) {
                    if (spoken(node).toLowerCase().contains(needle)) {
                        found.add(createNodeInfo(node));
                    }
                }
            } catch (Throwable error) {
                failOpen("findAccessibilityNodeInfosByText", error);
            }
            return found;
        }

        @Override
        public AccessibilityNodeInfo findFocus(int focus) {
            try {
                if (focus == AccessibilityNodeInfo.FOCUS_ACCESSIBILITY) {
                    Node node = current.byId(accessibilityFocusedId);
                    return node == null ? null : createNodeInfo(node);
                }
                Node node = current.byId(current.focusedId);
                return node == null ? null : createNodeInfo(node);
            } catch (Throwable error) {
                failOpen("findFocus", error);
                return null;
            }
        }
    };

    private NotaeAccessibility(Activity activity, View host) {
        this.activity = activity;
        this.host = host;
        this.manager = (AccessibilityManager) activity.getSystemService(
            Activity.ACCESSIBILITY_SERVICE);
    }

    /** Installs the bridge on the activity's content view; call after `super.onCreate`. */
    static NotaeAccessibility install(Activity activity) {
        NotaeAccessibility bridge = null;
        try {
            bridge = installUnguarded(activity);
            return bridge;
        } catch (Throwable error) {
            failOpen("install", error);
            if (bridge != null) {
                bridge.uninstall();
            }
            return null;
        }
    }

    private static NotaeAccessibility installUnguarded(Activity activity) {
        ViewGroup content = activity.findViewById(android.R.id.content);
        View host = content != null && content.getChildCount() > 0 ? content.getChildAt(0) : null;
        if (host == null) {
            return null;
        }
        NotaeAccessibility bridge = new NotaeAccessibility(activity, host);
        bridge.installed = true;
        host.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        host.setAccessibilityDelegate(bridge);
        host.setOnHoverListener(bridge::onHover);
        bridge.hideInputOverlays(content, host);
        if (bridge.manager != null) {
            bridge.manager.addAccessibilityStateChangeListener(bridge.stateListener);
            bridge.setActive(bridge.manager.isEnabled());
        }
        return bridge;
    }

    /**
     * Slint's soft-keyboard bridge adds an invisible view next to the content view. It only
     * exists for the input method, so a screen reader must not offer it as an unlabelled stop.
     */
    private void hideInputOverlays(ViewGroup content, View host) {
        for (int i = 0; i < content.getChildCount(); i++) {
            View child = content.getChildAt(i);
            if (child != host) {
                child.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
            }
        }
        content.setOnHierarchyChangeListener(new ViewGroup.OnHierarchyChangeListener() {
            @Override
            public void onChildViewAdded(View parent, View child) {
                try {
                    if (child != host) {
                        child.setImportantForAccessibility(
                            View.IMPORTANT_FOR_ACCESSIBILITY_NO);
                    }
                } catch (Throwable error) {
                    failOpen("onChildViewAdded", error);
                }
            }

            @Override
            public void onChildViewRemoved(View parent, View child) {
            }
        });
    }

    /** Detaches the bridge; the snapshot is dropped and Slint stops tracking the tree. */
    void uninstall() {
        try {
            uninstallUnguarded();
        } catch (Throwable error) {
            failOpen("uninstall", error);
        }
    }

    private void uninstallUnguarded() {
        installed = false;
        if (manager != null) {
            manager.removeAccessibilityStateChangeListener(stateListener);
        }
        host.setAccessibilityDelegate(null);
        host.setOnHoverListener(null);
        if (host.getParent() instanceof ViewGroup) {
            ((ViewGroup) host.getParent()).setOnHierarchyChangeListener(null);
        }
        setActive(false);
        current = Snapshot.EMPTY;
        delivered = Snapshot.EMPTY;
    }

    private void setActive(boolean enabled) {
        if (active == enabled) {
            return;
        }
        active = enabled;
        nativeSetActive(enabled);
    }

    /**
     * An accessibility client that connects without toggling the global state (for example a
     * test harness) still queries the provider; that starts tracking, and the first query waits
     * briefly for the first snapshot so it does not see an empty window.
     */
    private void activateOnFirstQuery() {
        if (!active) {
            setActive(true);
        }
        if (current == Snapshot.EMPTY && installed && !waitedForFirstSnapshot) {
            waitedForFirstSnapshot = true;
            try {
                firstSnapshot.await(FIRST_SNAPSHOT_WAIT_MS, TimeUnit.MILLISECONDS);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
            }
        }
    }

    @Override
    public AccessibilityNodeProvider getAccessibilityNodeProvider(View view) {
        return provider;
    }

    // ---- Snapshot intake (Slint thread) ------------------------------------------------------

    /** Receives one encoded snapshot from Rust (see `Snapshot::encode`). */
    void publish(int[] ints, String[] strings) {
        try {
            publishUnguarded(ints, strings);
        } catch (Throwable error) {
            failOpen("publish", error);
        }
    }

    private void publishUnguarded(int[] ints, String[] strings) {
        if (!installed || ints == null || strings == null || ints.length < HEADER_INTS
                || ints[0] != WIRE_VERSION) {
            return;
        }
        int count = ints[1];
        if (count < 0 || ints.length != HEADER_INTS + count * NODE_INTS
                || strings.length != count * NODE_STRINGS) {
            return;
        }
        Node[] nodes = new Node[count];
        ArrayList<Node> roots = new ArrayList<>();
        SparseIntArray byId = new SparseIntArray(count);
        for (int i = 0; i < count; i++) {
            int base = HEADER_INTS + i * NODE_INTS;
            Node node = new Node();
            node.id = ints[base];
            node.parent = ints[base + 1];
            node.role = ints[base + 2];
            node.flags = ints[base + 3];
            node.bounds.set(ints[base + 4], ints[base + 5], ints[base + 6], ints[base + 7]);
            node.selectionStart = ints[base + 8];
            node.selectionEnd = ints[base + 9];
            node.rangeCurrent = Float.intBitsToFloat(ints[base + 10]);
            node.rangeMin = Float.intBitsToFloat(ints[base + 11]);
            node.rangeMax = Float.intBitsToFloat(ints[base + 12]);
            node.itemIndex = ints[base + 13];
            node.itemCount = ints[base + 14];
            node.label = strings[i * NODE_STRINGS];
            node.value = strings[i * NODE_STRINGS + 1];
            node.description = strings[i * NODE_STRINGS + 2];
            node.hint = strings[i * NODE_STRINGS + 3];
            node.authorId = strings[i * NODE_STRINGS + 4];
            nodes[i] = node;
            byId.put(node.id, i);
        }
        for (Node node : nodes) {
            int parentIndex = node.parent < 0 ? -1 : byId.get(node.parent, -1);
            if (parentIndex < 0) {
                roots.add(node);
            } else {
                nodes[parentIndex].children.add(node);
            }
        }
        current = new Snapshot(nodes, roots, ints[2]);
        firstSnapshot.countDown();
        synchronized (this) {
            if (contentChangePosted) {
                return;
            }
            contentChangePosted = true;
        }
        host.post(this::deliverChanges);
    }

    // ---- Change events (UI thread) -----------------------------------------------------------

    private void deliverChanges() {
        try {
            deliverChangesUnguarded();
        } catch (Throwable error) {
            failOpen("deliverChanges", error);
        }
    }

    private void deliverChangesUnguarded() {
        synchronized (this) {
            contentChangePosted = false;
        }
        Snapshot now = current;
        Snapshot before = delivered;
        delivered = now;
        if (!installed || manager == null || !manager.isEnabled()) {
            return;
        }
        if (accessibilityFocusedId != View.NO_ID && now.byId(accessibilityFocusedId) == null) {
            accessibilityFocusedId = View.NO_ID;
        }
        if (hoveredId != View.NO_ID && now.byId(hoveredId) == null) {
            hoveredId = View.NO_ID;
        }
        for (Node node : now.nodes) {
            Node old = before.byId(node.id);
            if (old != null && node.has(FLAG_EDITABLE) && node.has(FLAG_FOCUSED)
                    && !node.has(FLAG_PASSWORD)) {
                sendTextEvents(old, node);
            }
        }
        if (isScreenChange(before, now)) {
            announceScreen(now);
        }
        AccessibilityEvent changed =
            AccessibilityEvent.obtain(AccessibilityEvent.TYPE_WINDOW_CONTENT_CHANGED);
        changed.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_SUBTREE);
        changed.setPackageName(host.getContext().getPackageName());
        changed.setClassName(host.getClass().getName());
        changed.setSource(host);
        send(changed);
    }

    /**
     * The whole UI is one window, so a page switch is the app's "new activity": most nodes
     * are replaced at once. A screen reader needs the window-state event to drop its stale
     * focus and read the new page instead of staying on a node that no longer exists.
     */
    private static boolean isScreenChange(Snapshot before, Snapshot now) {
        if (before.nodes.length < 4 || now.nodes.length == 0) {
            return false;
        }
        int kept = 0;
        for (Node node : before.nodes) {
            if (now.byId(node.id) != null) {
                kept++;
            }
        }
        return kept * 100 < before.nodes.length * SCREEN_CHANGE_KEPT_PERCENT;
    }

    private void announceScreen(Snapshot now) {
        // A page's heading is one of its first nodes (after a back or menu button); a text
        // further down is content, not a title, so the event then carries no text.
        String title = "";
        for (int i = 0; i < Math.min(TITLE_SEARCH_NODES, now.nodes.length); i++) {
            Node node = now.nodes[i];
            if (node.role == ROLE_TEXT && !spoken(node).isEmpty()) {
                title = spoken(node);
                break;
            }
        }
        AccessibilityEvent event =
            AccessibilityEvent.obtain(AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED);
        event.setPackageName(host.getContext().getPackageName());
        event.setClassName(activity.getClass().getName());
        event.setSource(host);
        if (!title.isEmpty()) {
            event.getText().add(title);
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            event.setContentChangeTypes(AccessibilityEvent.CONTENT_CHANGE_TYPE_PANE_APPEARED);
        }
        send(event);
    }

    private void sendTextEvents(Node old, Node node) {
        String before = old.value == null ? "" : old.value;
        String after = node.value == null ? "" : node.value;
        if (!before.equals(after)) {
            int prefix = 0;
            int limit = Math.min(before.length(), after.length());
            while (prefix < limit && before.charAt(prefix) == after.charAt(prefix)) {
                prefix++;
            }
            int suffix = 0;
            while (suffix < limit - prefix
                    && before.charAt(before.length() - 1 - suffix)
                        == after.charAt(after.length() - 1 - suffix)) {
                suffix++;
            }
            AccessibilityEvent event = nodeEvent(node, AccessibilityEvent.TYPE_VIEW_TEXT_CHANGED);
            event.setFromIndex(prefix);
            event.setRemovedCount(before.length() - prefix - suffix);
            event.setAddedCount(after.length() - prefix - suffix);
            event.setBeforeText(before);
            event.getText().clear();
            event.getText().add(after);
            send(event);
        }
        if (node.has(FLAG_HAS_SELECTION)
                && (old.selectionStart != node.selectionStart
                    || old.selectionEnd != node.selectionEnd)) {
            AccessibilityEvent event =
                nodeEvent(node, AccessibilityEvent.TYPE_VIEW_TEXT_SELECTION_CHANGED);
            event.setFromIndex(node.selectionStart);
            event.setToIndex(node.selectionEnd);
            event.setItemCount(after.length());
            event.getText().clear();
            event.getText().add(after);
            send(event);
        }
    }

    private AccessibilityEvent nodeEvent(Node node, int type) {
        AccessibilityEvent event = AccessibilityEvent.obtain(type);
        event.setPackageName(host.getContext().getPackageName());
        event.setClassName(className(node));
        event.setEnabled(node.has(FLAG_ENABLED));
        event.setSource(host, node.id);
        CharSequence text = spoken(node);
        if (text.length() > 0) {
            event.getText().add(text);
        }
        return event;
    }

    private void send(AccessibilityEvent event) {
        if (host.getParent() != null && manager != null && manager.isEnabled()) {
            host.getParent().requestSendAccessibilityEvent(host, event);
        }
    }

    // ---- Hover (explore by touch) --------------------------------------------------------------

    private boolean onHover(View view, MotionEvent event) {
        try {
            return onHoverUnguarded(event);
        } catch (Throwable error) {
            failOpen("onHover", error);
            return false;
        }
    }

    private boolean onHoverUnguarded(MotionEvent event) {
        if (manager == null || !manager.isEnabled() || !manager.isTouchExplorationEnabled()) {
            return false;
        }
        int action = event.getActionMasked();
        if (action == MotionEvent.ACTION_HOVER_ENTER || action == MotionEvent.ACTION_HOVER_MOVE) {
            Node hit = hitTest(Math.round(event.getX()), Math.round(event.getY()));
            int id = hit == null ? View.NO_ID : hit.id;
            if (id == hoveredId) {
                return hit != null;
            }
            if (hoveredId != View.NO_ID) {
                Node previous = current.byId(hoveredId);
                if (previous != null) {
                    send(nodeEvent(previous, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT));
                }
            }
            hoveredId = id;
            if (hit != null) {
                send(nodeEvent(hit, AccessibilityEvent.TYPE_VIEW_HOVER_ENTER));
            }
            return hit != null;
        }
        if (action == MotionEvent.ACTION_HOVER_EXIT && hoveredId != View.NO_ID) {
            Node previous = current.byId(hoveredId);
            hoveredId = View.NO_ID;
            if (previous != null) {
                send(nodeEvent(previous, AccessibilityEvent.TYPE_VIEW_HOVER_EXIT));
                return true;
            }
        }
        return false;
    }

    /** Topmost exposed node at a point in host coordinates: later nodes draw over earlier ones. */
    private Node hitTest(int x, int y) {
        Node[] nodes = current.nodes;
        for (int i = nodes.length - 1; i >= 0; i--) {
            Node node = nodes[i];
            if (node.bounds.contains(x, y) && node.has(FLAG_ENABLED) && isFocusableByTouch(node)) {
                return node;
            }
        }
        return null;
    }

    private static boolean isFocusableByTouch(Node node) {
        return node.has(FLAG_CLICK) || node.has(FLAG_EDITABLE) || !spoken(node).isEmpty();
    }

    // ---- Node construction ---------------------------------------------------------------------

    private AccessibilityNodeInfo createHostInfo() {
        AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain(host);
        host.onInitializeAccessibilityNodeInfo(info);
        for (Node root : current.roots) {
            info.addChild(host, root.id);
        }
        return info;
    }

    // `obtain` is the only constructor on API 23-32; the Rust side targets minSdk 23.
    @SuppressWarnings("deprecation")
    private AccessibilityNodeInfo createNodeInfo(Node node) {
        AccessibilityNodeInfo info = AccessibilityNodeInfo.obtain(host, node.id);
        info.setPackageName(host.getContext().getPackageName());
        info.setClassName(className(node));
        if (node.parent < 0) {
            info.setParent(host);
        } else {
            info.setParent(host, node.parent);
        }
        for (Node child : node.children) {
            info.addChild(host, child.id);
        }

        Rect screen = new Rect(node.bounds);
        int[] origin = new int[2];
        host.getLocationOnScreen(origin);
        screen.offset(origin[0], origin[1]);
        info.setBoundsInScreen(screen);
        info.setBoundsInParent(node.bounds);
        Rect visible = new Rect();
        info.setVisibleToUser(host.getGlobalVisibleRect(visible) && Rect.intersects(visible, screen));

        info.setEnabled(node.has(FLAG_ENABLED));
        info.setImportantForAccessibility(true);
        info.setClickable(node.has(FLAG_CLICK));
        info.setFocusable(node.has(FLAG_FOCUSABLE));
        info.setFocused(node.has(FLAG_FOCUSED) && node.has(FLAG_FOCUSABLE));
        info.setAccessibilityFocused(node.id == accessibilityFocusedId);
        info.setSelected(node.has(FLAG_SELECTED));
        info.setCheckable(node.has(FLAG_CHECKABLE));
        info.setChecked(node.has(FLAG_CHECKED));
        info.setPassword(node.has(FLAG_PASSWORD));
        if (node.authorId != null && !node.authorId.isEmpty()) {
            info.setViewIdResourceName(host.getContext().getPackageName() + ":id/" + node.authorId);
        }

        if (node.role == ROLE_TEXTINPUT) {
            info.setEditable(!node.has(FLAG_READ_ONLY));
            info.setMultiLine(node.has(FLAG_MULTILINE));
            info.setText(node.value == null ? "" : node.value);
            if (node.has(FLAG_HAS_SELECTION)) {
                info.setTextSelection(node.selectionStart, node.selectionEnd);
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                String hint = node.hint != null && !node.hint.isEmpty() ? node.hint : node.label;
                info.setHintText(hint);
            }
            if (node.label != null && !node.label.isEmpty()) {
                info.setContentDescription(node.label);
            }
        } else if (node.role == ROLE_TEXT) {
            info.setText(spoken(node));
        } else {
            info.setContentDescription(spoken(node));
        }
        String roleDescription = roleDescription(node);
        if (roleDescription != null) {
            info.getExtras().putCharSequence("AccessibilityNodeInfo.roleDescription", roleDescription);
        }
        if (node.has(FLAG_EXPANDABLE) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            info.setStateDescription(node.has(FLAG_EXPANDED) ? "Expanded" : "Collapsed");
        }
        if (node.has(FLAG_HAS_RANGE)) {
            info.setRangeInfo(AccessibilityNodeInfo.RangeInfo.obtain(
                AccessibilityNodeInfo.RangeInfo.RANGE_TYPE_FLOAT,
                node.rangeMin, node.rangeMax, node.rangeCurrent));
        }
        if (node.itemCount >= 0 && node.itemIndex >= 0) {
            info.setCollectionItemInfo(AccessibilityNodeInfo.CollectionItemInfo.obtain(
                node.itemIndex, 1, 0, 1, false, node.has(FLAG_SELECTED)));
        }
        if (node.role == ROLE_LIST || node.role == ROLE_TABLE) {
            int rows = 0;
            for (Node child : node.children) {
                if (child.itemCount >= 0) {
                    rows = Math.max(rows, child.itemCount);
                }
            }
            info.setCollectionInfo(AccessibilityNodeInfo.CollectionInfo.obtain(rows, 1, false));
        }

        addActions(info, node);
        return info;
    }

    private void addActions(AccessibilityNodeInfo info, Node node) {
        if (node.id == accessibilityFocusedId) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLEAR_ACCESSIBILITY_FOCUS);
        } else {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_ACCESSIBILITY_FOCUS);
        }
        if (!node.has(FLAG_ENABLED)) {
            return;
        }
        if (node.has(FLAG_CLICK) || node.has(FLAG_EDITABLE)) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_CLICK);
        }
        if (node.has(FLAG_FOCUSABLE)) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_FOCUS);
        }
        if (node.has(FLAG_EDITABLE) && node.has(FLAG_SET_VALUE)) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_TEXT);
        }
        if (node.has(FLAG_EDITABLE) && node.has(FLAG_HAS_SELECTION)) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_SELECTION);
        }
        if (node.has(FLAG_HAS_RANGE) && node.has(FLAG_SET_VALUE)
                && Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS);
        }
        if (node.has(FLAG_INCREMENT)) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_FORWARD);
        }
        if (node.has(FLAG_DECREMENT)) {
            info.addAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SCROLL_BACKWARD);
        }
        if (node.has(FLAG_EXPAND)) {
            info.addAction(node.has(FLAG_EXPANDED)
                ? AccessibilityNodeInfo.AccessibilityAction.ACTION_COLLAPSE
                : AccessibilityNodeInfo.AccessibilityAction.ACTION_EXPAND);
        }
    }

    // ---- Actions (UI thread -> Slint thread) -----------------------------------------------------

    private boolean performNodeAction(Node node, int action, Bundle arguments) {
        switch (action) {
            case AccessibilityNodeInfo.ACTION_ACCESSIBILITY_FOCUS:
                if (accessibilityFocusedId == node.id) {
                    return false;
                }
                accessibilityFocusedId = node.id;
                send(nodeEvent(node, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUSED));
                return true;
            case AccessibilityNodeInfo.ACTION_CLEAR_ACCESSIBILITY_FOCUS:
                if (accessibilityFocusedId != node.id) {
                    return false;
                }
                accessibilityFocusedId = View.NO_ID;
                send(nodeEvent(node, AccessibilityEvent.TYPE_VIEW_ACCESSIBILITY_FOCUS_CLEARED));
                return true;
            default:
                break;
        }
        if (!node.has(FLAG_ENABLED)) {
            return false;
        }
        switch (action) {
            case AccessibilityNodeInfo.ACTION_CLICK:
                if (!node.has(FLAG_CLICK) && !node.has(FLAG_EDITABLE)) {
                    return false;
                }
                nativePerformAction(node.id, KIND_CLICK, null, 0, 0);
                send(nodeEvent(node, AccessibilityEvent.TYPE_VIEW_CLICKED));
                return true;
            case AccessibilityNodeInfo.ACTION_FOCUS:
                if (!node.has(FLAG_FOCUSABLE)) {
                    return false;
                }
                nativePerformAction(node.id, KIND_FOCUS, null, 0, 0);
                return true;
            case AccessibilityNodeInfo.ACTION_SET_TEXT: {
                if (!node.has(FLAG_EDITABLE) || !node.has(FLAG_SET_VALUE) || arguments == null) {
                    return false;
                }
                CharSequence text = arguments.getCharSequence(
                    AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE);
                nativePerformAction(node.id, KIND_SET_TEXT, text == null ? "" : text.toString(), 0, 0);
                return true;
            }
            case AccessibilityNodeInfo.ACTION_SET_SELECTION: {
                if (!node.has(FLAG_EDITABLE) || !node.has(FLAG_HAS_SELECTION)) {
                    return false;
                }
                int start = arguments == null ? 0
                    : arguments.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 0);
                int end = arguments == null ? start
                    : arguments.getInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, start);
                nativePerformAction(node.id, KIND_SET_SELECTION, null, start, end);
                return true;
            }
            case AccessibilityNodeInfo.ACTION_SCROLL_FORWARD:
                if (!node.has(FLAG_INCREMENT)) {
                    return false;
                }
                nativePerformAction(node.id, KIND_INCREMENT, null, 0, 0);
                return true;
            case AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD:
                if (!node.has(FLAG_DECREMENT)) {
                    return false;
                }
                nativePerformAction(node.id, KIND_DECREMENT, null, 0, 0);
                return true;
            case AccessibilityNodeInfo.ACTION_EXPAND:
            case AccessibilityNodeInfo.ACTION_COLLAPSE:
                if (!node.has(FLAG_EXPAND)) {
                    return false;
                }
                nativePerformAction(node.id, KIND_EXPAND, null, 0, 0);
                return true;
            default:
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N
                        && action == AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId()
                        && node.has(FLAG_HAS_RANGE) && node.has(FLAG_SET_VALUE)
                        && arguments != null) {
                    float progress = arguments.getFloat(
                        AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, node.rangeCurrent);
                    nativePerformAction(node.id, KIND_SET_TEXT, Float.toString(progress), 0, 0);
                    return true;
                }
                return false;
        }
    }

    // ---- Role mapping ---------------------------------------------------------------------------

    private static String className(Node node) {
        switch (node.role) {
            case ROLE_BUTTON: return "android.widget.Button";
            case ROLE_CHECKBOX: return "android.widget.CheckBox";
            case ROLE_SWITCH: return "android.widget.Switch";
            case ROLE_RADIOBUTTON: return "android.widget.RadioButton";
            case ROLE_COMBOBOX: return "android.widget.Spinner";
            case ROLE_SLIDER:
            case ROLE_SPINBUTTON: return "android.widget.SeekBar";
            case ROLE_PROGRESS: return "android.widget.ProgressBar";
            case ROLE_TEXT: return "android.widget.TextView";
            case ROLE_TEXTINPUT: return "android.widget.EditText";
            case ROLE_IMAGE: return "android.widget.ImageView";
            case ROLE_LIST: return "android.widget.ListView";
            case ROLE_TABLE: return "android.widget.GridView";
            case ROLE_TABLIST: return "android.widget.TabWidget";
            case ROLE_RADIOGROUP: return "android.widget.RadioGroup";
            case ROLE_GROUP:
            case ROLE_PANE:
            case ROLE_TABPANEL:
            case ROLE_TREE: return "android.view.ViewGroup";
            default: return node.children.isEmpty() ? "android.view.View" : "android.view.ViewGroup";
        }
    }

    /** Roles Android has no widget class for get a spoken role instead. */
    private static String roleDescription(Node node) {
        switch (node.role) {
            case ROLE_TAB: return "Tab";
            case ROLE_LISTITEM: return "List item";
            case ROLE_TREE: return "Tree";
            case ROLE_SPINBUTTON: return "Spin button";
            default: return null;
        }
    }

    /** What a screen reader says for the node: label, then value, then description. */
    private static String spoken(Node node) {
        StringBuilder speech = new StringBuilder();
        append(speech, node.label);
        if (node.role != ROLE_TEXTINPUT && node.role != ROLE_PROGRESS && node.role != ROLE_SLIDER) {
            append(speech, node.value);
        }
        append(speech, node.description);
        return speech.toString();
    }

    private static void append(StringBuilder speech, String part) {
        if (part == null || part.isEmpty()) {
            return;
        }
        if (speech.length() > 0) {
            speech.append(", ");
        }
        speech.append(part);
    }
}
