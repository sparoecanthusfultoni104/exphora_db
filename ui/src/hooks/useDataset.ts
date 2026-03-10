import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LoadedTab } from "../types";
import { useAppStore } from "../store/appStore";

export function useDataset() {
    const addTabs = useAppStore((s) => s.addTabs);
    const removeTab = useAppStore((s) => s.removeTab);
    const addRecentFile = useAppStore((s) => s.addRecentFile);

    const openFile = useCallback(async () => {
        try {
            const path = await invoke<string | null>("open_file_dialog");
            if (!path) return;
            const tabs = await invoke<LoadedTab[]>("load_file", { path });
            if (tabs.length === 0) return;
            addTabs(tabs);
            addRecentFile(path);
        } catch (err) {
            console.error("openFile error:", err);
            alert(`Error abriendo archivo: ${err}`);
        }
    }, [addTabs, addRecentFile]);

    const openPath = useCallback(
        async (path: string) => {
            try {
                const tabs = await invoke<LoadedTab[]>("load_file", { path });
                if (tabs.length === 0) return;
                addTabs(tabs);
                addRecentFile(path);
            } catch (err) {
                console.error("openPath error:", err);
                alert(`Error cargando ${path}: ${err}`);
            }
        },
        [addTabs, addRecentFile]
    );

    const closeTab = useCallback(
        (id: string) => {
            removeTab(id);
        },
        [removeTab]
    );

    return { openFile, openPath, closeTab };
}
