// ── Serializable types mirroring the Rust backend ────────────────────────────

export interface LoadedTab {
    id: string;
    name: string;
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
    };
}
