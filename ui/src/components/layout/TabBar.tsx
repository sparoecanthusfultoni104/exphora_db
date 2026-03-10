import React, { useEffect, useState, useRef } from "react";
import { X, Save, ChevronDown } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useDataset } from "../../hooks/useDataset";
import { toViewState } from "../../types";
import { invoke } from "@tauri-apps/api/core";

export function TabBar() {
    const tabs = useAppStore((s) => s.tabs);
    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabUiStateMap = useAppStore((s) => s.tabUi);
    const setActiveTab = useAppStore((s) => s.setActiveTab);
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const { closeTab } = useDataset();

    const [saveLabel, setSaveLabel] = useState<string>("Save view");
    const [toastMsg, setToastMsg] = useState<string | null>(null);

    const savedNotesRef = useRef<Record<string, string>>({});

    useEffect(() => {
        const activeIds = new Set(tabs.map(t => t.id));
        tabs.forEach(tab => {
            const ui = tabUiStateMap[tab.id];
            if (ui && !(tab.id in savedNotesRef.current)) {
                savedNotesRef.current[tab.id] = ui.notes || "";
            }
        });
        Object.keys(savedNotesRef.current).forEach(id => {
            if (!activeIds.has(id)) delete savedNotesRef.current[id];
        });
    }, [tabs, tabUiStateMap]);

    // Ctrl+W closes active tab
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            if ((e.ctrlKey || e.metaKey) && e.key === "w") {
                e.preventDefault();
                if (activeTabId) closeTab(activeTabId);
            }
        };
        window.addEventListener("keydown", handler);
        return () => window.removeEventListener("keydown", handler);
    }, [activeTabId, closeTab]);

    if (tabs.length === 0) return null;

    const handleSaveView = async (saveAs: boolean = false) => {
        if (!activeTabId) return;
        const activeTab = tabs.find(t => t.id === activeTabId);
        const activeUi = useAppStore.getState().tabUi[activeTabId]; // Fetch live state, bypass closure
        if (!activeTab || !activeUi) return;

        let viewState;
        try {
            viewState = toViewState(activeTab, activeUi);
        } catch (err: any) {
            setToastMsg(err.message || String(err));
            setTimeout(() => setToastMsg(null), 3000);
            return;
        }

        try {
            setSaveLabel("Saving...");
            const targetPath = (!saveAs && activeUi.savedViewPath) ? activeUi.savedViewPath : null;

            const savedPath = await invoke<string>("save_view", {
                tabId: activeTabId,
                viewName: activeTab.name,
                view: viewState,
                path: targetPath
            });

            updateTabUi(activeTabId, { savedViewPath: savedPath });
            savedNotesRef.current[activeTabId] = activeUi.notes || "";

            const savedViewsJson = localStorage.getItem("exphora_saved_views");
            let savedViews: any[] = savedViewsJson ? JSON.parse(savedViewsJson) : [];
            savedViews.unshift({
                name: activeTab.name,
                path: savedPath,
                datasetPath: viewState.datasetPath,
                savedAt: new Date().toISOString()
            });
            savedViews = savedViews.slice(0, 20);
            localStorage.setItem("exphora_saved_views", JSON.stringify(savedViews));
            window.dispatchEvent(new Event("exphora-views-updated"));

            setSaveLabel("View saved");
            setTimeout(() => setSaveLabel("Save view"), 2000);
        } catch (err) {
            if (err !== "Dialog cancelled") {
                setToastMsg(`Error: ${err}`);
                setTimeout(() => setToastMsg(null), 3000);
            }
            setSaveLabel("Save view");
        }
    };

    const handleKeyDown = (e: React.KeyboardEvent, index: number) => {
        if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setActiveTab(tabs[index].id);
        } else if (e.key === "ArrowRight") {
            e.preventDefault();
            const nextElem = e.currentTarget.nextElementSibling as HTMLElement;
            if (nextElem) nextElem.focus();
        } else if (e.key === "ArrowLeft") {
            e.preventDefault();
            const prevElem = e.currentTarget.previousElementSibling as HTMLElement;
            if (prevElem) prevElem.focus();
        }
    };

    return (
        <div
            className="flex items-end gap-0.5 px-2 shrink-0 border-b border-zinc-800 bg-zinc-950 overflow-x-auto"
            style={{ height: 34, minHeight: 34 }}
        >
            <div className="flex items-end gap-0.5 flex-1">
                {tabs.map((tab, i) => {
                    const isActive = tab.id === activeTabId;
                    return (
                        <div
                            key={tab.id}
                            tabIndex={0}
                            className={`tab-pill ${isActive ? "active" : ""} focus:outline-none focus:ring-1 focus:ring-violet-500`}
                            onClick={() => setActiveTab(tab.id)}
                            onKeyDown={(e) => handleKeyDown(e, i)}
                        >
                            <span className="truncate" style={{ maxWidth: 140 }} title={tab.name}>
                                {tab.name}
                            </span>
                            <button
                                tabIndex={-1}
                                className="ml-1 rounded hover:bg-zinc-700 p-0.5 transition-colors"
                                onClick={(e) => {
                                    e.stopPropagation();
                                    closeTab(tab.id);
                                }}
                            >
                                <X size={10} />
                            </button>
                        </div>
                    );
                })}
            </div>

            {activeTabId && (
                <div className="flex items-center ml-2 mb-1 gap-2">
                    {toastMsg && (
                        <span className="text-xs text-rose-400 bg-rose-400/10 px-2 py-0.5 rounded animate-fade-in">
                            {toastMsg}
                        </span>
                    )}
                    {(() => {
                        const activeUi = tabUiStateMap[activeTabId];
                        const savedNotes = savedNotesRef.current[activeTabId] || "";
                        const currentNotes = activeUi?.notes || "";
                        const hasUnsavedNotes = currentNotes.trim() !== savedNotes.trim();

                        return (
                            <div className="flex bg-violet-500/10 border border-violet-500/20 rounded-md overflow-hidden transition-colors hover:bg-violet-500/20">
                                <button
                                    onClick={() => handleSaveView(false)}
                                    className="flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium text-violet-300 relative"
                                >
                                    {hasUnsavedNotes && <span className="absolute left-1 top-1 w-1.5 h-1.5 rounded-full bg-orange-400" />}
                                    <Save size={12} />
                                    {saveLabel}
                                </button>
                                {activeUi?.savedViewPath && (
                                    <button
                                        onClick={() => handleSaveView(true)}
                                        title="Save as..."
                                        className="flex items-center justify-center px-1.5 py-1 text-violet-300 border-l border-violet-500/20 hover:bg-violet-500/30 transition-colors"
                                    >
                                        <ChevronDown size={12} />
                                    </button>
                                )}
                            </div>
                        );
                    })()}
                </div>
            )}
        </div>
    );
}
