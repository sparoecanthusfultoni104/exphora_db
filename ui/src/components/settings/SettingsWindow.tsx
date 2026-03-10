import React from "react";
import { X, Trash2 } from "lucide-react";
import { useAppStore } from "../../store/appStore";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface SettingsWindowProps {
    onClose: () => void;
}

export function SettingsWindow({ onClose }: SettingsWindowProps) {
    const theme = useAppStore((s) => s.theme);
    const setTheme = useAppStore((s) => s.setTheme);
    const recentFiles = useAppStore((s) => s.recentFiles);

    const modalRef = React.useRef<HTMLDivElement>(null);
    useFocusTrap(modalRef, true, true, true);

    return (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center animate-fade-in" onClick={onClose}>
            <div ref={modalRef} className="panel p-0 w-80 animate-fade-in overflow-hidden" onClick={(e) => e.stopPropagation()}>
                <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800">
                    <span className="text-zinc-100 font-semibold text-sm">Settings</span>
                    <button className="btn ghost h-7 w-7 p-0" onClick={onClose}><X size={13} /></button>
                </div>

                <div className="p-4 space-y-4">
                    {/* Theme */}
                    <div>
                        <label className="text-xs text-zinc-400 font-semibold block mb-2">Theme</label>
                        <div className="flex gap-2">
                            {(["dark", "light"] as const).map((t) => (
                                <button
                                    key={t}
                                    className={`btn flex-1 ${theme === t ? "primary" : ""}`}
                                    onClick={() => setTheme(t)}
                                >
                                    {t === "dark" ? "🌙 Dark" : "☀️ Light"}
                                </button>
                            ))}
                        </div>
                    </div>

                    {/* Recent files */}
                    <div>
                        <div className="flex items-center justify-between mb-2">
                            <label className="text-xs text-zinc-400 font-semibold">Recent files</label>
                            <span className="badge">{recentFiles.length}</span>
                        </div>
                        <button
                            className="btn danger w-full text-xs h-7"
                            onClick={() => useAppStore.setState({ recentFiles: [] })}
                            disabled={recentFiles.length === 0}
                        >
                            <Trash2 size={12} /> Clear history
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
