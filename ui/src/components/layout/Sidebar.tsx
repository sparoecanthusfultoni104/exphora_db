import React from "react";
import { Clock, Eye, EyeOff } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useDataset } from "../../hooks/useDataset";

interface SidebarProps {
    isOpen: boolean;
}

export function Sidebar({ isOpen }: SidebarProps) {
    const recentFiles = useAppStore((s) => s.recentFiles);
    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabs = useAppStore((s) => s.tabs);
    const tabUi = useAppStore((s) => (activeTabId ? s.tabUi[activeTabId] : null));
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const activeTab = tabs.find((t) => t.id === activeTabId);
    const { openPath } = useDataset();

    if (!isOpen) return null;

    return (
        <div
            className="flex flex-col border-r border-zinc-800 bg-zinc-900 shrink-0 overflow-hidden"
            style={{ width: 220 }}
        >
            {/* Recents */}
            <div className="px-3 py-2 border-b border-zinc-800">
                <div className="flex items-center gap-1.5 text-zinc-400 text-xs font-semibold mb-1.5">
                    <Clock size={12} />
                    <span>Recent Files</span>
                </div>
                <div className="space-y-0.5 max-h-40 overflow-y-auto">
                    {recentFiles.length === 0 && (
                        <span className="text-zinc-600 text-xs">No recent files</span>
                    )}
                    {recentFiles.map((f) => {
                        const name = f.split(/[\\/]/).pop() ?? f;
                        return (
                            <button
                                key={f}
                                tabIndex={0}
                                className="w-full text-left text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 rounded px-1.5 py-1 truncate transition-colors focus:bg-zinc-800 focus:outline-none"
                                title={f}
                                onClick={() => openPath(f)}
                                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openPath(f); } }}
                            >
                                {name}
                            </button>
                        );
                    })}
                </div>
            </div>

            {/* Column visibility */}
            {activeTab && tabUi && (
                <div className="px-3 py-2 flex-1 overflow-y-auto">
                    <div className="flex items-center gap-1.5 text-zinc-400 text-xs font-semibold mb-1.5">
                        <Eye size={12} />
                        <span>Columns</span>
                    </div>
                    <div className="space-y-0.5">
                        {activeTab.columns.map((col) => {
                            const visible = tabUi.visibleColumns[col] !== false;
                            return (
                                <label
                                    key={col}
                                    tabIndex={0}
                                    className="flex items-center gap-2 text-xs cursor-pointer group px-1 py-0.5 rounded hover:bg-zinc-800 focus-within:bg-zinc-800 transition-colors"
                                    onKeyDown={(e) => {
                                        if (e.key === "Enter" || e.key === " ") {
                                            e.preventDefault();
                                            if (!activeTabId) return;
                                            updateTabUi(activeTabId, {
                                                visibleColumns: { ...tabUi.visibleColumns, [col]: !visible },
                                            });
                                        }
                                    }}
                                >
                                    <input
                                        type="checkbox"
                                        tabIndex={-1}
                                        checked={visible}
                                        onChange={() => {
                                            if (!activeTabId) return;
                                            updateTabUi(activeTabId, {
                                                visibleColumns: {
                                                    ...tabUi.visibleColumns,
                                                    [col]: !visible,
                                                },
                                            });
                                        }}
                                    />
                                    <span
                                        className={`truncate ${visible ? "text-zinc-300" : "text-zinc-600"}`}
                                    >
                                        {col}
                                    </span>
                                    {!visible && <EyeOff size={10} className="text-zinc-600 ml-auto shrink-0" />}
                                </label>
                            );
                        })}
                    </div>
                </div>
            )}
        </div>
    );
}
