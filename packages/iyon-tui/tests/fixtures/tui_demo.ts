import { History, Scene, TextFunnel, TextStreamSource, View } from "../../src/index.ts";
import { AppHarness } from "../../src/testing/index.ts";

export async function runTuiDemo(): Promise<{
  screenRows: readonly string[];
  nativeHistoryRows: readonly string[];
  input: string;
  stream: string;
  focused: boolean;
}> {
  const harness = await AppHarness.open({ width: 16, height: 3 });
  const history = new History();
  const input = harness.createTextInput();
  const source = TextStreamSource.create();
  const port = harness.contentPort();
  const connector = port.connect(source, TextFunnel.plain());
  const slot = harness.createViewSlot(View.spacer(0));

  await input.setText("compose");
  await history.push(View.text("completed history"));
  connector.activate();
  source.append("streaming text");
  const body = View.vertical([
    View.text("composer").bold(),
    await input.view(),
    View.content(port),
    await slot.view(),
  ]);
  await harness.render(new Scene(body, history));
  harness.pressKey("Enter");

  const screenRows = harness.screenRows();
  const nativeHistoryRows = harness.nativeHistoryRows();
  source.seal();
  await slot.dispose();
  await input.dispose();
  await history.dispose();
  await harness.close();
  source.dispose();
  return {
    screenRows,
    nativeHistoryRows,
    input: "compose",
    stream: "streaming text",
    focused: true,
  };
}
