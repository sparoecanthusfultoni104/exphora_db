import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
    FolderOpen,
    Download,
    Search,
    Sun,
    Moon,
    Settings,
    Share2,
    Check,
    AlertCircle,
    Loader2
} from "lucide-react";
import { AppInfo } from "../../types";
import { useAppStore } from "../../store/appStore";
import { useDataset } from "../../hooks/useDataset";
import { RelinkModal } from "../modals/RelinkModal";

interface TopBarProps {
    onToggleSettings: () => void;
    onToggleP2P: () => void;
}

export function TopBar({ onToggleSettings, onToggleP2P }: TopBarProps) {
    const [appInfo, setAppInfo] = useState<AppInfo>({ version: "", build_date: "" });
    const [showExport, setShowExport] = useState(false);
    const [showOpen, setShowOpen] = useState(false);
    const [relinkViewPath, setRelinkViewPath] = useState<string | null>(null);
    const theme = useAppStore((s) => s.theme);
    const setTheme = useAppStore((s) => s.setTheme);
    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabs = useAppStore((s) => s.tabs);
    const tabUi = useAppStore((s) => (activeTabId ? s.tabUi[activeTabId] : null));
    const activeTab = tabs.find((t) => t.id === activeTabId);
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const { openFile } = useDataset();

    useEffect(() => {
        invoke<AppInfo>("get_app_info").then(setAppInfo).catch(console.error);
    }, []);

    // Keyboard shortcuts
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key === "o") {
                e.preventDefault();
                openFile();
            }
            if ((e.ctrlKey || e.metaKey) && e.key === "f") {
                e.preventDefault();
                document.getElementById("global-search")?.focus();
            }
        };
        window.addEventListener("keydown", handler);
        return () => window.removeEventListener("keydown", handler);
    }, [openFile]);

    const handleExport = async (format: string) => {
        setShowExport(false);
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
            alert(`Export error: ${err}`);
        }
    };

    const handleLoadView = async (path: string) => {
        try {
            const viewFile: any = await invoke("load_view", { filePath: path });
            const newTabs = await invoke<any[]>("load_file", { path: viewFile.view.datasetPath });
            if (newTabs.length === 0) return;
            useAppStore.getState().addTabs(newTabs);
            const { fromViewState } = await import("../../types");
            useAppStore.getState().updateTabUi(newTabs[0].id, {
                ...fromViewState(viewFile.view),
                savedViewPath: viewFile.saved_path || undefined,
                viewNotes: viewFile.viewNotes || "",
                columnNotes: viewFile.columnNotes || {}
            });
            
            const name = path.split(/[\\/]/).pop()?.replace(/\.[^/.]+$/, "") || "Unknown";
            useAppStore.getState().addRecentView({
                name,
                path,
                datasetPath: viewFile.view.datasetPath,
                openedAt: new Date().toISOString()
            });
        } catch (err: any) {
            const errStr = typeof err === "string" ? err : JSON.stringify(err);
            if (errStr.includes("DATASET_NOT_FOUND")) {
                setRelinkViewPath(path);
            } else {
                alert(`Error loading view: ${errStr}`);
            }
        }
    };

    return (
        <div className="flex items-center gap-2 px-3 py-1.5 border-b border-zinc-800 bg-zinc-900 select-none shrink-0" style={{ height: 46 }}>
            {/* App title */}
            <div className="flex items-center gap-2 mr-2">
                <div className="w-6 h-6 rounded-md bg-violet-600 flex items-center justify-center">
                    <span className="text-white text-xs font-bold">E</span>
                </div>
                <div className="flex flex-col leading-none">
                    <span className="text-zinc-100 font-semibold text-sm">Exphora DB --Dev</span>
                    {appInfo.version && (
                        <span className="text-zinc-500 text-[10px]">
                            {appInfo.build_date}
                        </span>
                    )}
                </div>
            </div>

            {/* Open Dropdown */}
            <div className="relative">
                <button className="btn" tabIndex={0} onClick={() => setShowOpen((v) => !v)} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") setShowOpen((v) => !v); }} title="Open (Ctrl+O)">
                    <FolderOpen size={14} />
                    <span>Open</span>
                </button>
                {showOpen && (
                    <div
                        className="context-menu absolute top-full left-0 mt-1 animate-fade-in"
                        onMouseLeave={() => setShowOpen(false)}
                    >
                        <div className="context-menu-item" onClick={() => { setShowOpen(false); openFile(); }}>
                            Open file
                        </div>
                        <div className="context-menu-item" onClick={async () => {
                            setShowOpen(false);
                            try {
                                const { open } = await import("@tauri-apps/plugin-dialog");
                                const path = await open({ filters: [{ name: "Exphora View", extensions: ["exh"] }] });
                                if (typeof path === "string") {
                                    handleLoadView(path);
                                }
                            } catch (e) {
                                alert(e);
                            }
                        }}>
                            Open view (.exh)
                        </div>
                    </div>
                )}
            </div>

            {/* Export dropdown */}
            {activeTab && (
                <div className="relative">
                    <button className="btn" tabIndex={0} onClick={() => setShowExport((v) => !v)} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") setShowExport((v) => !v); }}>
                        <Download size={14} />
                        <span>Export</span>
                    </button>
                    {showExport && (
                        <div
                            className="context-menu absolute top-full left-0 mt-1 animate-fade-in"
                            onMouseLeave={() => setShowExport(false)}
                        >
                            {["csv", "json", "xlsx", "markdown", "pdf"].map((fmt) => (
                                <div
                                    key={fmt}
                                    className="context-menu-item"
                                    onClick={() => handleExport(fmt)}
                                >
                                    {fmt.toUpperCase()}
                                </div>
                            ))}
                        </div>
                    )}
                </div>
            )}

            {/* Spacer */}
            <div className="flex-1" />

            {/* Save Status Output */}
            {activeTabId && tabUi?.saveStatus && tabUi.saveStatus !== 'idle' && (
                <div className="flex items-center gap-1.5 mr-2 text-xs">
                    {tabUi.saveStatus === 'saving' && (
                        <>
                            <Loader2 size={13} className="animate-spin text-zinc-500" />
                            <span className="text-zinc-500">Saving...</span>
                        </>
                    )}
                    {tabUi.saveStatus === 'saved' && (
                        <>
                            <Check size={13} className="text-emerald-500" />
                            <span className="text-emerald-500">Saved</span>
                        </>
                    )}
                    {tabUi.saveStatus === 'error' && (
                        <>
                            <AlertCircle size={13} className="text-rose-500" />
                            <span className="text-rose-500">Save failed</span>
                        </>
                    )}
                </div>
            )}

            {/* Global search */}
            {activeTabId && (
                <div className="flex items-center gap-1 bg-zinc-800 border border-zinc-700 rounded-md px-2" style={{ height: 28 }}>
                    <Search size={12} className="text-zinc-500 shrink-0" />
                    <input
                        id="global-search"
                        type="text"
                        tabIndex={0}
                        placeholder="Search…  (Ctrl+F)"
                        className="bg-transparent border-none text-xs text-zinc-100 placeholder-zinc-500 outline-none w-48"
                        value={tabUi?.textSearch ?? ""}
                        onChange={(e) => {
                            if (!activeTabId) return;
                            updateTabUi(activeTabId, { textSearch: e.target.value });
                        }}
                    />
                </div>
            )}

            {/* P2P */}
            <button className="btn ghost" tabIndex={0} onClick={onToggleP2P} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onToggleP2P(); }} title="P2P">
                <Share2 size={14} />
            </button>

            {/* Theme */}
            <button
                className="btn ghost"
                tabIndex={0}
                onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
                onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") setTheme(theme === "dark" ? "light" : "dark"); }}
                title="Toggle theme"
            >
                {theme === "dark" ? <Sun size={14} /> : <Moon size={14} />}
            </button>

            {/* Settings */}
            <button className="btn ghost" tabIndex={0} onClick={onToggleSettings} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") onToggleSettings(); }} title="Settings">
                <Settings size={14} />
            </button>

            {relinkViewPath && (
                <RelinkModal
                    viewFilePath={relinkViewPath}
                    onClose={() => setRelinkViewPath(null)}
                    onRelinkSuccess={(viewFile) => {
                        setRelinkViewPath(null);
                        if (relinkViewPath) handleLoadView(relinkViewPath);
                    }}
                />
            )}
        </div>
    );
}
