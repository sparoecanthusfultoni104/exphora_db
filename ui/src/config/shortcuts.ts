export interface Shortcut {
    key: string;
    ctrlKey: boolean;
    shiftKey: boolean;
    description: string;
    context: string;
}

export const SHORTCUTS: Shortcut[] = [
    { key: "O", ctrlKey: true, shiftKey: false, description: "Abrir archivo", context: "Global" },
    { key: "W", ctrlKey: true, shiftKey: false, description: "Cerrar pestaña activa", context: "Global" },
    { key: "R", ctrlKey: true, shiftKey: false, description: "Recargar dataset actual", context: "Global" },
    { key: "Tab", ctrlKey: true, shiftKey: false, description: "Pestaña siguiente", context: "Global" },
    { key: "Tab", ctrlKey: true, shiftKey: true, description: "Pestaña anterior", context: "Global" },
    { key: "1-9", ctrlKey: true, shiftKey: false, description: "Saltar a pestaña 1-9", context: "Global" },
    { key: "F", ctrlKey: true, shiftKey: false, description: "Enfocar búsqueda global", context: "Global" },
    { key: "C", ctrlKey: true, shiftKey: true, description: "Limpiar todos los filtros", context: "Global" },
    { key: "F", ctrlKey: true, shiftKey: true, description: "Abrir buscador de columnas (Filtrar)", context: "Global" },
    { key: "S", ctrlKey: true, shiftKey: true, description: "Abrir buscador de columnas (Estadísticas)", context: "Global" },
    { key: "G", ctrlKey: true, shiftKey: true, description: "Abrir buscador de columnas (Gráfico)", context: "Global" },
    { key: "E", ctrlKey: true, shiftKey: false, description: "Exportar dataset", context: "Global" },
    { key: "D", ctrlKey: true, shiftKey: false, description: "Alternar modo claro/oscuro", context: "Global" },
    { key: ",", ctrlKey: true, shiftKey: false, description: "Abrir configuración", context: "Global" },
    { key: "P", ctrlKey: true, shiftKey: false, description: "Abrir panel P2P", context: "Global" },
    { key: "Escape", ctrlKey: false, shiftKey: false, description: "Cerrar overlays/modales", context: "Global" },
];
