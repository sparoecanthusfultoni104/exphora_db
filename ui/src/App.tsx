import React, { useState, useEffect } from "react";
import { TopBar } from "./components/layout/TopBar";
import { Sidebar } from "./components/layout/Sidebar";
import { TabBar } from "./components/layout/TabBar";
import { DataTable } from "./components/table/DataTable";
import { StatsPanel } from "./components/stats/StatsPanel";
import { FrequencyChart } from "./components/charts/FrequencyChart";
import { P2PPanel } from "./components/p2p/P2PPanel";
import { SettingsWindow } from "./components/settings/SettingsWindow";
import { ColumnPickerModal } from "./components/modals/ColumnPickerModal";
import { ExportPickerModal } from "./components/modals/ExportPickerModal";
import { useAppStore } from "./store/appStore";
import { useDataset } from "./hooks/useDataset";

export default function App() {
    const [showSettings, setShowSettings] = useState(false);
    const [showP2P, setShowP2P] = useState(false);
    const [showSidebar, setShowSidebar] = useState(true);
    const [showColumnPicker, setShowColumnPicker] = useState<false | { action: "filter" | "stats" | "chart" }>(false);
    const [showExportPicker, setShowExportPicker] = useState(false);

    const activeTabId = useAppStore((s) => s.activeTabId);
    const tabs = useAppStore((s) => s.tabs);
    const setActiveTab = useAppStore((s) => s.setActiveTab);
    const recentFiles = useAppStore((s) => s.recentFiles);
    const ui = useAppStore((s) => (activeTabId ? s.tabUi[activeTabId] : null));
    const updateTabUi = useAppStore((s) => s.updateTabUi);
    const theme = useAppStore((s) => s.theme);
    const setTheme = useAppStore((s) => s.setTheme);
    const { openFile, openPath } = useDataset();

    // Apply theme class to document
    useEffect(() => {
        document.documentElement.setAttribute("data-theme", theme);
    }, [theme]);

    // Global keyboard shortcuts
    useEffect(() => {
        const handler = (e: KeyboardEvent) => {
            const el = document.activeElement as HTMLElement;
            const isInput = el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);

            if (e.key === "Escape") {
                setShowSettings(false);
                setShowP2P(false);
                setShowColumnPicker(false);
                setShowExportPicker(false);
                if (activeTabId && ui) {
                    if (ui.activeStats) updateTabUi(activeTabId, { activeStats: null, activeStatsCol: null });
                    if (ui.showFrequencyChart) updateTabUi(activeTabId, { showFrequencyChart: false });
                }
                return;
            }

            if (isInput) return;

            if (e.ctrlKey || e.metaKey) {
                const key = e.key.toLowerCase();

                // Tabs Navigation
                if (key === "tab") {
                    e.preventDefault();
                    if (tabs.length > 1) {
                        const idx = tabs.findIndex((t) => t.id === activeTabId);
                        if (e.shiftKey) {
                            const next = idx > 0 ? idx - 1 : tabs.length - 1;
                            setActiveTab(tabs[next].id);
                        } else {
                            const next = idx < tabs.length - 1 ? idx + 1 : 0;
                            setActiveTab(tabs[next].id);
                        }
                    }
                } else if (key >= "1" && key <= "9") {
                    e.preventDefault();
                    const idx = parseInt(key, 10) - 1;
                    if (idx < tabs.length) {
                        setActiveTab(tabs[idx].id);
                    }
                }

                // Actions
                else if (key === "o") {
                    e.preventDefault();
                    openFile();
                } else if (key === "r") {
                    e.preventDefault();
                    if (activeTabId) {
                        const tab = tabs.find(t => t.id === activeTabId);
                        if (tab) {
                            const stem = tab.name.split("/")[0];
                            const path = recentFiles.find(f => {
                                const fName = f.split(/[\\/]/).pop() || "";
                                return fName === stem || fName.startsWith(stem + ".");
                            });
                            if (path) openPath(path);
                        }
                    }
                } else if (key === "f" && !e.shiftKey) {
                    e.preventDefault();
                    document.getElementById("global-search")?.focus();
                } else if (key === "c" && e.shiftKey) {
                    e.preventDefault();
                    window.dispatchEvent(new CustomEvent("exphoradb:clear-filters"));
                } else if (key === "f" && e.shiftKey) {
                    e.preventDefault();
                    setShowColumnPicker({ action: "filter" });
                } else if (key === "s" && e.shiftKey) {
                    e.preventDefault();
                    setShowColumnPicker({ action: "stats" });
                } else if (key === "g" && e.shiftKey) {
                    e.preventDefault();
                    setShowColumnPicker({ action: "chart" });
                } else if (key === "e") {
                    e.preventDefault();
                    setShowExportPicker(true);
                } else if (key === "d") {
                    e.preventDefault();
                    setTheme(theme === "dark" ? "light" : "dark");
                } else if (e.key === ",") {
                    e.preventDefault();
                    setShowSettings(true);
                } else if (key === "p") {
                    e.preventDefault();
                    setShowP2P(true);
                }
            }
        };
        window.addEventListener("keydown", handler);
        return () => window.removeEventListener("keydown", handler);
    }, [activeTabId, ui, updateTabUi, tabs, setActiveTab, openFile, openPath, recentFiles, theme, setTheme]);

    return (
        <div className="flex flex-col h-full overflow-hidden" style={{ background: "var(--color-app-bg)", color: "var(--color-text-primary)" }}>
            {/* Top bar */}
            <TopBar
                onToggleSettings={() => setShowSettings((v) => !v)}
                onToggleP2P={() => setShowP2P((v) => !v)}
            />

            {/* Tab bar */}
            <TabBar />

            {/* Main area */}
            <div className="flex flex-1 overflow-hidden">
                {/* Sidebar */}
                <Sidebar isOpen={showSidebar} />

                {/* Content area */}
                <div className="flex-1 flex flex-col overflow-hidden relative">
                    {tabs.length === 0 ? (
                        /* Empty state */
                        <div className="flex flex-col items-center justify-center flex-1 gap-4">
                            <div className="w-16 h-16 rounded-2xl bg-violet-600/20 flex items-center justify-center">
                                <span className="text-violet-400 text-3xl">⊞</span>
                            </div>
                            <div className="text-center">
                                <h2 className="text-zinc-300 text-xl font-semibold mb-1">ExphoraDB</h2>
                                <p className="text-zinc-500 text-sm">Open a file to start</p>
                                <p className="text-zinc-600 text-xs mt-1">JSON · CSV · NDJSON · XML · SQLite</p>
                            </div>
                            <button className="btn primary" onClick={openFile}>
                                Open file  <span className="text-violet-200 text-xs">(Ctrl+O)</span>
                            </button>
                        </div>
                    ) : activeTabId ? (
                        <DataTable key={activeTabId} tabId={activeTabId} />
                    ) : null}
                </div>
            </div>

            {/* Overlay panels */}
            {activeTabId && ui && (
                <>
                    {ui.activeStats && <StatsPanel tabId={activeTabId} />}
                    {ui.showFrequencyChart && ui.frequencyChartCol && (
                        <FrequencyChart
                            tabId={activeTabId}
                            col={ui.frequencyChartCol}
                            onClose={() => updateTabUi(activeTabId, { showFrequencyChart: false })}
                        />
                    )}
                </>
            )}

            {showP2P && <P2PPanel onClose={() => setShowP2P(false)} />}
            {showSettings && <SettingsWindow onClose={() => setShowSettings(false)} />}
            {showColumnPicker && <ColumnPickerModal action={showColumnPicker.action} onClose={() => setShowColumnPicker(false)} />}
            {showExportPicker && <ExportPickerModal onClose={() => setShowExportPicker(false)} />}
        </div>
    );
}
