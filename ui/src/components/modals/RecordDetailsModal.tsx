import React, { useRef, useState } from "react";
import { X, Search } from "lucide-react";
import { LoadedTab } from "../../types";
import { useFocusTrap } from "../../hooks/useFocusTrap";

interface RecordDetailsModalProps {
    tab: LoadedTab;
    recordIdx: number;
    onClose: () => void;
}

export function RecordDetailsModal({ tab, recordIdx, onClose }: RecordDetailsModalProps) {
    const [search, setSearch] = useState("");
    const modalRef = useRef<HTMLDivElement>(null);
    useFocusTrap(modalRef, true, true, true);

    const record = tab.records[recordIdx] as Record<string, unknown>;
    const cols = tab.columns.filter((c) => c.toLowerCase().includes(search.toLowerCase()));

    const isNumericCell = (val: string) => val !== "" && !isNaN(parseFloat(val));

    return (
        <div
            className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center animate-fade-in"
            onClick={onClose}
            onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
        >
            <div
                ref={modalRef}
                className="panel p-0 w-[500px] h-[600px] max-h-[80vh] flex flex-col animate-fade-in overflow-hidden"
                onClick={(e) => e.stopPropagation()}
            >
                {/* Header */}
                <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 bg-zinc-900/50">
                    <span className="text-zinc-100 font-semibold text-sm">
                        Record Details <span className="text-zinc-500 font-normal ml-1">#{recordIdx + 1}</span>
                    </span>
                    <button className="btn ghost h-7 w-7 p-0" onClick={onClose}><X size={13} /></button>
                </div>

                {/* Search */}
                <div className="p-3 border-b border-zinc-800 bg-zinc-900/30">
                    <div className="flex items-center gap-2 bg-zinc-800/50 border border-zinc-700/50 rounded-md px-2 py-1.5 focus-within:border-violet-500/50 transition-colors">
                        <Search size={14} className="text-zinc-500 shrink-0" />
                        <input
                            type="text"
                            placeholder="Filter data..."
                            autoFocus
                            className="bg-transparent border-none text-xs text-zinc-100 placeholder-zinc-500 outline-none w-full"
                            value={search}
                            onChange={(e) => setSearch(e.target.value)}
                        />
                    </div>
                </div>

                {/* Content */}
                <div className="flex-1 overflow-y-auto w-full">
                    {cols.length === 0 ? (
                        <div className="p-8 text-center text-zinc-500 text-sm">No fields match your search</div>
                    ) : (
                        <table className="w-full text-left border-collapse mt-1">
                            <tbody>
                                {cols.map((col, idx) => {
                                    const rawVal = record[col];
                                    const strVal = (rawVal === null || rawVal === undefined) ? "" : String(rawVal);
                                    const numeric = isNumericCell(strVal);

                                    return (
                                        <tr key={col} className={`group border-b border-zinc-800/30 ${idx % 2 === 0 ? 'bg-transparent' : 'bg-zinc-800/10'} hover:bg-zinc-800/40 transition-colors`}>
                                            <td className="py-2 pl-4 pr-3 text-xs font-semibold text-zinc-400 w-1/3 align-top border-r border-zinc-800/30 break-all select-all">
                                                {col}
                                            </td>
                                            <td className="py-2 px-4 text-xs select-all">
                                                {strVal === "" ? (
                                                    <span className="text-zinc-600 italic">null</span>
                                                ) : (
                                                    <span className={`text-zinc-200 break-all ${numeric ? 'font-mono text-violet-300' : ''}`}>
                                                        {strVal}
                                                    </span>
                                                )}
                                            </td>
                                        </tr>
                                    );
                                })}
                            </tbody>
                        </table>
                    )}
                </div>
            </div>
        </div>
    );
}
