import type { ExtensionAPI } from "iyon:plugins";
import { Scene, View } from "@iyon/tui";
export function activate(api: ExtensionAPI) { api.scene.replace({ id: "fixture.scene.replace", replace: () => new Scene(View.text("replacement")) }); }
