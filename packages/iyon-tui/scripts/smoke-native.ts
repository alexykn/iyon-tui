import { TextFunnel, TextStreamSource, View } from "../src/index.ts";
import { AppHarness } from "../src/testing/index.ts";

const source = TextStreamSource.create();
const harness = await AppHarness.open({ width: 32, height: 4 });
try {
  const port = harness.contentPort();
  const connector = port.connect(source, TextFunnel.plain());
  connector.activate();
  harness.render({ body: View.content(port) });
  source.append("packaged TUI smoke\n");
  harness.flush();
  const rows = harness.screenRows();
  if (!rows.some((row) => row.includes("packaged TUI smoke"))) {
    throw new Error(`native package smoke did not render content: ${JSON.stringify(rows)}`);
  }
  console.log(JSON.stringify({ native: "iyon-tui-native/s6", content: "ok", rows }));
} finally {
  harness.close();
  source.dispose();
}
