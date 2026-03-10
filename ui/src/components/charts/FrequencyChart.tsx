import React, { useRef, useCallback } from "react";
import {
    BarChart,
    Bar,
    XAxis,
    YAxis,
    CartesianGrid,
    Tooltip,
    ResponsiveContainer,
} from "recharts";
import { X, Download } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface FrequencyChartProps {
    tabId: string;
    col: string;
    onClose: () => void;
}

export function FrequencyChart({ tabId, col, onClose }: FrequencyChartProps) {
    const ui = useAppStore((s) => s.tabUi[tabId])!;
    const tab = useAppStore((s) => s.tabs.find((t) => t.id === tabId))!;
    const chartRef = useRef<HTMLDivElement>(null);

    useFocusTrap(chartRef, true, true, true);

    // Build frequency data from filtered records (top 20)
    const freq: Record<string, number> = {};
    for (const idx of ui.filteredIndices) {
        const rec = tab.records[idx] as Record<string, unknown>;
        const val = String(rec[col] ?? "null") || "null";
        freq[val] = (freq[val] ?? 0) + 1;
    }
    const total = ui.filteredIndices.length;
    const data = Object.entries(freq)
        .sort((a, b) => b[1] - a[1])
        .slice(0, 20)
        .map(([name, count]) => ({ name, count, pct: total > 0 ? ((count / total) * 100).toFixed(1) : "0" }));

    const exportPng = useCallback(() => {
        const svgEl = chartRef.current?.querySelector("svg");
        if (!svgEl) return;
        const svgData = new XMLSerializer().serializeToString(svgEl);
        const canvas = document.createElement("canvas");
        canvas.width = 480;
        canvas.height = 400;
        const ctx = canvas.getContext("2d")!;
        ctx.fillStyle = "#18181b";
        ctx.fillRect(0, 0, 480, 400);
        const img = new Image();
        img.onload = () => {
            ctx.drawImage(img, 0, 0);
            const a = document.createElement("a");
            a.download = `${col}_frecuencias.png`;
            a.href = canvas.toDataURL("image/png");
            a.click();
        };
        img.src = "data:image/svg+xml;base64," + btoa(unescape(encodeURIComponent(svgData)));
    }, [col]);

    const CustomTooltip = ({ active, payload }: { active?: boolean; payload?: { payload: { name: string; count: number; pct: string } }[] }) => {
        if (!active || !payload?.length) return null;
        const d = payload[0].payload;
        return (
            <div className="panel px-3 py-2 text-xs">
                <div className="text-zinc-300 font-mono mb-1">{d.name}</div>
                <div className="text-zinc-400">{d.count.toLocaleString()} registros</div>
                <div className="text-violet-400">{d.pct}% del total</div>
            </div>
        );
    };

    return (
        <div
            className="freq-chart-panel panel animate-fade-in flex flex-col"
            style={{ left: "calc(50vw - 240px)", top: "calc(50vh - 210px)" }}
            onClick={(e) => e.stopPropagation()}
        >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 shrink-0">
                <div>
                    <div className="text-zinc-100 font-semibold text-sm">{col}</div>
                    <div className="text-zinc-500 text-xs">Top {data.length} valores por frecuencia</div>
                </div>
                <div className="flex gap-1">
                    <button className="btn ghost h-7 px-2 text-xs" onClick={exportPng}>
                        <Download size={12} /> PNG
                    </button>
                    <button className="btn ghost h-7 w-7 p-0" onClick={onClose}>
                        <X size={13} />
                    </button>
                </div>
            </div>

            {/* Chart */}
            <div ref={chartRef} className="flex-1 p-3">
                <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={data} layout="vertical" margin={{ left: 0, right: 16, top: 4, bottom: 4 }}>
                        <CartesianGrid strokeDasharray="3 3" stroke="#27272a" horizontal={false} />
                        <XAxis type="number" tick={{ fill: "#71717a", fontSize: 11 }} axisLine={false} tickLine={false} />
                        <YAxis
                            type="category"
                            dataKey="name"
                            width={110}
                            tick={{ fill: "#a1a1aa", fontSize: 11, fontFamily: "monospace" }}
                            axisLine={false}
                            tickLine={false}
                            tickFormatter={(v) => (String(v).length > 14 ? String(v).slice(0, 13) + "…" : v)}
                        />
                        <Tooltip content={<CustomTooltip />} cursor={{ fill: "rgba(139,92,246,0.1)" }} />
                        <Bar dataKey="count" fill="#8b5cf6" radius={[0, 3, 3, 0]} />
                    </BarChart>
                </ResponsiveContainer>
            </div>
        </div>
    );
}
