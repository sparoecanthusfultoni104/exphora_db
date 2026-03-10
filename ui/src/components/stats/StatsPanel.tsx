import React from "react";
import { X, BarChart2 } from "lucide-react";
import { ColumnStats } from "../../types";
import { useAppStore } from "../../store/appStore";
import { useStats } from "../../hooks/useStats";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface StatsPanelProps {
    tabId: string;
}

function fmt(n: number | null, decimals = 4): string {
    if (n === null || n === undefined) return "—";
    return n.toLocaleString(undefined, { maximumFractionDigits: decimals });
}

export function StatsPanel({ tabId }: StatsPanelProps) {
    const ui = useAppStore((s) => s.tabUi[tabId]);
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const { clearStats } = useStats(tabId);

    if (!ui?.activeStats || !ui.activeStatsCol) return null;

    const s = ui.activeStats;
    const col = ui.activeStatsCol;

    const panelRef = React.useRef<HTMLDivElement>(null);
    useFocusTrap(panelRef, true, true, true);

    const nullCount = s.total - s.non_null;
    const nullPct = s.total > 0 ? ((nullCount / s.total) * 100).toFixed(1) : "0";
    const fillPct = s.total > 0 ? ((s.non_null / s.total) * 100).toFixed(1) : "0";

    return (
        <>
            <div className="sheet-overlay" onClick={clearStats} />
            <div className="sheet" ref={panelRef}>
                <div className="sheet-header">
                    <div>
                        <div className="text-zinc-100 font-semibold text-sm">{col}</div>
                        <div className="text-zinc-500 text-xs">Estadísticas de columna</div>
                    </div>
                    <button className="btn ghost h-7 w-7 p-0" onClick={clearStats}>
                        <X size={14} />
                    </button>
                </div>

                <div className="flex-1 overflow-y-auto p-4 space-y-4">
                    {/* Summary grid */}
                    <div className="grid grid-cols-2 gap-2">
                        {[
                            ["Total filas", s.total.toLocaleString()],
                            ["No nulos", `${s.non_null.toLocaleString()} (${fillPct}%)`],
                            ["Nulos", `${nullCount.toLocaleString()} (${nullPct}%)`],
                            ["Únicos", s.unique.toLocaleString()],
                        ].map(([label, value]) => (
                            <div key={label} className="bg-zinc-800 rounded-lg p-3">
                                <div className="text-zinc-500 text-[11px] mb-0.5">{label}</div>
                                <div className="text-zinc-100 text-sm font-semibold">{value}</div>
                            </div>
                        ))}
                    </div>

                    {/* Numeric stats */}
                    {s.is_numeric && (
                        <div>
                            <div className="text-zinc-400 text-xs font-semibold mb-2">Estadísticas numéricas</div>
                            <div className="grid grid-cols-2 gap-2">
                                {[
                                    ["Mínimo", fmt(s.min)],
                                    ["Máximo", fmt(s.max)],
                                    ["Media", fmt(s.mean)],
                                    ["Mediana", fmt(s.median)],
                                ].map(([label, value]) => (
                                    <div key={label} className="bg-zinc-800 rounded-lg p-3">
                                        <div className="text-zinc-500 text-[11px] mb-0.5">{label}</div>
                                        <div className="text-zinc-100 text-sm font-mono">{value}</div>
                                    </div>
                                ))}
                            </div>
                        </div>
                    )}

                    {/* Top values */}
                    {s.top_values.length > 0 && (
                        <div>
                            <div className="text-zinc-400 text-xs font-semibold mb-2">Valores más frecuentes</div>
                            <div className="space-y-1">
                                {s.top_values.map(([val, count]) => {
                                    const pct = s.total > 0 ? (count / s.total) * 100 : 0;
                                    return (
                                        <div key={val} className="flex items-center gap-2">
                                            <div className="flex-1 truncate text-xs text-zinc-300 font-mono">{val || <em className="text-zinc-600">null</em>}</div>
                                            <div className="text-zinc-500 text-xs w-12 text-right">{count.toLocaleString()}</div>
                                            <div className="w-20 h-2 bg-zinc-800 rounded-full overflow-hidden">
                                                <div className="h-full bg-violet-500 rounded-full" style={{ width: `${pct}%` }} />
                                            </div>
                                        </div>
                                    );
                                })}
                            </div>
                        </div>
                    )}

                    {/* Frequency chart button */}
                    <button
                        className="btn w-full"
                        onClick={() => updateTabUi(tabId, { showFrequencyChart: true, frequencyChartCol: col })}
                    >
                        <BarChart2 size={13} />
                        Ver gráfico de frecuencias
                    </button>
                </div>
            </div>
        </>
    );
}
