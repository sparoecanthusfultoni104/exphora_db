// ui/src/types/index.ts

// ── Serializable types mirroring the Rust backend ────────────────────────────

export interface LoadedTab {
    id: string;
    name: string;
    path?: string;
    columns: string[];
    records: Record<string, unknown>[];
    total_rows: number;
}

export interface AppInfo {
    version: string;
    build_date: string;
}

// ── Filter types ──────────────────────────────────────────────────────────────

export type FilterOp =
    | "Contains"
    | "NotContains"
    | "Equals"
    | "NotEquals"
    | "GreaterThan"
    | "LessThan"
    | "IsNull"
    | "IsNotNull"
    | "BoolTrue";

export type FilterConnector = "And" | "Or";
export type FilterMode = "Easy" | "Advanced";

export interface FilterRule {
    op: FilterOp;
    value: string;
    connector: FilterConnector;
}

export interface EasyFilter {
    selected: string[];
    all_selected: boolean;
}

export interface DynamicFiltersDto {
    text_search: string;
    filters: Record<string, FilterRule[]>;
    easy_filters: Record<string, EasyFilter>;
    filter_mode: Record<string, FilterMode>;
}

// ── Stats ─────────────────────────────────────────────────────────────────────

export interface ColumnStats {
    total: number;
    non_null: number;
    unique: number;
    min: number | null;
    max: number | null;
    mean: number | null;
    median: number | null;
    top_values: [string, number][];
    is_numeric: boolean;
}

// ── Command results ───────────────────────────────────────────────────────────

export interface FilterResult {
    filtered_indices: number[];
    total_matching: number;
}

export interface UniqueValuesResult {
    col: string;
    values: [string, number][];
    truncated: boolean;
}

// ── Per-tab UI state (managed in frontend only) ───────────────────────────────

export interface ViewState {
    datasetPath: string;
    filters: Record<string, FilterRule[]> | any;
    textSearch: string;
    visibleColumns: Record<string, boolean>;
    frozenCols: string[];
    calcCols: { name: string; expr: string }[];
    sortCol: string | null;
    sortAsc: boolean;
    showFrequencyChart: boolean;
    frequencyChartCol: string | null;
    charts: any | null;
}

export interface TabUiState {
    filteredIndices: number[];
    filters: DynamicFiltersDto;
    sortCol: string | null;
    sortAsc: boolean;
    visibleColumns: Record<string, boolean>;
    frozenCols: string[];
    calcCols: { name: string; expr: string }[];
    calcColCache: Record<string, (string | null)[]>;
    textSearch: string;
    activeStatsCol: string | null;
    activeStats: ColumnStats | null;
    showFrequencyChart: boolean;
    frequencyChartCol: string | null;

    // Feature: Inline Editing
    editingCell: { rowIndex: number; colName: string } | null;
    editHistory: {
        past: Array<{ rowIndex: number; colName: string; oldValue: string; newValue: string }>;
        future: Array<{ rowIndex: number; colName: string; oldValue: string; newValue: string }>;
    };
    saveStatus: 'idle' | 'saving' | 'saved' | 'error';
    editedCells: Record<string, boolean>; // key format: `${rowIndex}-${colName}`
    viewNotes: string;
    columnNotes: Record<string, string>;
    savedViewPath?: string;
}

export function defaultTabUiState(tab: LoadedTab): TabUiState {
    const visibleColumns: Record<string, boolean> = {};
    for (const col of tab.columns) visibleColumns[col] = true;
    return {
        filteredIndices: tab.records.map((_, i) => i),
        filters: {
            text_search: "",
            filters: {},
            easy_filters: {},
            filter_mode: {},
        },
        sortCol: null,
        sortAsc: true,
        visibleColumns,
        frozenCols: [],
        calcCols: [],
        calcColCache: {},
        textSearch: "",
        activeStatsCol: null,
        activeStats: null,
        showFrequencyChart: false,
        frequencyChartCol: null,
        editingCell: null,
        editHistory: { past: [], future: [] },
        saveStatus: 'idle',
        editedCells: {},
        viewNotes: "",
        columnNotes: {}
    };
}

export function toViewState(tab: LoadedTab, ui: TabUiState): ViewState {
    if (!tab.path) {
        throw new Error("Cannot save a view for a tab without a file path.");
    }
    return {
        datasetPath: tab.path,
        filters: JSON.parse(JSON.stringify(ui.filters)),
        textSearch: ui.textSearch,
        visibleColumns: { ...ui.visibleColumns },
        frozenCols: [...ui.frozenCols],
        calcCols: ui.calcCols.map(c => ({ ...c })),
        sortCol: ui.sortCol,
        sortAsc: ui.sortAsc,
        showFrequencyChart: ui.showFrequencyChart,
        frequencyChartCol: ui.frequencyChartCol,
        charts: null
    };
}

export function fromViewState(view: ViewState): Partial<TabUiState> {
    return {
        filters: JSON.parse(JSON.stringify(view.filters)),
        textSearch: view.textSearch,
        visibleColumns: { ...view.visibleColumns },
        frozenCols: [...view.frozenCols],
        calcCols: view.calcCols.map(c => ({ ...c })),
        sortCol: view.sortCol,
        sortAsc: view.sortAsc,
        showFrequencyChart: view.showFrequencyChart,
        frequencyChartCol: view.frequencyChartCol
    };
}

export interface RecentViewEntry {
    name: string;        // nombre de la vista dado por el usuario
    path: string;        // ruta absoluta del archivo .exh
    datasetPath: string; // ruta del dataset asociado
    openedAt: string;    // ISO 8601 timestamp
}
