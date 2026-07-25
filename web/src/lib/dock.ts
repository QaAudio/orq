/**
 * Thin compatibility shim — Details layout is now a 12-col reactive grid
 * (see canvasGrid.ts). Panel id/label constants remain for panel components.
 */
export {
  DETAILS_PANEL_IDS as ALL_PANEL_IDS,
  DETAILS_PANEL_LABELS as PANEL_LABELS,
  DEFAULT_PANEL_HEIGHTS,
  type DetailsPanelId,
} from "./canvasGrid";

import type { DetailsPanelId } from "./canvasGrid";
import { DETAILS_PANEL_IDS } from "./canvasGrid";

export type PanelId = DetailsPanelId;

export function isPanelId(v: unknown): v is PanelId {
  return typeof v === "string" && (DETAILS_PANEL_IDS as readonly string[]).includes(v);
}
