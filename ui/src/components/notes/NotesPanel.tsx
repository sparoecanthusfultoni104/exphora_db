import React, { useState, useRef, useEffect } from "react";
import MDEditor from "@uiw/react-md-editor";

interface NotesPanelProps {
    tabId: string;
    notes: string;
    onChange: (value: string) => void;
}

export function NotesPanel({ tabId, notes, onChange }: NotesPanelProps) {
    const [mode, setMode] = useState<"edit" | "split" | "preview">("split");
    const [height, setHeight] = useState(240);

    const handleMouseDown = (e: React.MouseEvent) => {
        e.preventDefault();
        const startY = e.clientY;
        const startHeight = height;

        const onMouseMove = (moveEvent: MouseEvent) => {
            const dy = startY - moveEvent.clientY;
            let newHeight = startHeight + dy;
            
            const minHeight = 160;
            const maxHeight = window.innerHeight * 0.6;
            
            if (newHeight < minHeight) newHeight = minHeight;
            if (newHeight > maxHeight) newHeight = maxHeight;
            
            setHeight(newHeight);
        };

        const onMouseUp = () => {
            document.removeEventListener("mousemove", onMouseMove);
            document.removeEventListener("mouseup", onMouseUp);
        };

        document.addEventListener("mousemove", onMouseMove);
        document.addEventListener("mouseup", onMouseUp);
    };

    return (
        <div className="flex flex-col bg-zinc-900 border-t border-zinc-800 shrink-0 relative z-20" style={{ height }}>
            {/* Top Resize Handle */}
            <div 
                className="absolute top-0 left-0 right-0 h-1 bg-transparent hover:bg-violet-500/50 cursor-ns-resize transition-colors shrink-0 z-30 transform -translate-y-1/2"
                onMouseDown={handleMouseDown}
            />
            {/* Header */}
            <div className="flex items-center justify-between px-3 py-1.5 border-b border-zinc-800 shrink-0 bg-zinc-950/50">
                <span className="text-xs font-semibold text-zinc-400 pl-1">Notes</span>
                <div className="flex bg-zinc-900 border border-zinc-800 rounded">
                    <button 
                        className={`px-3 py-1 text-xs transition-colors rounded-l ${mode === "edit" ? "bg-violet-500/20 text-violet-300" : "text-zinc-400 hover:text-zinc-200"}`}
                        onClick={() => setMode("edit")}
                    >
                        Edit
                    </button>
                    <button 
                        className={`px-3 py-1 text-xs border-l border-zinc-800 transition-colors ${mode === "split" ? "bg-violet-500/20 text-violet-300" : "text-zinc-400 hover:text-zinc-200"}`}
                        onClick={() => setMode("split")}
                    >
                        Split
                    </button>
                    <button 
                        className={`px-3 py-1 text-xs border-l border-zinc-800 transition-colors rounded-r ${mode === "preview" ? "bg-violet-500/20 text-violet-300" : "text-zinc-400 hover:text-zinc-200"}`}
                        onClick={() => setMode("preview")}
                    >
                        Preview
                    </button>
                </div>
            </div>
            {/* Editor Body */}
            <div className="flex-1 overflow-hidden" data-color-mode="dark">
                {mode === "preview" ? (
                    <div className="h-full overflow-y-auto p-4 bg-zinc-900 custom-scrollbar">
                        <MDEditor.Markdown source={notes} className="wmde-markdown-custom" />
                    </div>
                ) : (
                    <MDEditor
                        value={notes}
                        onChange={(val) => onChange(val || "")}
                        preview={mode === "split" ? "live" : "edit"}
                        hideToolbar={true}
                        height="100%"
                        className="border-none !shadow-none !rounded-none bg-zinc-900"
                    />
                )}
            </div>
        </div>
    );
}
