import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ColumnStats } from "../types";
import { useAppStore } from "../store/appStore";

export function useStats(tabId: string) {
    const tab = useAppStore((s) => s.tabs.find((t) => t.id === tabId));
    const ui = useAppStore((s) => s.tabUi[tabId]);
    const updateTabUi = useAppStore((s) => s.updateTabUi);

    const loadStats = useCallback(
        async (col: string) => {
            if (!tab || !ui) return;
            try {
                const stats = await invoke<ColumnStats>("get_column_stats", {
                    col,
                    records: tab.records,
                    filteredIndices: ui.filteredIndices,
                });
                updateTabUi(tabId, {
                    activeStatsCol: col,
                    activeStats: stats,
                });
            } catch (err) {
                console.error("loadStats error:", err);
            }
        },
        [tab, ui, tabId, updateTabUi]
    );

    const clearStats = useCallback(() => {
        updateTabUi(tabId, { activeStatsCol: null, activeStats: null });
    }, [tabId, updateTabUi]);

    return { loadStats, clearStats };
}
