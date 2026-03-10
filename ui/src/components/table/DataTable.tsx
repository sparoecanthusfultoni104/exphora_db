import React, { useState, useRef, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { invoke } from "@tauri-apps/api/core";
import { ChevronUp, ChevronDown, Pin, PinOff, Calculator, BarChart2, SlidersHorizontal, Trash2 } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useFilters } from "../../hooks/useFilters";
import { useStats } from "../../hooks/useStats";
import { useAutoSave } from "../../hooks/useAutoSave";
import { FilterPanel } from "./FilterPanel";
import { useFocusTrap } from "../../hooks/useFocusTrap";
import { RecordDetailsModal } from "../modals/RecordDetailsModal";

const COL_WIDTH = 140;
const ROW_HEIGHT = 22;
const HDR_HEIGHT = 32;

interface ParsedCondition {
    field: string | null;
    value: string;
}

function parseSearchQuery(query: string): ParsedCondition[] {
    if (!query.trim()) return [];
    const conditions: ParsedCondition[] = [];
    const fieldRegex =
        /(\w+):\s*"([^"]*)"|(\w+):\s*([^,"\s]+)/g;
    let match;
    let hasFieldMatch = false;

    while ((match = fieldRegex.exec(query)) !== null) {
        hasFieldMatch = true;
        const field = match[1] ?? match[3];
        const value = match[2] !== undefined
            ? match[2]
            : (match[4] ?? "");
        if (field && value) {
            conditions.push({ field, value });
        }
    }

    if (!hasFieldMatch) {
        conditions.push({ field: null, value: query.trim() });
    }

    return conditions;
}

function highlightText(
    text: string,
    query: string
): React.ReactNode {
    if (!text || !query || !query.trim()) return text;
    const escaped = query.replace(
        /[.*+?^${}()|[\]\\]/g, '\\$&');
    const regex = new RegExp(`(${escaped})`, 'gi');
    const parts = text.split(regex);
    if (parts.length === 1) return text;
    return (
        <>
            {parts.map((part, i) =>
                i % 2 === 1 ? (
                    <span key={i} style={{
                        background: 'var(--color-warning)',
                        color: 'var(--color-bg)',
                        borderRadius: 'var(--radius-sm)',
                        padding: '0 2px'
                    }}>{part}</span>
                ) : part
            )}
        </>
    );
}

interface ContextMenu {
    x: number;
    y: number;
    col: string;
}

interface FilterPanelState {
    col: string;
    anchorX: number;
    anchorY: number;
}

interface AddCalcColState {
    name: string;
    expr: string;
}

export function DataTable({ tabId }: { tabId: string }) {
    const tab = useAppStore((s) => s.tabs.find((t) => t.id === tabId))!;
    const ui = useAppStore((s) => s.tabUi[tabId])!;
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const startEditing = useAppStore((s) => s.startEditing);
    const confirmEdit = useAppStore((s) => s. confirmEdit);
    const cancelEditing = useAppStore((s) => s.cancelEditing);

    const { applyFilters, getUniqueValues } = useFilters(tabId);
    const { loadStats } = useStats(tabId);
    
    // Feature: Auto-Save
    useAutoSave(tabId);

    const [ctxMenu, setCtxMenu] = useState<ContextMenu | null>(null);
    const [filterPanel, setFilterPanel] = useState<FilterPanelState | null>(null);
    const [addCalcDialog, setAddCalcDialog] = useState<AddCalcColState | null>(null);

    const parentRef = useRef<HTMLDivElement>(null);
    const ctxMenuRef = useRef<HTMLDivElement>(null);
    const rowCtxMenuRef = useRef<HTMLDivElement>(null);

    const [selectedRow, setSelectedRow] = useState<number | null>(null);
    const [rowCtxMenu, setRowCtxMenu] = useState<{ x: number; y: number; displayIdx: number; recordIdx: number } | null>(null);
    const [detailsRecordIdx, setDetailsRecordIdx] = useState<number | null>(null);

    useFocusTrap(ctxMenuRef, !!ctxMenu, true, true);
    useFocusTrap(rowCtxMenuRef, !!rowCtxMenu, true, true);

    React.useEffect(() => {
        const handleOpenFilter = (e: any) => {
            const ev = e as CustomEvent<{ col: string }>;
            const { col } = ev.detail;
            const el = document.querySelector(`[data-col="${col}"]`) as HTMLElement;
            if (el) {
                const rect = el.getBoundingClientRect();
                setFilterPanel({ col, anchorX: rect.left, anchorY: rect.bottom + 4 });
            } else {
                setFilterPanel({ col, anchorX: window.innerWidth / 2 - 150, anchorY: window.innerHeight / 2 - 200 });
            }
        };
        window.addEventListener("exphoradb:open-filter", handleOpenFilter);
        return () => window.removeEventListener("exphoradb:open-filter", handleOpenFilter);
    }, []);

    const visibleCols = [
        ...tab.columns.filter((c) => ui.visibleColumns[c] !== false),
        ...(ui.calcCols?.map((cc) => cc.name) ?? []),
    ];
    const frozenCols = ui.frozenCols.filter((c) => visibleCols.includes(c));
    const unfrozenCols = visibleCols.filter((c) => !frozenCols.includes(c));

    const filteredIndices = ui.filteredIndices;
    const textSearch = (ui.textSearch || "").trim();
    
    const conditions = parseSearchQuery(textSearch);

    const highlight = (val: string, col: string) => {
        if (!conditions.length || !val) return val;
        const matching = conditions.filter(({ field, value }) => {
            if (!value) return false;
            if (field) {
                return field.toLowerCase() === col.toLowerCase();
            }
            return true; // búsqueda global resalta en todo
        });
        if (!matching.length) return val;
        // Resaltar la primera condición que aplica
        return highlightText(val, matching[0].value);
    };

    // Filter by text search if active
    const displayIndices = conditions.length > 0
        ? filteredIndices.filter((idx) => {
            const rec = tab.records[idx] as Record<string, unknown>;
            return conditions.every(({ field, value }) => {
                if (!value) return true;
                const valLower = value.toLowerCase();
                if (field) {
                    const actualCol = Object.keys(rec).find(
                        k => k.toLowerCase() === field.toLowerCase()
                    ) ?? field;
                    return String(rec[actualCol] ?? "")
                        .toLowerCase()
                        .includes(valLower);
                }
                return visibleCols.some(c =>
                    String(rec[c] ?? "")
                        .toLowerCase()
                        .includes(valLower)
                );
            });
        })
        : filteredIndices;

    const rowVirtualizer = useVirtualizer({
        count: displayIndices.length,
        getScrollElement: () => parentRef.current,
        estimateSize: () => ROW_HEIGHT,
        overscan: 10,
    });

    const handleSort = (col: string) => {
        const asc = ui.sortCol === col ? !ui.sortAsc : true;
        // Sort filteredIndices
        const sorted = [...displayIndices].sort((a, b) => {
            const va = String((tab.records[a] as Record<string, unknown>)[col] ?? "");
            const vb = String((tab.records[b] as Record<string, unknown>)[col] ?? "");
            const na = parseFloat(va), nb = parseFloat(vb);
            const cmp = !isNaN(na) && !isNaN(nb) ? na - nb : va.localeCompare(vb);
            return asc ? cmp : -cmp;
        });
        updateTabUi(tabId, { sortCol: col, sortAsc: asc, filteredIndices: sorted });
    };

    const toggleFreeze = (col: string) => {
        const frozen = ui.frozenCols.includes(col)
            ? ui.frozenCols.filter((c) => c !== col)
            : ui.frozenCols.length < 5
                ? [...ui.frozenCols, col]
                : ui.frozenCols;
        updateTabUi(tabId, { frozenCols: frozen });
    };

    const removeCalcCol = (name: string) => {
        updateTabUi(tabId, {
            calcCols: ui.calcCols.filter((cc) => cc.name !== name),
            calcColCache: Object.fromEntries(
                Object.entries(ui.calcColCache).filter(([k]) => k !== name)
            ),
        });
    };

    const addCalcCol = async () => {
        if (!addCalcDialog?.name || !addCalcDialog.expr) return;
        try {
            const values = await invoke<(string | null)[]>("eval_calc_column", {
                exprStr: addCalcDialog.expr,
                records: tab.records,
            });
            updateTabUi(tabId, {
                calcCols: [...ui.calcCols, { name: addCalcDialog.name, expr: addCalcDialog.expr }],
                calcColCache: { ...ui.calcColCache, [addCalcDialog.name]: values },
            });
            setAddCalcDialog(null);
        } catch (err) {
            alert(`Expression error: ${err}`);
        }
    };

    const getCellValue = (col: string, recordIdx: number): string => {
        if (ui.calcColCache[col]) {
            return ui.calcColCache[col][recordIdx] ?? "";
        }
        const rec = tab.records[recordIdx] as Record<string, unknown>;
        const v = rec[col];
        if (v === null || v === undefined) return "";
        return String(v);
    };

    const isCalcCol = (col: string) => ui.calcCols.some((cc) => cc.name === col);
    const isNumericCell = (val: string) => val !== "" && !isNaN(parseFloat(val));

    const renderHeaderCell = (col: string, frozen: boolean) => {
        const sorted = ui.sortCol === col;
        const calc = isCalcCol(col);
        const isFrozen = frozenCols.includes(col);
        return (
            <div
                key={col}
                tabIndex={0}
                data-col={col}
                className={`table-header-cell ${sorted ? "sorted" : ""} ${calc ? "calc" : ""} ${isFrozen ? "frozen" : ""} focus:outline-none focus:ring-1 focus:ring-violet-500`}
                style={{ width: COL_WIDTH, minWidth: COL_WIDTH }}
                onClick={() => handleSort(col)}
                onContextMenu={(e) => {
                    e.preventDefault();
                    setCtxMenu({ x: e.clientX, y: e.clientY, col });
                }}
                onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        const rect = e.currentTarget.getBoundingClientRect();
                        setCtxMenu({ x: rect.left, y: rect.bottom, col });
                    }
                }}
            >
                {calc && <span className="mr-1 text-violet-400">ƒ</span>}
                <span className="truncate flex-1">{col}</span>
                {sorted && (
                    <span className="ml-1 shrink-0">
                        {ui.sortAsc ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
                    </span>
                )}
            </div>
        );
    };

    const frozenWidth = frozenCols.length * COL_WIDTH;
    const totalContentWidth = visibleCols.length * COL_WIDTH;

    return (
        <div className="flex flex-col flex-1 overflow-hidden relative" onClick={() => { setCtxMenu(null); setRowCtxMenu(null); }}>
            {/* ── Column count bar ── */}
            <div className="flex items-center gap-3 px-3 py-1 border-b border-zinc-800 text-xs text-zinc-500 shrink-0 bg-zinc-900">
                <span>{displayIndices.length.toLocaleString()} / {tab.total_rows.toLocaleString()} rows</span>
                <span>·</span>
                <span>{visibleCols.length} columns</span>
                <div className="flex-1" />
                <button
                    className="btn ghost text-zinc-400 text-xs h-6"
                    onClick={() => setAddCalcDialog({ name: "", expr: "" })}
                >
                    <Calculator size={12} />
                    <span>+ add calc column</span>
                </button>
            </div>

            {/* ── Table ── */}
            <div className="flex flex-1 overflow-hidden">
                {/* Frozen columns */}
                {frozenCols.length > 0 && (
                    <div
                        className="flex flex-col shrink-0 border-r-2 z-10"
                        style={{ borderColor: "rgba(139,92,246,0.4)", width: frozenWidth }}
                    >
                        <div className="flex" style={{ height: HDR_HEIGHT, minHeight: HDR_HEIGHT }}>
                            {frozenCols.map((c) => renderHeaderCell(c, true))}
                        </div>
                        <div className="overflow-hidden flex-1" style={{ position: "relative" }}>
                            {rowVirtualizer.getVirtualItems().map((vRow) => {
                                const idx = displayIndices[vRow.index];
                                return (
                                    <div
                                        key={vRow.key}
                                        className={`table-row ${vRow.index % 2 === 0 ? "even" : "odd"} ${selectedRow === vRow.index ? "selected" : ""}`}
                                        style={{ position: "absolute", top: vRow.start, width: frozenWidth }}
                                        onClick={() => setSelectedRow(vRow.index)}
                                        onContextMenu={(e) => {
                                            e.preventDefault();
                                            setSelectedRow(vRow.index);
                                            setRowCtxMenu({ x: e.clientX, y: e.clientY, displayIdx: vRow.index, recordIdx: idx });
                                        }}
                                    >
                                        {frozenCols.map((c) => {
                                            const val = getCellValue(c, idx);
                                            const isEditing = ui.editingCell?.rowIndex === idx && ui.editingCell?.colName === c;
                                            const isEdited = !!ui.editedCells[`${idx}-${c}`];
                                            
                                            return (
                                                <div
                                                    key={c}
                                                    className={`table-cell ${isNumericCell(val) && !isEditing ? "numeric" : ""} ${val === "" && !isEditing ? "null-val" : ""}`}
                                                    style={{ 
                                                        width: COL_WIDTH, 
                                                        minWidth: COL_WIDTH,
                                                        borderLeft: isEdited ? "2px solid var(--color-brand)" : undefined
                                                    }}
                                                    title={isEditing ? undefined : (val || "null")}
                                                    onDoubleClick={() => {
                                                        if (!isCalcCol(c)) startEditing(tabId, idx, c);
                                                    }}
                                                >
                                                    {isEditing ? (
                                                        <input
                                                            className="inline-edit-input"
                                                            type="text"
                                                            defaultValue={val}
                                                            autoFocus
                                                            onKeyDown={(e) => {
                                                                if (e.key === "Enter") {
                                                                    confirmEdit(tabId, idx, c, e.currentTarget.value);
                                                                } else if (e.key === "Escape") {
                                                                    cancelEditing(tabId);
                                                                }
                                                            }}
                                                            onBlur={(e) => {
                                                                confirmEdit(tabId, idx, c, e.target.value);
                                                            }}
                                                            style={{
                                                                width: "100%",
                                                                height: "100%",
                                                                backgroundColor: "var(--color-surface)",
                                                                border: "none",
                                                                outline: "2px solid var(--color-brand)",
                                                                borderRadius: "var(--radius-sm)",
                                                                fontSize: "var(--font-size-sm)",
                                                                color: "var(--color-text-primary)",
                                                                padding: "0 var(--table-cell-px)"
                                                            }}
                                                        />
                                                    ) : (
                                                        val ? highlight(val, c) : <em>null</em>
                                                    )}
                                                </div>
                                            );
                                        })}
                                    </div>
                                );
                            })}
                            <div style={{ height: rowVirtualizer.getTotalSize() }} />
                        </div>
                    </div>
                )}

                {/* Scrollable area */}
                <div className="flex-1 overflow-auto" ref={parentRef}>
                    {/* Header row */}
                    <div className="flex sticky top-0 z-10" style={{ width: totalContentWidth - frozenWidth }}>
                        {unfrozenCols.map((c) => renderHeaderCell(c, false))}
                    </div>
                    {/* Rows */}
                    <div style={{ height: rowVirtualizer.getTotalSize(), position: "relative", width: totalContentWidth - frozenWidth }}>
                        {rowVirtualizer.getVirtualItems().map((vRow) => {
                            const idx = displayIndices[vRow.index];
                            return (
                                <div
                                    key={vRow.key}
                                    className={`table-row ${vRow.index % 2 === 0 ? "even" : "odd"} ${selectedRow === vRow.index ? "selected" : ""}`}
                                    style={{ position: "absolute", top: vRow.start, width: totalContentWidth - frozenWidth }}
                                    onClick={() => setSelectedRow(vRow.index)}
                                    onContextMenu={(e) => {
                                        e.preventDefault();
                                        setSelectedRow(vRow.index);
                                        setRowCtxMenu({ x: e.clientX, y: e.clientY, displayIdx: vRow.index, recordIdx: idx });
                                    }}
                                >
                                    {unfrozenCols.map((c) => {
                                        const val = getCellValue(c, idx);
                                        const isEditing = ui.editingCell?.rowIndex === idx && ui.editingCell?.colName === c;
                                        const isEdited = !!ui.editedCells[`${idx}-${c}`];

                                        return (
                                            <div
                                                key={c}
                                                className={`table-cell ${isNumericCell(val) && !isEditing ? "numeric" : ""} ${val === "" && !isEditing ? "null-val" : ""}`}
                                                style={{ 
                                                    width: COL_WIDTH, 
                                                    minWidth: COL_WIDTH,
                                                    borderLeft: isEdited ? "2px solid var(--color-brand)" : undefined
                                                }}
                                                title={isEditing ? undefined : (val || "null")}
                                                onDoubleClick={() => {
                                                    if (!isCalcCol(c)) startEditing(tabId, idx, c);
                                                }}
                                            >
                                                {isEditing ? (
                                                    <input
                                                        className="inline-edit-input"
                                                        type="text"
                                                        defaultValue={val}
                                                        autoFocus
                                                        onKeyDown={(e) => {
                                                            if (e.key === "Enter") {
                                                                confirmEdit(tabId, idx, c, e.currentTarget.value);
                                                            } else if (e.key === "Escape") {
                                                                cancelEditing(tabId);
                                                            }
                                                        }}
                                                        onBlur={(e) => {
                                                            confirmEdit(tabId, idx, c, e.target.value);
                                                        }}
                                                        style={{
                                                            width: "100%",
                                                            height: "100%",
                                                            backgroundColor: "var(--color-surface)",
                                                            border: "none",
                                                            outline: "2px solid var(--color-brand)",
                                                            borderRadius: "var(--radius-sm)",
                                                            fontSize: "var(--font-size-sm)",
                                                            color: "var(--color-text-primary)",
                                                            padding: "0 var(--table-cell-px)"
                                                        }}
                                                    />
                                                ) : (
                                                    val ? highlight(val, c) : <em>null</em>
                                                )}
                                            </div>
                                        );
                                    })}
                                </div>
                            );
                        })}
                    </div>
                </div>
            </div>

            {/* ── Context menu ── */}
            {ctxMenu && (
                <div
                    ref={ctxMenuRef}
                    className="context-menu animate-fade-in"
                    style={{ position: "fixed", left: ctxMenu.x, top: ctxMenu.y }}
                    onClick={(e) => e.stopPropagation()}
                    onKeyDown={(e) => {
                        if (e.key === "Escape") {
                            e.preventDefault();
                            setCtxMenu(null);
                        } else if (e.key === "ArrowDown") {
                            e.preventDefault();
                            const next = document.activeElement?.nextElementSibling as HTMLElement;
                            if (next?.classList.contains("context-menu-item")) next.focus();
                            else {
                                const first = ctxMenuRef.current?.querySelector(".context-menu-item") as HTMLElement;
                                if (first) first.focus();
                            }
                        } else if (e.key === "ArrowUp") {
                            e.preventDefault();
                            const prev = document.activeElement?.previousElementSibling as HTMLElement;
                            if (prev?.classList.contains("context-menu-item")) prev.focus();
                            else {
                                const items = ctxMenuRef.current?.querySelectorAll(".context-menu-item");
                                if (items && items.length > 0) (items[items.length - 1] as HTMLElement).focus();
                            }
                        }
                    }}
                >
                    <div className="px-2 py-1 text-zinc-500 text-[11px] font-semibold">{ctxMenu.col}</div>
                    <div className="context-menu-sep" />
                    <div tabIndex={0} className="context-menu-item focus:bg-zinc-800 focus:outline-none" onClick={() => { handleSort(ctxMenu.col); setCtxMenu(null); }} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { handleSort(ctxMenu.col); setCtxMenu(null); } }}>
                        <ChevronUp size={13} /> Sort
                    </div>
                    <div tabIndex={0} className="context-menu-item focus:bg-zinc-800 focus:outline-none" onClick={async () => {
                        const el = document.querySelector(`[data-col="${ctxMenu.col}"]`) as HTMLElement;
                        const rect = el?.getBoundingClientRect() ?? { bottom: ctxMenu.y, left: ctxMenu.x };
                        setFilterPanel({ col: ctxMenu.col, anchorX: rect.left, anchorY: rect.bottom + 4 });
                        setCtxMenu(null);
                    }} onKeyDown={async (e) => {
                        if (e.key === "Enter" || e.key === " ") {
                            const el = document.querySelector(`[data-col="${ctxMenu.col}"]`) as HTMLElement;
                            const rect = el?.getBoundingClientRect() ?? { bottom: ctxMenu.y, left: ctxMenu.x };
                            setFilterPanel({ col: ctxMenu.col, anchorX: rect.left, anchorY: rect.bottom + 4 });
                            setCtxMenu(null);
                        }
                    }}>
                        <SlidersHorizontal size={13} /> Filter
                    </div>
                    <div tabIndex={0} className="context-menu-item focus:bg-zinc-800 focus:outline-none" onClick={() => { loadStats(ctxMenu.col); setCtxMenu(null); }} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { loadStats(ctxMenu.col); setCtxMenu(null); } }}>
                        <BarChart2 size={13} /> View Statistics
                    </div>
                    <div tabIndex={0} className="context-menu-item focus:bg-zinc-800 focus:outline-none" onClick={() => { loadStats(ctxMenu.col); updateTabUi(tabId, { showFrequencyChart: true, frequencyChartCol: ctxMenu.col }); setCtxMenu(null); }} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { loadStats(ctxMenu.col); updateTabUi(tabId, { showFrequencyChart: true, frequencyChartCol: ctxMenu.col }); setCtxMenu(null); } }}>
                        <BarChart2 size={13} /> Frequency Chart
                    </div>
                    <div className="context-menu-sep" />
                    <div tabIndex={0} className="context-menu-item focus:bg-zinc-800 focus:outline-none" onClick={() => { toggleFreeze(ctxMenu.col); setCtxMenu(null); }} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { toggleFreeze(ctxMenu.col); setCtxMenu(null); } }}>
                        {frozenCols.includes(ctxMenu.col) ? <><PinOff size={13} /> Unfreeze</> : <><Pin size={13} /> Freeze Column</>}
                    </div>
                    {isCalcCol(ctxMenu.col) && (
                        <div tabIndex={0} className="context-menu-item danger focus:bg-zinc-800 focus:outline-none" onClick={() => { removeCalcCol(ctxMenu.col); setCtxMenu(null); }} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { removeCalcCol(ctxMenu.col); setCtxMenu(null); } }}>
                            <Trash2 size={13} /> Remove Calc Column
                        </div>
                    )}
                </div>
            )}

            {/* ── Filter panel ── */}
            {filterPanel && (
                <FilterPanel
                    tabId={tabId}
                    col={filterPanel.col}
                    anchorX={filterPanel.anchorX}
                    anchorY={filterPanel.anchorY}
                    onClose={() => setFilterPanel(null)}
                />
            )}

            {/* ── Add calc column dialog ── */}
            {addCalcDialog !== null && (
                <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center animate-fade-in" onClick={() => setAddCalcDialog(null)}>
                    <div className="panel p-5 w-96 animate-fade-in" onClick={(e) => e.stopPropagation()}>
                        <h3 className="text-zinc-100 font-semibold mb-3 flex items-center gap-2">
                            <Calculator size={15} className="text-violet-400" /> New Calculated Column
                        </h3>
                        <div className="space-y-3">
                            <div>
                                <label className="text-xs text-zinc-400 block mb-1">Name</label>
                                <input type="text" className="w-full" placeholder="e.g. price_tax" value={addCalcDialog.name}
                                    onChange={(e) => setAddCalcDialog((d) => d ? { ...d, name: e.target.value } : d)} />
                            </div>
                            <div>
                                <label className="text-xs text-zinc-400 block mb-1">Expression</label>
                                <input type="text" className="w-full font-mono" placeholder="e.g. price * 1.21" value={addCalcDialog.expr}
                                    onChange={(e) => setAddCalcDialog((d) => d ? { ...d, expr: e.target.value } : d)} />
                                <p className="text-zinc-600 text-[11px] mt-1">upper, lower, len, trim, round, str, num — reference columns by name</p>
                            </div>
                            <div className="flex gap-2 justify-end">
                                <button className="btn" onClick={() => setAddCalcDialog(null)}>Cancel</button>
                                <button className="btn primary" onClick={addCalcCol}>Add</button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {/* ── Row context menu ── */}
            {rowCtxMenu && (
                <div
                    ref={rowCtxMenuRef}
                    className="context-menu animate-fade-in"
                    style={{ position: "fixed", left: rowCtxMenu.x, top: rowCtxMenu.y }}
                    onClick={(e) => e.stopPropagation()}
                    onKeyDown={(e) => {
                        if (e.key === "Escape") {
                            e.preventDefault();
                            setRowCtxMenu(null);
                        } else if (e.key === "ArrowDown") {
                            e.preventDefault();
                            const next = document.activeElement?.nextElementSibling as HTMLElement;
                            if (next?.classList.contains("context-menu-item")) next.focus();
                            else {
                                const first = rowCtxMenuRef.current?.querySelector(".context-menu-item") as HTMLElement;
                                if (first) first.focus();
                            }
                        } else if (e.key === "ArrowUp") {
                            e.preventDefault();
                            const prev = document.activeElement?.previousElementSibling as HTMLElement;
                            if (prev?.classList.contains("context-menu-item")) prev.focus();
                            else {
                                const items = rowCtxMenuRef.current?.querySelectorAll(".context-menu-item");
                                if (items && items.length > 0) (items[items.length - 1] as HTMLElement).focus();
                            }
                        }
                    }}
                >
                    <div className="px-2 py-1 text-zinc-500 text-[11px] font-semibold">Row {rowCtxMenu.recordIdx + 1}</div>
                    <div className="context-menu-sep" />
                    <div tabIndex={0} className="context-menu-item focus:bg-zinc-800 focus:outline-none" onClick={() => { setDetailsRecordIdx(rowCtxMenu.recordIdx); setRowCtxMenu(null); }} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { setDetailsRecordIdx(rowCtxMenu.recordIdx); setRowCtxMenu(null); } }}>
                        View Record Details
                    </div>
                </div>
            )}

            {/* ── Record Details Modal ── */}
            {detailsRecordIdx !== null && (
                <RecordDetailsModal
                    tab={tab}
                    recordIdx={detailsRecordIdx}
                    onClose={() => setDetailsRecordIdx(null)}
                />
            )}
        </div>
    );
}
