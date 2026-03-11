import { create } from "zustand";
import { LoadedTab, TabUiState, defaultTabUiState, RecentViewEntry } from "../types";

interface AppState {
    tabs: LoadedTab[];
    activeTabId: string | null;
    tabUi: Record<string, TabUiState>;
    theme: "dark" | "light";
    recentFiles: string[];
    recentViews: RecentViewEntry[];
    isNotesWindowOpen: boolean;

    // ── Actions ──────────────────────────────────────────────────────────────
    addTabs: (newTabs: LoadedTab[]) => void;
    removeTab: (id: string) => void;
    setActiveTab: (id: string) => void;
    updateTabUi: (id: string, patch: Partial<TabUiState>) => void;
    setTheme: (t: "dark" | "light") => void;
    addRecentFile: (path: string) => void;
    addRecentView: (entry: RecentViewEntry) => void;
    toggleNotesWindow: () => void;

    // Feature: Inline Editing
    startEditing: (tabId: string, rowIndex: number, colName: string) => void;
    confirmEdit: (tabId: string, rowIndex: number, colName: string, newValue: string) => void;
    cancelEditing: (tabId: string) => void;
    undoEdit: (tabId: string) => void;
    redoEdit: (tabId: string) => void;
    setSaveStatus: (tabId: string, status: TabUiState['saveStatus']) => void;
}

export const useAppStore = create<AppState>((set) => {
    let initialRecentViews: RecentViewEntry[] = [];
    try {
        const stored = localStorage.getItem("recentViews");
        if (stored) {
            initialRecentViews = JSON.parse(stored);
        }
    } catch (e) {
        console.warn("Failed to parse recentViews from localStorage", e);
    }

    return {
        tabs: [],
        activeTabId: null,
        tabUi: {},
        theme: "dark",
        recentFiles: [],
        recentViews: initialRecentViews,
        isNotesWindowOpen: false,

    addTabs: (newTabs) =>
        set((s) => {
            const tabUiPatch: Record<string, TabUiState> = {};
            for (const t of newTabs) tabUiPatch[t.id] = defaultTabUiState(t);
            return {
                tabs: [...s.tabs, ...newTabs],
                tabUi: { ...s.tabUi, ...tabUiPatch },
                activeTabId: newTabs[newTabs.length - 1]?.id ?? s.activeTabId,
            };
        }),

    removeTab: (id) =>
        set((s) => {
            const idx = s.tabs.findIndex((t) => t.id === id);
            const remaining = s.tabs.filter((t) => t.id !== id);
            const { [id]: _, ...restUi } = s.tabUi;
            let nextActive = s.activeTabId;
            if (s.activeTabId === id) {
                nextActive = remaining[Math.max(0, idx - 1)]?.id ?? null;
            }
            return { tabs: remaining, tabUi: restUi, activeTabId: nextActive };
        }),

    setActiveTab: (id) => set({ activeTabId: id }),

    updateTabUi: (id, patch) =>
        set((s) => ({
            tabUi: { ...s.tabUi, [id]: { ...s.tabUi[id], ...patch } },
        })),

    setTheme: (t) => set({ theme: t }),

    addRecentFile: (path) =>
        set((s) => {
            const deduped = [path, ...s.recentFiles.filter((r) => r !== path)].slice(
                0,
                20
            );
            return { recentFiles: deduped };
        }),

    addRecentView: (entry) =>
        set((s) => {
            const filtered = s.recentViews.filter((r) => r.path !== entry.path);
            const deduped = [entry, ...filtered].slice(0, 20);
            localStorage.setItem("recentViews", JSON.stringify(deduped));
            return { recentViews: deduped };
        }),

    toggleNotesWindow: () =>
        set((s) => ({ isNotesWindowOpen: !s.isNotesWindowOpen })),

    startEditing: (tabId, rowIndex, colName) =>
        set((s) => {
            const ui = s.tabUi[tabId];
            if (!ui) return s;
            return {
                tabUi: {
                    ...s.tabUi,
                    [tabId]: { ...ui, editingCell: { rowIndex, colName } }
                }
            };
        }),

    confirmEdit: (tabId, rowIndex, colName, newValue) =>
        set((s) => {
            const tab = s.tabs.find((t) => t.id === tabId);
            const ui = s.tabUi[tabId];
            if (!tab || !ui) return s;

            // Optional optimization: skip if old value and new value are the same
            const oldValue = String((tab.records[rowIndex] as any)?.[colName] ?? "");
            if (oldValue === newValue) {
                return {
                    tabUi: {
                        ...s.tabUi,
                        [tabId]: { ...ui, editingCell: null }
                    }
                };
            }

            // Create new record array with mutability
            const newRecords = [...tab.records];
            newRecords[rowIndex] = { ...newRecords[rowIndex], [colName]: newValue };
            
            const newTabs = s.tabs.map(t => t.id === tabId ? { ...t, records: newRecords } : t);

            // Update UI state for history and edited marker
            const pastEntry = { rowIndex, colName, oldValue, newValue };
            let past = [...ui.editHistory.past, pastEntry];
            if (past.length > 50) past.shift();

            return {
                tabs: newTabs,
                tabUi: {
                    ...s.tabUi,
                    [tabId]: {
                        ...ui,
                        editingCell: null,
                        editHistory: { past, future: [] },
                        editedCells: { ...ui.editedCells, [`${rowIndex}-${colName}`]: true }
                    }
                }
            };
        }),

    cancelEditing: (tabId) =>
        set((s) => {
            const ui = s.tabUi[tabId];
            if (!ui) return s;
            return {
                tabUi: {
                    ...s.tabUi,
                    [tabId]: { ...ui, editingCell: null }
                }
            };
        }),

    undoEdit: (tabId) =>
        set((s) => {
            const tab = s.tabs.find((t) => t.id === tabId);
            const ui = s.tabUi[tabId];
            if (!tab || !ui || ui.editHistory.past.length === 0) return s;

            const past = [...ui.editHistory.past];
            const edit = past.pop()!;
            
            const newRecords = [...tab.records];
            newRecords[edit.rowIndex] = { ...newRecords[edit.rowIndex], [edit.colName]: edit.oldValue };

            const newTabs = s.tabs.map(t => t.id === tabId ? { ...t, records: newRecords } : t);

            // Removing the edited cell status if there are no more history items touching this cell?
            // Actually simpler to just leave it marked as dirty or unmark it if we want. But we must allow the auto-saver to save anyway.
            // Leaving edited status as is for simplicity, or re-calculating if needed.
            const future = [...ui.editHistory.future, edit];

            return {
                tabs: newTabs,
                tabUi: {
                    ...s.tabUi,
                    [tabId]: {
                        ...ui,
                        editHistory: { past, future },
                        editingCell: null
                    }
                }
            };
        }),

    redoEdit: (tabId) =>
        set((s) => {
            const tab = s.tabs.find((t) => t.id === tabId);
            const ui = s.tabUi[tabId];
            if (!tab || !ui || ui.editHistory.future.length === 0) return s;

            const future = [...ui.editHistory.future];
            const edit = future.pop()!;

            const newRecords = [...tab.records];
            newRecords[edit.rowIndex] = { ...newRecords[edit.rowIndex], [edit.colName]: edit.newValue };

            const newTabs = s.tabs.map(t => t.id === tabId ? { ...t, records: newRecords } : t);

            const past = [...ui.editHistory.past, edit];

            return {
                tabs: newTabs,
                tabUi: {
                    ...s.tabUi,
                    [tabId]: {
                        ...ui,
                        editHistory: { past, future },
                        editingCell: null,
                        // Make sure it remains marked as edited
                        editedCells: { ...ui.editedCells, [`${edit.rowIndex}-${edit.colName}`]: true }
                    }
                }
            };
        }),

    setSaveStatus: (tabId, status) =>
        set((s) => {
            const ui = s.tabUi[tabId];
            if (!ui) return s;
            return {
                tabUi: {
                    ...s.tabUi,
                    [tabId]: { ...ui, saveStatus: status }
                }
            };
        }),
    };
});
