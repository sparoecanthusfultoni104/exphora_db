import React, { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Share2, Download, Copy, CheckCircle2 } from "lucide-react";
import { LoadedTab } from "../../types";
import { useAppStore } from "../../store/appStore";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface P2PPanelProps {
    onClose: () => void;
}

export function P2PPanel({ onClose }: P2PPanelProps) {
    const [tab, setTab] = useState<"share" | "fetch">("share");

    // Share state
    const tabs = useAppStore((s) => s.tabs);
    const activeTabId = useAppStore((s) => s.activeTabId);
    const activeTab = tabs.find((t) => t.id === activeTabId);
    const addTabs = useAppStore((s) => s.addTabs);

    const [port, setPort] = useState("9876");
    const [sharing, setSharing] = useState(false);
    const [shareLink, setShareLink] = useState("");
    const [copied, setCopied] = useState(false);

    // Fetch state
    const [fetchLink, setFetchLink] = useState("");
    const [fetching, setFetching] = useState(false);

    const panelRef = React.useRef<HTMLDivElement>(null);
    useFocusTrap(panelRef, true, true, true);

    const handleShare = async () => {
        if (!activeTab) return;
        setSharing(true);
        setShareLink("");
        try {
            const link = await invoke<string>("p2p_share", {
                name: activeTab.name,
                records: activeTab.records,
                port: parseInt(port, 10),
            });
            setShareLink(link);
        } catch (err) {
            alert(`Sharing error: ${err}`);
        } finally {
            setSharing(false);
        }
    };

    const handleFetch = async () => {
        if (!fetchLink.trim()) return;
        setFetching(true);
        try {
            const newTab = await invoke<LoadedTab>("p2p_fetch", { link: fetchLink.trim() });
            addTabs([newTab]);
            onClose();
        } catch (err) {
            alert(`Error receiving dataset: ${err}`);
        } finally {
            setFetching(false);
        }
    };

    const copyLink = async () => {
        await navigator.clipboard.writeText(shareLink);
        setCopied(true);
        setTimeout(() => setCopied(false), 2000);
    };

    return (
        <>
            <div className="sheet-overlay" onClick={onClose} />
            <div className="sheet" ref={panelRef}>
                <div className="sheet-header">
                    <div className="flex items-center gap-2 text-zinc-100 font-semibold text-sm">
                        <Share2 size={15} className="text-violet-400" />
                        P2P — Share &amp; Fetch
                    </div>
                    <button className="btn ghost h-7 w-7 p-0" onClick={onClose}><X size={14} /></button>
                </div>

                {/* Mode tabs */}
                <div className="flex border-b border-zinc-800 shrink-0">
                    {(["share", "fetch"] as const).map((m) => (
                        <button
                            key={m}
                            className={`flex-1 text-xs py-2 transition-colors ${tab === m ? "text-violet-400 border-b-2 border-violet-500" : "text-zinc-500 hover:text-zinc-300"}`}
                            onClick={() => setTab(m)}
                        >
                            {m === "share" ? "Share" : "Fetch"}
                        </button>
                    ))}
                </div>

                <div className="flex-1 overflow-y-auto p-4">
                    {tab === "share" ? (
                        <div className="space-y-4">
                            <div>
                                <label className="text-xs text-zinc-400 block mb-1">Dataset to share</label>
                                <div className="bg-zinc-800 rounded-md px-3 py-2 text-sm text-zinc-300">
                                    {activeTab?.name ?? <span className="text-zinc-600">No active tab</span>}
                                </div>
                            </div>
                            <div>
                                <label className="text-xs text-zinc-400 block mb-1">Local port</label>
                                <input type="number" className="w-full" value={port} onChange={(e) => setPort(e.target.value)} min={1024} max={65535} />
                            </div>
                            <button
                                className="btn primary w-full"
                                disabled={!activeTab || sharing}
                                onClick={handleShare}
                            >
                                {sharing ? "Generating…" : <><Share2 size={13} /> Generate link</>}
                            </button>

                            {shareLink && (
                                <div className="space-y-2">
                                    <div className="bg-zinc-800 rounded-md p-2 text-xs font-mono text-violet-300 break-all">
                                        {shareLink}
                                    </div>
                                    <button className="btn w-full" onClick={copyLink}>
                                        {copied ? <><CheckCircle2 size={13} className="text-green-400" /> Copied!</> : <><Copy size={13} /> Copy link</>}
                                    </button>
                                </div>
                            )}
                        </div>
                    ) : (
                        <div className="space-y-4">
                            <div>
                                <label className="text-xs text-zinc-400 block mb-1">Link (exphora:…)</label>
                                <textarea
                                    className="w-full bg-zinc-800 border border-zinc-700 rounded-md text-xs text-zinc-100 p-2 font-mono h-20 resize-none outline-none focus:border-violet-500 transition-colors"
                                    placeholder="exphora:nombre:..."
                                    value={fetchLink}
                                    onChange={(e) => setFetchLink(e.target.value)}
                                />
                            </div>
                            <button
                                className="btn primary w-full"
                                disabled={!fetchLink.trim() || fetching}
                                onClick={handleFetch}
                            >
                                {fetching ? "Downloading…" : <><Download size={13} /> Fetch dataset</>}
                            </button>
                        </div>
                    )}
                </div>
            </div>
        </>
    );
}
