import {
  AppHarness,
  Component,
  History,
  Scene,
  TextInput,
  TextStream,
  View,
} from "../../src/index.ts";

export async function runTuiDemo(): Promise<{
  screenRows: readonly string[];
  nativeHistoryRows: readonly string[];
  input: string;
  stream: string;
  focused: boolean;
}> {
  const harness = await AppHarness.open({ width: 16, height: 3 });
  const history = new History();
  const input = new TextInput();
  const stream = new TextStream();
  const component = new Component();

  await input.setText("compose");
  await history.push(View.text("completed history"));
  await stream.update("streaming text");
  await history.pushStream(stream);
  const body = View.vertical([
    View.text("composer").bold(),
    await input.view(),
    View.text((await stream.snapshot()).text),
    await component.view(),
  ]);
  await harness.render(new Scene(body, history));
  harness.pressKey("Enter");

  const screenRows = harness.screenRows();
  const nativeHistoryRows = harness.nativeHistoryRows();
  await stream.seal();
  await component.dispose();
  await input.dispose();
  await history.dispose();
  await stream.dispose();
  await harness.close();
  return {
    screenRows,
    nativeHistoryRows,
    input: "compose",
    stream: "streaming text",
    focused: true,
  };
}
