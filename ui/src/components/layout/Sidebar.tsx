import React, { useEffect, useState } from "react";
import { Clock, Eye, EyeOff, LayoutTemplate, ChevronDown, History } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useDataset } from "../../hooks/useDataset";
import { invoke } from "@tauri-apps/api/core";
import { fromViewState } from "../../types";
import { RelinkModal } from "../modals/RelinkModal";

interface SidebarProps {
    isOpen: boolean;
}

export function Sidebar({ isOpen }: SidebarProps) {
    const recentFiles = useAppStore((s) => s.recentFiles);
    const recentViews = useAppStore((s) => s.recentViews);
    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabs = useAppStore((s) => s.tabs);
    const tabUi = useAppStore((s) => (activeTabId ? s.tabUi[activeTabId] : null));
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const addTabs = useAppStore((s) => s.addTabs);
    const addRecentView = useAppStore((s) => s.addRecentView);
    const activeTab = tabs.find((t) => t.id === activeTabId);
    const { openPath } = useDataset();

    const [savedViews, setSavedViews] = useState<any[]>([]);
    const [relinkViewPath, setRelinkViewPath] = useState<string | null>(null);

    const [collapsed, setCollapsed] = useState<{
        recentFiles: boolean; columns: boolean; views: boolean; recentViews: boolean;
    }>(() => {
        try {
            const stored = localStorage.getItem("sidebar_collapsed_sections");
            const parsed = stored ? JSON.parse(stored) : {};
            return { recentFiles: false, columns: false, views: false, recentViews: false, ...parsed };
        } catch {
            return { recentFiles: false, columns: false, views: false, recentViews: false };
        }
    });

    const toggleSection = (key: "recentFiles" | "columns" | "views" | "recentViews") => {
        setCollapsed((prev) => {
            const next = { ...prev, [key]: !prev[key] };
            localStorage.setItem("sidebar_collapsed_sections", JSON.stringify(next));
            return next;
        });
    };

    useEffect(() => {
        const loadViews = () => {
            const stored = localStorage.getItem("exphora_saved_views");
            if (stored) setSavedViews(JSON.parse(stored));
        };
        loadViews();
        window.addEventListener("exphora-views-updated", loadViews);
        return () => window.removeEventListener("exphora-views-updated", loadViews);
    }, []);

    const handleLoadView = async (path: string, name?: string) => {
        try {
            const viewFile: any = await invoke("load_view", { filePath: path });
            const newTabs = await invoke<any[]>("load_file", { path: viewFile.view.datasetPath });
            if (newTabs.length === 0) return;
            addTabs(newTabs);
            updateTabUi(newTabs[0].id, { 
                ...fromViewState(viewFile.view), 
                savedViewPath: viewFile.saved_path || undefined,
                viewNotes: viewFile.viewNotes || "",
                columnNotes: viewFile.columnNotes || {}
            });
            
            if (name) {
                addRecentView({
                    name,
                    path,
                    datasetPath: viewFile.view.datasetPath,
                    openedAt: new Date().toISOString()
                });
            }
        } catch (err: any) {
            const errStr = typeof err === "string" ? err : JSON.stringify(err);
            if (errStr.includes("DATASET_NOT_FOUND")) {
                setRelinkViewPath(path);
            } else {
                alert(`Error loading view: ${errStr}`);
            }
        }
    };

    if (!isOpen) return null;

    return (
        <div
            className="flex flex-col border-r border-zinc-800 bg-zinc-900 shrink-0 overflow-hidden"
            style={{ width: 220 }}
        >
            {/* Recents */}
            <div className="px-3 py-2 border-b border-zinc-800">
                <button
                    className="w-full flex items-center justify-between text-zinc-400 text-xs font-semibold mb-1.5 hover:text-zinc-200 hover:bg-zinc-800 focus:bg-zinc-800 focus:outline-none px-1 rounded transition-colors group"
                    onClick={() => toggleSection("recentFiles")}
                >
                    <div className="flex items-center gap-1.5 py-1">
                        <Clock size={12} />
                        <span>Recent Files</span>
                    </div>
                    <ChevronDown size={14} className={`transform transition-transform duration-300 ${collapsed.recentFiles ? "-rotate-90" : ""}`} />
                </button>
                <div 
                    className="grid transition-all duration-300 ease-in-out" 
                    style={{ gridTemplateRows: collapsed.recentFiles ? "0fr" : "1fr" }}
                >
                    <div className="overflow-hidden space-y-0.5">
                        <div className="max-h-40 overflow-y-auto">
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
                </div>
            </div>

            {/* Recent Views */}
            <div className="px-3 py-2 border-b border-zinc-800">
                <button
                    className="w-full flex items-center justify-between text-zinc-400 text-xs font-semibold mb-1.5 hover:text-zinc-200 hover:bg-zinc-800 focus:bg-zinc-800 focus:outline-none px-1 rounded transition-colors group"
                    onClick={() => toggleSection("recentViews")}
                >
                    <div className="flex items-center gap-1.5 py-1">
                        <History size={12} />
                        <span>Recent Views</span>
                    </div>
                    <ChevronDown size={14} className={`transform transition-transform duration-300 ${collapsed.recentViews ? "-rotate-90" : ""}`} />
                </button>
                <div 
                    className="grid transition-all duration-300 ease-in-out" 
                    style={{ gridTemplateRows: collapsed.recentViews ? "0fr" : "1fr" }}
                >
                    <div className="overflow-hidden space-y-0.5">
                        <div className="max-h-40 overflow-y-auto">
                            {recentViews.length === 0 && (
                                <span className="text-zinc-600 text-xs">No recent views</span>
                            )}
                            {recentViews.map((entry, i) => {
                                const fileName = entry.datasetPath.split(/[\\/]/).pop()?.replace(/\.[^/.]+$/, "") ?? entry.datasetPath;
                                return (
                                    <button
                                        key={i}
                                        tabIndex={0}
                                        className="w-full text-left text-xs hover:bg-zinc-800 rounded px-1.5 py-1 transition-colors focus:bg-zinc-800 focus:outline-none flex flex-col gap-0.5"
                                        title={entry.path}
                                        onClick={() => handleLoadView(entry.path, entry.name)}
                                    >
                                        <span className="text-zinc-300 font-medium truncate w-full">{entry.name}</span>
                                        <span className="text-zinc-500 truncate w-full" style={{ fontSize: "10px" }}>{fileName}</span>
                                    </button>
                                );
                            })}
                        </div>
                    </div>
                </div>
            </div>

            {/* Column visibility */}
            {activeTab && tabUi && (
                <div className="px-3 py-2 flex-1 overflow-y-auto min-h-0">
                    <button
                        className="w-full flex items-center justify-between text-zinc-400 text-xs font-semibold mb-1.5 hover:text-zinc-200 hover:bg-zinc-800 focus:bg-zinc-800 focus:outline-none px-1 rounded transition-colors group"
                        onClick={() => toggleSection("columns")}
                    >
                        <div className="flex items-center gap-1.5 py-1">
                            <Eye size={12} />
                            <span>Columns</span>
                        </div>
                        <ChevronDown size={14} className={`transform transition-transform duration-300 ${collapsed.columns ? "-rotate-90" : ""}`} />
                    </button>
                    <div 
                        className="grid transition-all duration-300 ease-in-out" 
                        style={{ gridTemplateRows: collapsed.columns ? "0fr" : "1fr" }}
                    >
                        <div className="overflow-hidden space-y-0.5">
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
                </div>
            )}

            {/* Vistas */}
            <div className="px-3 py-2 flex-1 overflow-y-auto border-t border-zinc-800 min-h-0">
                <button
                    className="w-full flex items-center justify-between text-zinc-400 text-xs font-semibold mb-1.5 hover:text-zinc-200 hover:bg-zinc-800 focus:bg-zinc-800 focus:outline-none px-1 rounded transition-colors group"
                    onClick={() => toggleSection("views")}
                >
                    <div className="flex items-center gap-1.5 py-1">
                        <LayoutTemplate size={12} />
                        <span>Views</span>
                    </div>
                    <ChevronDown size={14} className={`transform transition-transform duration-300 ${collapsed.views ? "-rotate-90" : ""}`} />
                </button>
                <div 
                    className="grid transition-all duration-300 ease-in-out" 
                    style={{ gridTemplateRows: collapsed.views ? "0fr" : "1fr" }}
                >
                    <div className="overflow-hidden space-y-0.5">
                        {savedViews.length === 0 && (
                            <span className="text-zinc-600 text-xs">No saved views</span>
                        )}
                        {savedViews.map((item, i) => {
                            const fileName = item.datasetPath.split(/[\\/]/).pop() ?? item.datasetPath;
                            return (
                                <button
                                    key={i}
                                    tabIndex={0}
                                    className="w-full text-left text-xs hover:bg-zinc-800 rounded px-1.5 py-1 transition-colors focus:bg-zinc-800 focus:outline-none flex flex-col gap-0.5"
                                    title={item.path}
                                    onClick={() => handleLoadView(item.path, item.name)}
                                >
                                    <span className="text-zinc-300 font-medium truncate w-full">{item.name}</span>
                                    <span className="text-zinc-500 truncate w-full" style={{ fontSize: "10px" }}>{fileName}</span>
                                </button>
                            );
                        })}
                    </div>
                </div>
            </div>

            {relinkViewPath && (
                <RelinkModal
                    viewFilePath={relinkViewPath}
                    onClose={() => setRelinkViewPath(null)}
                    onRelinkSuccess={(viewFile) => {
                        setRelinkViewPath(null);
                        const fallbackName = relinkViewPath?.split(/[\\/]/).pop()?.replace(/\.[^/.]+$/, "") ?? "Unknown";
                        if (relinkViewPath) handleLoadView(relinkViewPath, fallbackName);
                    }}
                />
            )}
        </div>
    );
}
