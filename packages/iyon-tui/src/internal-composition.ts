/**
 * PERF-12 T13.1 — PRIVATE composition/construction surface.
 *
 * @internal Framework-private infrastructure. It is NOT part of the public
 * `iyon-tui` API (§26): applications must never import it.
 *
 * History: this facade once also exposed the Step 2 slot runtime and the
 * module/site registry consumed by the abandoned lexical SiteId transform.
 * Per AMENDMENT-C §11/§17 the transform architecture was removed; R0 retired
 * that machinery entirely (handoff §32.1 R0). What remains is the stable
 * active-scope-form helper surface (AMENDMENT-C §17.3) that R1's
 * RetainedExecutionRuntime will drive through scope-local semantic slots,
 * plus the §12 comparator contracts preserved in git history (`dad92b5`).
 */

export {
  composeBackground,
  composeBorder,
  composeClampRows,
  composeComponent,
  composeContainer,
  composeContentMax,
  composeDiff,
  composeFillHeight,
  composeFillWidth,
  composeFitHeight,
  composeFitWidth,
  composeForeground,
  composeHanging,
  composeHorizontal,
  composeMaxHeight,
  composeMaxWidth,
  composeMinHeight,
  composeMinWidth,
  composePadding,
  composeSpacer,
  composeStyle as composeStyleSpec,
  composeStyleState,
  composeStyledText,
  composeText,
  composeTextAlign,
  composeTextAttribute,
  composeVertical,
  composeWrap,
} from "./compose.ts";
