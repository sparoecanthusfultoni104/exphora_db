import React, { useState, useRef, useEffect } from "react";
import { X, SlidersHorizontal, BarChart2, PieChart } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useFocusTrap } from "../../hooks/useFocusTrap";
import { useStats } from "../../hooks/useStats";

interface ColumnPickerModalProps {
    action: "filter" | "stats" | "chart";
    onClose: () => void;
}

export function ColumnPickerModal({ action, onClose }: ColumnPickerModalProps) {
    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabs = useAppStore((s) => s.tabs);
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const { loadStats } = useStats(activeTabId || "");

    const activeTab = tabs.find((t) => t.id === activeTabId);
    const columns = activeTab ? activeTab.columns : [];

    const [search, setSearch] = useState("");
    const [selectedIndex, setSelectedIndex] = useState(0);
    const modalRef = useRef<HTMLDivElement>(null);
    const listRef = useRef<HTMLDivElement>(null);

    useFocusTrap(modalRef);

    const filteredCols = columns.filter((c) =>
        c.toLowerCase().includes(search.toLowerCase())
    );

    useEffect(() => {
        setSelectedIndex(0);
    }, [search]);

    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                e.stopPropagation();
                onClose();
            } else if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelectedIndex((prev) => Math.min(prev + 1, filteredCols.length - 1));
            } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelectedIndex((prev) => Math.max(prev - 1, 0));
            } else if (e.key === "Enter") {
                e.preventDefault();
                if (filteredCols.length > 0) {
                    handleSelect(filteredCols[selectedIndex]);
                }
            }
        };
        const modal = modalRef.current;
        if (modal) {
            modal.addEventListener("keydown", handler);
            return () => modal.removeEventListener("keydown", handler);
        }
    }, [filteredCols, selectedIndex, onClose]);

    // Scroll selected item into view securely
    useEffect(() => {
        if (listRef.current) {
            const selectedElement = listRef.current.children[selectedIndex] as HTMLElement;
            if (selectedElement) {
                selectedElement.scrollIntoView({ block: "nearest", behavior: "smooth" });
            }
        }
    }, [selectedIndex]);

    const handleSelect = (col: string) => {
        if (!activeTabId) return;

        // This simulates opening the panels. 
        // For stats and chart, it's just setting the active config in App store.
        // For filter, since the filter panel relies on anchor coordinates, 
        // we'll center it roughly using a dummy coordinate, or we can just 
        // rely on DataTable's implementation. Wait, the prompt says:
        // "Al seleccionar columna, dispara la acción correspondiente (abrir FilterPanel, StatsPanel o FrequencyChart)."

        if (action === "stats") {
            loadStats(col);
        } else if (action === "chart") {
            loadStats(col);
            updateTabUi(activeTabId, { showFrequencyChart: true, frequencyChartCol: col });
        } else if (action === "filter") {
            const ev = new CustomEvent("exphoradb:open-filter", { detail: { col } });
            window.dispatchEvent(ev);
        }
        onClose();
    };

    const getIcon = () => {
        if (action === "filter") return <SlidersHorizontal size={15} className="text-violet-400" />;
        if (action === "stats") return <BarChart2 size={15} className="text-blue-400" />;
        return <PieChart size={15} className="text-green-400" />;
    };

    const getTitle = () => {
        if (action === "filter") return "Filter Column";
        if (action === "stats") return "Column Statistics";
        return "Frequency Chart";
    };

    return (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center animate-fade-in" onClick={onClose}>
            <div ref={modalRef} className="panel p-0 w-96 animate-fade-in overflow-hidden flex flex-col" style={{ maxHeight: 400 }} onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 shrink-0">
                    <div className="flex items-center gap-2">
                        {getIcon()}
                        <span className="text-zinc-100 font-semibold text-sm">{getTitle()}</span>
                    </div>
                    <button className="btn ghost h-7 w-7 p-0" onClick={onClose}>
                        <X size={13} />
                    </button>
                </div>
                <div className="p-2 border-b border-zinc-800 shrink-0">
                    <input
                        type="text"
                        placeholder="Search column..."
                        className="w-full text-sm"
                        value={search}
                        onChange={(e) => setSearch(e.target.value)}
                        autoFocus
                    />
                </div>
                <div ref={listRef} className="p-2 overflow-y-auto flex-1 space-y-0.5">
                    {filteredCols.length === 0 && (
                        <div className="text-zinc-500 text-xs p-2 text-center">No columns found.</div>
                    )}
                    {filteredCols.map((col, i) => (
                        <button
                            key={col}
                            className={`w-full text-left px-3 py-2 rounded text-sm transition-colors ${i === selectedIndex ? "bg-violet-600/20 text-violet-300" : "text-zinc-300 hover:bg-zinc-800"
                                }`}
                            onClick={() => handleSelect(col)}
                            onMouseEnter={() => setSelectedIndex(i)}
                        >
                            {col}
                        </button>
                    ))}
                </div>
            </div>
        </div>
    );
}
