import { fetchModule } from "./utils";

// No exports
test("react@18", async () => {
  const module = await fetchModule("react", "18.1.0");
  expect(module.m.length).toBe(0);
  expect(
    Object.entries(module.f)
      .map(([file, type]) => {
        return [file, typeof type];
      })
      .sort((a, b) => a[0].localeCompare(b[0]))
  ).toMatchSnapshot();
});

// Conditional root exports
test("framer@1.3.6", async () => {
  const module = await fetchModule("framer", "1.3.6");
  expect(module.m.length).toBe(1);
  expect(
    Object.entries(module.f)
      .map(([file, type]) => {
        return [file, typeof type];
      })
      .sort((a, b) => a[0].localeCompare(b[0]))
  ).toMatchSnapshot();
});

// Relative and wildcard exports
test("framer@2.0.0-beta.13", async () => {
  const module = await fetchModule("framer", "2.0.0-beta.13");
  expect(module.m.length).toBe(13);
  expect(
    Object.entries(module.f)
      .map(([file, type]) => {
        return [file, typeof type];
      })
      .sort((a, b) => a[0].localeCompare(b[0]))
  ).toMatchSnapshot();
});

// Array exports
test("@babel/runtime@7.16.5", async () => {
  const module = await fetchModule("@babel/runtime", "7.16.5");
  expect(module.m.length).toBe(1);
  expect(
    Object.entries(module.f)
      .map(([file, type]) => {
        return [file, typeof type];
      })
      .sort((a, b) => a[0].localeCompare(b[0]))
  ).toMatchSnapshot();
});

// No main export, fallback to index
test("object-assign@4.1.1", async () => {
  const module = await fetchModule("object-assign", "4.1.1");
  expect(module.m.length).toBe(0);
  expect(
    Object.entries(module.f)
      .map(([file, type]) => {
        return [file, typeof type];
      })
      .sort((a, b) => a[0].localeCompare(b[0]))
  ).toMatchSnapshot();
});
