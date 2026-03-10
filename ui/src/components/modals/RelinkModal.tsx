import React from "react";
import { FolderSearch, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";

export interface RelinkModalProps {
    viewFilePath: string;
    onClose: () => void;
    onRelinkSuccess: (viewFile: any) => void;
}

export function RelinkModal({ viewFilePath, onClose, onRelinkSuccess }: RelinkModalProps) {
    const handleSearch = async () => {
        try {
            const { open } = await import("@tauri-apps/plugin-dialog");
            const newPath = await open({
                title: "Select new dataset location",
            });
            if (typeof newPath === "string") {
                const updatedViewFile = await invoke("relink_view", {
                    viewFilePath,
                    newDatasetPath: newPath
                });
                onRelinkSuccess(updatedViewFile);
            }
        } catch (err) {
            alert(`Error reconnecting view: ${err}`);
        }
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
            <div className="bg-zinc-900 border border-zinc-800 rounded-lg shadow-xl w-full max-w-sm flex flex-col overflow-hidden animate-slide-up">
                <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800 bg-zinc-950/50">
                    <h2 className="text-zinc-100 font-medium text-sm flex items-center gap-2">
                        <FolderSearch size={16} className="text-violet-500" />
                        File not found
                    </h2>
                    <button
                        onClick={onClose}
                        className="text-zinc-500 hover:text-zinc-300 transition-colors p-1"
                    >
                        <X size={14} />
                    </button>
                </div>
                <div className="p-4">
                    <p className="text-zinc-300 text-xs mb-4 leading-relaxed">
                        The data file cannot be found in its original location. 
                        Select the new location to reconnect this view.
                    </p>
                    <div className="flex justify-end gap-2">
                        <button
                            onClick={onClose}
                            className="px-3 py-1.5 rounded-md text-xs font-medium text-zinc-300 hover:bg-zinc-800 hover:text-white transition-colors"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={handleSearch}
                            className="px-3 py-1.5 rounded-md text-xs font-medium bg-violet-600 text-white hover:bg-violet-500 transition-colors shadow-sm"
                        >
                            Browse file
                        </button>
                    </div>
                </div>
            </div>
        </div>
    );
}
