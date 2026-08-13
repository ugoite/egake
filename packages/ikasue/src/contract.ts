/** Versioned native contract shared by Egake and Ikasue. */
export const IKASUE_ABI_VERSION = "ikasue-web/1" as const;

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | JsonObject;
export type JsonObject = { readonly [key: string]: JsonValue };

export interface IkaView {
  readonly version: typeof IKASUE_ABI_VERSION;
  readonly kind: string;
  readonly props?: Readonly<Record<string, JsonValue>>;
  readonly children?: readonly IkaView[];
  readonly text?: string;
}

export interface IkaPage {
  readonly name: string;
  readonly title: string;
  readonly view: IkaView;
}

export interface IkaSort {
  readonly field: string;
  readonly direction: "asc" | "desc";
}

export interface IkaQuery {
  readonly offset: number;
  readonly limit: number;
  readonly sort: readonly IkaSort[];
  readonly filter?: string;
}

export interface IkaEdit {
  readonly rowId: string;
  readonly columnId: string;
  readonly value: JsonValue;
}

export interface IkaSelect {
  readonly rowId: string;
  readonly columnId?: string;
}

export interface IkaDataGridColumn {
  readonly id: string;
  readonly label: string;
}

export interface IkaDataGridRow {
  readonly id: string;
  readonly cells: Readonly<Record<string, JsonValue>>;
}

export interface IkaDataGridProps {
  readonly columns: readonly IkaDataGridColumn[];
  readonly rows: readonly IkaDataGridRow[];
  readonly total: number;
  readonly loading: boolean;
  readonly error?: string;
  readonly editable?: boolean;
}

export interface IkaEventMap {
  "ika-query": IkaQuery;
  "ika-edit": IkaEdit;
  "ika-select": IkaSelect;
  "ika-action": { readonly id: string };
}
