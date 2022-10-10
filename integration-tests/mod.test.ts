import { fetchV2Module, V2Module } from "./utils";

function getSnapshot(data: V2Module): Record<string, number> {
  const res = {};
  for (let [key, value] of Object.entries(data)) {
    res[key] = value.byteLength;
  }
  return res;
}

test("react@18.1.0", async () => {
  const module = await fetchV2Module("react", "18.1.0");
  expect(getSnapshot(module)).toMatchSnapshot();
});

test("next@12.3.1", async () => {
  const module = await fetchV2Module("next", "12.3.1");
  expect(getSnapshot(module)).toMatchSnapshot();
});
