import React, { useEffect } from "react";
import { X } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useDataset } from "../../hooks/useDataset";

export function TabBar() {
    const tabs = useAppStore((s) => s.tabs);
    const activeTabId = useAppStore((s) => s.activeTabId);
    const setActiveTab = useAppStore((s) => s.setActiveTab);
    const { closeTab } = useDataset();

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
    );
}
