import React, { useEffect, useState, useRef } from "react";
import { X, RotateCcw } from "lucide-react";
import { DynamicFiltersDto, EasyFilter, FilterRule, UniqueValuesResult } from "../../types";
import { useAppStore } from "../../store/appStore";
import { useFilters } from "../../hooks/useFilters";
import { useFocusTrap } from "../../hooks/useFocusTrap";

const FILTER_OPS = [
    "Contains", "NotContains", "Equals", "NotEquals",
    "GreaterThan", "LessThan", "IsNull", "IsNotNull",
];
const OP_LABELS: Record<string, string> = {
    Contains: "contains", NotContains: "does not contain", Equals: "equals",
    NotEquals: "not equals", GreaterThan: "greater than", LessThan: "less than",
    IsNull: "is null", IsNotNull: "is not null",
};
const NO_VALUE_OPS = new Set(["IsNull", "IsNotNull"]);

interface FilterPanelProps {
    tabId: string;
    col: string;
    anchorX: number;
    anchorY: number;
    onClose: () => void;
}

export function FilterPanel({ tabId, col, anchorX, anchorY, onClose }: FilterPanelProps) {
    const ui = useAppStore((s) => s.tabUi[tabId])!;
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const { applyFilters, getUniqueValues } = useFilters(tabId);

    const currentMode = ui.filters.filter_mode[col] ?? "Easy";
    const [mode, setMode] = useState<"Easy" | "Advanced">(currentMode as "Easy" | "Advanced");
    const [uniqueResult, setUniqueResult] = useState<UniqueValuesResult | null>(null);
    const [localSearch, setLocalSearch] = useState("");
    const [easyFilter, setEasyFilter] = useState<EasyFilter>(
        ui.filters.easy_filters[col] ?? { selected: [], all_selected: true }
    );
    const [advRules, setAdvRules] = useState<FilterRule[]>(
        ui.filters.filters[col] ?? [{ op: "Contains", value: "", connector: "And" }]
    );

    const panelRef = useRef<HTMLDivElement>(null);
    useFocusTrap(panelRef, true, true, true);

    useEffect(() => {
        getUniqueValues(col).then(setUniqueResult);
    }, [col]);

    // Close on Escape or outside click
    useEffect(() => {
        const handler = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
        window.addEventListener("keydown", handler);
        return () => window.removeEventListener("keydown", handler);
    }, [onClose]);

    const commitFilters = (newEasy: EasyFilter, newRules: FilterRule[], newMode: "Easy" | "Advanced") => {
        const newFilters: DynamicFiltersDto = {
            ...ui.filters,
            filter_mode: { ...ui.filters.filter_mode, [col]: newMode },
            easy_filters: { ...ui.filters.easy_filters, [col]: newEasy },
            filters: { ...ui.filters.filters, [col]: newRules },
        };
        applyFilters(newFilters);
    };

    const allValues = uniqueResult?.values ?? [];
    const displayValues = localSearch
        ? allValues.filter(([v]) => v.toLowerCase().includes(localSearch.toLowerCase()))
        : allValues;

    const toggleValue = (val: string) => {
        let newSelected: string[];
        if (easyFilter.all_selected) {
            newSelected = allValues.map(([v]) => v).filter((v) => v !== val);
        } else {
            newSelected = easyFilter.selected.includes(val)
                ? easyFilter.selected.filter((v) => v !== val)
                : [...easyFilter.selected, val];
        }
        const allChecked = newSelected.length === allValues.length;
        const updated: EasyFilter = { selected: newSelected, all_selected: allChecked };
        setEasyFilter(updated);
        commitFilters(updated, advRules, mode);
    };

    const selectAll = () => {
        const updated: EasyFilter = { selected: allValues.map(([v]) => v), all_selected: true };
        setEasyFilter(updated);
        commitFilters(updated, advRules, mode);
    };

    const clearFilter = () => {
        const updated: EasyFilter = { selected: allValues.map(([v]) => v), all_selected: true };
        setEasyFilter(updated);
        const newFilters: DynamicFiltersDto = {
            ...ui.filters,
            filter_mode: { ...ui.filters.filter_mode, [col]: mode },
            easy_filters: { ...ui.filters.easy_filters, [col]: updated },
            filters: { ...ui.filters.filters, [col]: [] },
        };
        applyFilters(newFilters);
    };

    const updateRule = (idx: number, patch: Partial<FilterRule>) => {
        const updated = advRules.map((r, i) => i === idx ? { ...r, ...patch } : r);
        setAdvRules(updated);
        commitFilters(easyFilter, updated, mode);
    };

    const addRule = () => setAdvRules((r) => [...r, { op: "Contains", value: "", connector: "And" }]);
    const removeRule = (idx: number) => {
        const updated = advRules.filter((_, i) => i !== idx);
        setAdvRules(updated);
        commitFilters(easyFilter, updated, mode);
    };

    return (
        <div
            ref={panelRef}
            className="filter-panel panel animate-fade-in"
            style={{ left: anchorX, top: anchorY }}
            onClick={(e) => e.stopPropagation()}
        >
            {/* Header */}
            <div className="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
                <span className="text-xs font-semibold text-zinc-300 truncate max-w-[180px]">{col}</span>
                <div className="flex items-center gap-1">
                    <button className="btn ghost h-6 px-2 text-xs text-zinc-400" onClick={clearFilter}>
                        <RotateCcw size={11} /> Clear
                    </button>
                    <button className="btn ghost h-6 w-6 p-0 flex items-center justify-center" onClick={onClose}>
                        <X size={12} />
                    </button>
                </div>
            </div>

            {/* Mode tabs */}
            <div className="flex border-b border-zinc-800 shrink-0">
                {(["Easy", "Advanced"] as const).map((m) => (
                    <button
                        key={m}
                        className={`flex-1 text-xs py-1.5 transition-colors ${mode === m ? "text-violet-400 border-b-2 border-violet-500" : "text-zinc-500 hover:text-zinc-300"}`}
                        onClick={() => { setMode(m); commitFilters(easyFilter, advRules, m); }}
                    >
                        {m === "Easy" ? "Easy" : "Advanced"}
                    </button>
                ))}
            </div>

            {/* Content */}
            <div className="flex-1 overflow-y-auto p-2">
                {mode === "Easy" ? (
                    <>
                        <input type="text" className="w-full text-xs mb-2" placeholder="Search value…"
                            value={localSearch} onChange={(e) => setLocalSearch(e.target.value)} />
                        {uniqueResult?.truncated && (
                            <p className="text-[11px] text-amber-500 mb-1">Top 200 values</p>
                        )}
                        <div className="space-y-0.5">
                            {displayValues.map(([val, count]) => {
                                const checked = easyFilter.all_selected || easyFilter.selected.includes(val);
                                return (
                                    <label key={val} className="flex items-center gap-2 text-xs cursor-pointer hover:bg-zinc-800 rounded px-1 py-0.5">
                                        <input type="checkbox" checked={checked} onChange={() => toggleValue(val)} />
                                        <span className={`flex-1 truncate ${val === "null" ? "text-zinc-600 italic" : "text-zinc-300"}`}>{val}</span>
                                        <span className="badge">{count}</span>
                                    </label>
                                );
                            })}
                        </div>
                        <div className="mt-2 flex gap-1">
                            <button className="btn text-xs h-6" onClick={selectAll}>All</button>
                        </div>
                    </>
                ) : (
                    <div className="space-y-2">
                        {advRules.map((rule, i) => (
                            <div key={i} className="space-y-1">
                                {i > 0 && (
                                    <button
                                        className="text-xs text-violet-400 font-semibold"
                                        onClick={() => updateRule(i - 1, { connector: rule.connector === "And" ? "Or" : "And" })}
                                    >
                                        {advRules[i - 1].connector === "And" ? "AND" : "OR"} ↕
                                    </button>
                                )}
                                <div className="flex items-center gap-1">
                                    <select
                                        className="text-xs flex-1"
                                        value={rule.op}
                                        onChange={(e) => updateRule(i, { op: e.target.value as FilterRule["op"] })}
                                    >
                                        {FILTER_OPS.map((op) => <option key={op} value={op}>{OP_LABELS[op]}</option>)}
                                    </select>
                                    {!NO_VALUE_OPS.has(rule.op) && (
                                        <input type="text" className="text-xs w-24" value={rule.value} placeholder="value"
                                            onChange={(e) => updateRule(i, { value: e.target.value })} />
                                    )}
                                    {advRules.length > 1 && (
                                        <button className="btn ghost h-6 w-6 p-0 text-red-400" onClick={() => removeRule(i)}>
                                            <X size={11} />
                                        </button>
                                    )}
                                </div>
                            </div>
                        ))}
                        <button className="btn text-xs h-6 w-full" onClick={addRule}>+ Add Rule</button>
                    </div>
                )}
            </div>
        </div>
    );
}
