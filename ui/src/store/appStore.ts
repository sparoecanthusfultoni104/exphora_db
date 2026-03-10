import { create } from "zustand";
import { LoadedTab, TabUiState, defaultTabUiState } from "../types";

interface AppState {
    tabs: LoadedTab[];
    activeTabId: string | null;
    tabUi: Record<string, TabUiState>;
    theme: "dark" | "light";
    recentFiles: string[];

    // ── Actions ──────────────────────────────────────────────────────────────
    addTabs: (newTabs: LoadedTab[]) => void;
    removeTab: (id: string) => void;
    setActiveTab: (id: string) => void;
    updateTabUi: (id: string, patch: Partial<TabUiState>) => void;
    setTheme: (t: "dark" | "light") => void;
    addRecentFile: (path: string) => void;
}

export const useAppStore = create<AppState>((set) => ({
    tabs: [],
    activeTabId: null,
    tabUi: {},
    theme: "dark",
    recentFiles: [],

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
}));
