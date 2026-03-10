import React, { useRef, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Download, FileJson, FileSpreadsheet, FileText, Type } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface ExportPickerModalProps {
    onClose: () => void;
}

const EXPORT_OPTIONS = [
    { id: "csv", label: "CSV (.csv)", icon: FileText, color: "text-green-400" },
    { id: "json", label: "JSON (.json)", icon: FileJson, color: "text-yellow-400" },
    { id: "xlsx", label: "Excel (.xlsx)", icon: FileSpreadsheet, color: "text-emerald-500" },
    { id: "markdown", label: "Markdown (.md)", icon: Type, color: "text-blue-400" },
    { id: "pdf", label: "PDF (.pdf)", icon: Download, color: "text-red-400" },
];

export function ExportPickerModal({ onClose }: ExportPickerModalProps) {
    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabs = useAppStore((s) => s.tabs);
    const tabUi = useAppStore((s) => (activeTabId ? s.tabUi[activeTabId] : null));
    const activeTab = tabs.find((t) => t.id === activeTabId);

    const [selectedIndex, setSelectedIndex] = useState(0);
    const modalRef = useRef<HTMLDivElement>(null);

    useFocusTrap(modalRef);

    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if (e.key === "Escape") {
                e.stopPropagation();
                onClose();
            } else if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelectedIndex((prev) => Math.min(prev + 1, EXPORT_OPTIONS.length - 1));
            } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelectedIndex((prev) => Math.max(prev - 0, 0)); // prev - 1, bounded to 0
                setSelectedIndex((prev) => Math.max(prev - 1, 0)); // Corrected
            } else if (e.key === "Enter") {
                e.preventDefault();
                handleExport(EXPORT_OPTIONS[selectedIndex].id);
            }
        };
        const modal = modalRef.current;
        if (modal) {
            modal.addEventListener("keydown", handler);
            return () => modal.removeEventListener("keydown", handler);
        }
    }, [selectedIndex, onClose, activeTab, tabUi]);

    const handleExport = async (format: string) => {
        onClose(); // Close picker first
        if (!activeTab || !tabUi) return;
        try {
            const { save } = await import("@tauri-apps/plugin-dialog");
            const ext = format === "xlsx" ? "xlsx" : format === "markdown" ? "md" : format;
            const savePath = await save({ filters: [{ name: format.toUpperCase(), extensions: [ext] }] });
            if (!savePath) return;
            const visibleCols = activeTab.columns.filter(
                (c) => tabUi.visibleColumns[c] !== false
            );
            await invoke("export_format", {
                records: activeTab.records,
                columns: visibleCols,
                format,
                savePath,
                datasetName: activeTab.name,
            });
        } catch (err) {
            alert(`Error exportando: ${err}`);
        }
    };

    return (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center animate-fade-in" onClick={onClose}>
            <div ref={modalRef} className="panel p-0 w-80 animate-fade-in overflow-hidden flex flex-col" onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 shrink-0">
                    <div className="flex items-center gap-2">
                        <Download size={15} className="text-violet-400" />
                        <span className="text-zinc-100 font-semibold text-sm">Exportar Dataset</span>
                    </div>
                    <button className="btn ghost h-7 w-7 p-0" onClick={onClose}>
                        <X size={13} />
                    </button>
                </div>
                <div className="p-2 space-y-0.5">
                    {EXPORT_OPTIONS.map((opt, i) => {
                        const Icon = opt.icon;
                        return (
                            <button
                                key={opt.id}
                                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded text-sm transition-colors ${i === selectedIndex ? "bg-violet-600/20 text-zinc-100" : "text-zinc-300 hover:bg-zinc-800"
                                    }`}
                                onClick={() => handleExport(opt.id)}
                                onMouseEnter={() => setSelectedIndex(i)}
                            >
                                <Icon size={16} className={opt.color} />
                                {opt.label}
                            </button>
                        );
                    })}
                </div>
            </div>
        </div>
    );
}
