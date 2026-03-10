import { useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
    DynamicFiltersDto,
    FilterResult,
    UniqueValuesResult,
} from "../types";
import { useAppStore } from "../store/appStore";

const DEBOUNCE_MS = 150;

export function useFilters(tabId: string) {
    const tab = useAppStore((s) => s.tabs.find((t) => t.id === tabId));
    const ui = useAppStore((s) => s.tabUi[tabId]);
    const updateTabUi = useAppStore((s) => s.updateTabUi);

    const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    const applyFilters = useCallback(
        async (filters: DynamicFiltersDto) => {
            if (!tab) return;
            if (debounceRef.current) clearTimeout(debounceRef.current);
            debounceRef.current = setTimeout(async () => {
                try {
                    const result = await invoke<FilterResult>("apply_filters", {
                        records: tab.records,
                        filtersDto: filters,
                    });
                    updateTabUi(tabId, {
                        filteredIndices: result.filtered_indices,
                        filters,
                    });
                } catch (err) {
                    console.error("applyFilters error:", err);
                }
            }, DEBOUNCE_MS);
        },
        [tab, tabId, updateTabUi]
    );

    const getUniqueValues = useCallback(
        async (col: string): Promise<UniqueValuesResult | null> => {
            if (!tab || !ui) return null;
            try {
                return await invoke<UniqueValuesResult>("get_unique_values", {
                    col,
                    records: tab.records,
                    filteredIndices: ui.filteredIndices,
                });
            } catch (err) {
                console.error("getUniqueValues error:", err);
                return null;
            }
        },
        [tab, ui]
    );

    const resetFilters = useCallback(() => {
        if (!tab) return;
        const empty: DynamicFiltersDto = {
            text_search: "",
            filters: {},
            easy_filters: {},
            filter_mode: {},
        };
        updateTabUi(tabId, {
            filters: empty,
            filteredIndices: tab.records.map((_, i) => i),
        });
    }, [tab, tabId, updateTabUi]);

    return { applyFilters, getUniqueValues, resetFilters };
}
